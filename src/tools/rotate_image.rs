use anyhow::Result;
use rmcp::{
    ErrorData as McpError,
    handler::server::tool::Parameters,
    model::{CallToolResult, Content},
    schemars::JsonSchema,
};
use serde::Deserialize;
use chrono::Utc;
use base64::Engine;

use crate::{
    cache::{
        LocalFileStorage,
        ProcessedImageCacheMetadata,
        compute_hash,
    },
    image_processing,
    tools::ToolResponse,
};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RotateDirection {
    Right90,
    Left90,
    Flip180,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RotateImageRequest {
    #[schemars(description = "图像URL")]
    pub url: String,
    #[schemars(description = "旋转方向")]
    pub direction: RotateDirection,
}

pub async fn rotate_image(
    storage: &LocalFileStorage,
    Parameters(request): Parameters<RotateImageRequest>,
) -> Result<CallToolResult, McpError> {
    let cache_key_input = format!("rotate:{}:{}", request.url, match request.direction {
        RotateDirection::Right90 => "right_90",
        RotateDirection::Left90 => "left_90",
        RotateDirection::Flip180 => "flip_180",
    });
    let hash = compute_hash(&cache_key_input);
    let prefix = format!("processed/{hash}");
    let meta_key = LocalFileStorage::get_meta_key(&prefix);
    if let Ok(Some(meta_bytes)) = storage.get(&meta_key).await {
        if let Ok(metadata) = serde_json::from_slice::<ProcessedImageCacheMetadata>(&meta_bytes) {
            let response = ToolResponse {
                url: metadata.cached_image_url,
                name: "rotated-image".to_string(),
                mime_type: metadata.mime_type,
                text: "图像已旋转。".to_string(),
            };
            let json = serde_json::to_string(&response).map_err(|err| {
                McpError::internal_error(
                    "serialize tool response failed",
                    Some(serde_json::Value::String(err.to_string())),
                )
            })?;
            return Ok(CallToolResult::success(vec![
                Content::text(json),
            ]));
        }
    }
    let response = reqwest::get(&request.url).await.map_err(|err| {
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
    let angle = match request.direction {
        RotateDirection::Right90 => 90,
        RotateDirection::Left90 => -90,
        RotateDirection::Flip180 => 180,
    };
    let dims = image_processing::get_rotated_dimensions(width, height, angle);
    let rotated_pixels = image_processing::rotate_pixels(&pixels, width, height, angle);
    let rotated_bytes = image_processing::encode_png(&rotated_pixels, dims[0], dims[1]).map_err(
        |err| {
            McpError::internal_error(
                "encode image failed",
                Some(serde_json::Value::String(err.to_string())),
            )
        },
    )?;

    let cached_image_key = LocalFileStorage::get_result_key(&prefix, "png");
    if let Err(_err) = storage.put(&cached_image_key, &rotated_bytes).await {
        let base64_image = base64::engine::general_purpose::STANDARD.encode(&rotated_bytes);
        let response = ToolResponse {
            url: String::new(),
            name: "rotated-image".to_string(),
            mime_type: "image/png".to_string(),
            text: "图像已旋转。".to_string(),
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
        name: "rotated-image".to_string(),
        mime_type: "image/png".to_string(),
        text: "图像已旋转。".to_string(),
    };
    let json = serde_json::to_string(&response).map_err(|err| {
        McpError::internal_error(
            "serialize tool response failed",
            Some(serde_json::Value::String(err.to_string())),
        )
    })?;
    Ok(CallToolResult::success(vec![Content::text(json)]))
}
