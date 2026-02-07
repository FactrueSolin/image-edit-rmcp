use anyhow::Result;
use base64::Engine;
use chrono::Utc;
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content},
    schemars::JsonSchema,
};
use serde::Deserialize;

use crate::{
    cache::{LocalFileStorage, ProcessedImageCacheMetadata, compute_hash},
    image_processing,
    tools::{ToolResponse, validate_http_url},
};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CropImageRequest {
    #[schemars(description = "图像URL")]
    pub image_url: String,
    #[schemars(description = "左上角 x 坐标（Qwen3 坐标，0-999）")]
    pub x1: u32,
    #[schemars(description = "左上角 y 坐标（Qwen3 坐标，0-999）")]
    pub y1: u32,
    #[schemars(description = "右下角 x 坐标（Qwen3 坐标，0-999）")]
    pub x2: u32,
    #[schemars(description = "右下角 y 坐标（Qwen3 坐标，0-999）")]
    pub y2: u32,
}

pub async fn crop_image(
    storage: &LocalFileStorage,
    Parameters(request): Parameters<CropImageRequest>,
) -> Result<CallToolResult, McpError> {
    let validated_url = validate_http_url(&request.image_url)?;
    let validated_url = validated_url.to_string();
    let x1 = request.x1;
    let y1 = request.y1;
    let x2 = request.x2;
    let y2 = request.y2;

    let max_coord = 999;
    if x1 > max_coord || y1 > max_coord || x2 > max_coord || y2 > max_coord {
        return Err(McpError::invalid_params(
            "coordinates must be within [0, 999]",
            None,
        ));
    }

    let cache_key_input = format!("crop:{}:{}:{}:{}:{}", validated_url, x1, y1, x2, y2);
    let hash = compute_hash(&cache_key_input);
    let prefix = format!("processed/{hash}");
    let meta_key = LocalFileStorage::get_meta_key(&prefix);
    if let Ok(Some(meta_bytes)) = storage.get(&meta_key).await {
        if let Ok(metadata) = serde_json::from_slice::<ProcessedImageCacheMetadata>(&meta_bytes) {
            let response = ToolResponse {
                url: metadata.cached_image_url,
                name: "cropped-image".to_string(),
                mime_type: metadata.mime_type,
                text: "图像已裁剪。".to_string(),
            };
            let json = serde_json::to_string(&response).map_err(|err| {
                McpError::internal_error(
                    "serialize tool response failed",
                    Some(serde_json::Value::String(err.to_string())),
                )
            })?;
            return Ok(CallToolResult::success(vec![Content::text(json)]));
        }
    }

    let response = reqwest::get(&validated_url).await.map_err(|err| {
        McpError::internal_error(
            "fetch image failed",
            Some(serde_json::Value::String(err.to_string())),
        )
    })?;
    let status = response.status();
    if !status.is_success() {
        return Err(McpError::internal_error(
            "fetch image failed",
            Some(serde_json::Value::String(format!("HTTP {status}"))),
        ));
    }
    let headers = response.headers().clone();
    let bytes = response.bytes().await.map_err(|err| {
        McpError::internal_error(
            "read image bytes failed",
            Some(serde_json::Value::String(err.to_string())),
        )
    })?;
    let mime_from_header = headers
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.split(';').next().unwrap_or(value).trim().to_string());
    let detected = image_processing::detect_mime_type(bytes.as_ref()).map(str::to_string);
    let mime_type = detected
        .or(mime_from_header)
        .ok_or_else(|| McpError::internal_error("unsupported image type", None))?;

    let (pixels, width, height) = image_processing::decode_image(bytes.as_ref(), &mime_type)
        .map_err(|err| {
            McpError::internal_error(
                "decode image failed",
                Some(serde_json::Value::String(err.to_string())),
            )
        })?;
    let mut abs_x1 = (x1 as f32 / 1000.0 * width as f32).floor() as u32;
    let mut abs_y1 = (y1 as f32 / 1000.0 * height as f32).floor() as u32;
    let mut abs_x2 = (x2 as f32 / 1000.0 * width as f32).floor() as u32;
    let mut abs_y2 = (y2 as f32 / 1000.0 * height as f32).floor() as u32;

    if abs_x1 > abs_x2 {
        std::mem::swap(&mut abs_x1, &mut abs_x2);
    }
    if abs_y1 > abs_y2 {
        std::mem::swap(&mut abs_y1, &mut abs_y2);
    }

    abs_x1 = abs_x1.min(width);
    abs_x2 = abs_x2.min(width);
    abs_y1 = abs_y1.min(height);
    abs_y2 = abs_y2.min(height);

    let new_width = abs_x2.saturating_sub(abs_x1);
    let new_height = abs_y2.saturating_sub(abs_y1);
    if new_width == 0 || new_height == 0 {
        return Err(McpError::invalid_params("cropped size is zero", None));
    }

    let expected_len = width.saturating_mul(height).saturating_mul(4u32) as usize;
    if pixels.len() != expected_len {
        return Err(McpError::internal_error("invalid image buffer", None));
    }

    let mut cropped_pixels = vec![0u8; (new_width * new_height * 4) as usize];
    for y in 0..new_height {
        for x in 0..new_width {
            let src_x = abs_x1 + x;
            let src_y = abs_y1 + y;
            let src_index = ((src_y * width + src_x) * 4) as usize;
            let dst_index = ((y * new_width + x) * 4) as usize;
            cropped_pixels[dst_index..dst_index + 4]
                .copy_from_slice(&pixels[src_index..src_index + 4]);
        }
    }

    let cropped_bytes = image_processing::encode_png(&cropped_pixels, new_width, new_height)
        .map_err(|err| {
            McpError::internal_error(
                "encode image failed",
                Some(serde_json::Value::String(err.to_string())),
            )
        })?;

    let cached_image_key = LocalFileStorage::get_result_key(&prefix, "png");
    if let Err(_err) = storage.put(&cached_image_key, &cropped_bytes).await {
        let base64_image = base64::engine::general_purpose::STANDARD.encode(&cropped_bytes);
        let response = ToolResponse {
            url: String::new(),
            name: "cropped-image".to_string(),
            mime_type: "image/png".to_string(),
            text: "图像已裁剪。".to_string(),
        };
        let json = serde_json::to_string(&response).map_err(|err| {
            McpError::internal_error(
                "serialize tool response failed",
                Some(serde_json::Value::String(err.to_string())),
            )
        })?;
        return Ok(CallToolResult::success(vec![
            Content::image(base64_image, "image/png"),
            Content::text(json),
        ]));
    }
    let cached_image_url = storage.get_public_url(&cached_image_key);
    let metadata = ProcessedImageCacheMetadata {
        cache_key_input,
        cached_image_key,
        cached_image_url: cached_image_url.clone(),
        mime_type: "image/png".to_string(),
        created_at: Utc::now().to_rfc3339(),
    };
    let meta_json = serde_json::to_vec(&metadata).map_err(|err| {
        McpError::internal_error(
            "serialize cache metadata failed",
            Some(serde_json::Value::String(err.to_string())),
        )
    })?;
    storage.put(&meta_key, &meta_json).await.map_err(|err| {
        McpError::internal_error(
            "save cache metadata failed",
            Some(serde_json::Value::String(err.to_string())),
        )
    })?;

    let response = ToolResponse {
        url: cached_image_url,
        name: "cropped-image".to_string(),
        mime_type: "image/png".to_string(),
        text: "图像已裁剪。".to_string(),
    };
    let json = serde_json::to_string(&response).map_err(|err| {
        McpError::internal_error(
            "serialize tool response failed",
            Some(serde_json::Value::String(err.to_string())),
        )
    })?;
    Ok(CallToolResult::success(vec![Content::text(json)]))
}
