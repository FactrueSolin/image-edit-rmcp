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
    tools::{ToolResponse, validate_http_url},
};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CropImageRequest {
    #[schemars(description = "图像URL")]
    pub url: String,
    #[schemars(description = "左侧百分比")]
    pub left: Option<f32>,
    #[schemars(description = "顶部百分比")]
    pub top: Option<f32>,
    #[schemars(description = "右侧百分比")]
    pub right: Option<f32>,
    #[schemars(description = "底部百分比")]
    pub bottom: Option<f32>,
}

pub async fn crop_image(
    storage: &LocalFileStorage,
    Parameters(request): Parameters<CropImageRequest>,
) -> Result<CallToolResult, McpError> {
    let validated_url = validate_http_url(&request.url)?;
    let validated_url = validated_url.to_string();
    let left_value = request.left.unwrap_or(0.0);
    let top_value = request.top.unwrap_or(0.0);
    let right_value = request.right.unwrap_or(100.0);
    let bottom_value = request.bottom.unwrap_or(100.0);
    let left_ratio = left_value / 100.0;
    let top_ratio = top_value / 100.0;
    let right_ratio = right_value / 100.0;
    let bottom_ratio = bottom_value / 100.0;

    if left_ratio >= right_ratio {
        return Err(McpError::invalid_params("left must be less than right", None));
    }
    if top_ratio >= bottom_ratio {
        return Err(McpError::invalid_params("top must be less than bottom", None));
    }

    let cache_key_input = format!(
        "crop:{}:{}:{}:{}:{}",
        validated_url, left_value, top_value, right_value, bottom_value
    );
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
            return Ok(CallToolResult::success(vec![
                Content::text(json),
            ]));
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
    let dims = image_processing::get_cropped_dimensions(
        width,
        height,
        left_ratio,
        top_ratio,
        right_ratio,
        bottom_ratio,
    );
    if dims[0] == 0 || dims[1] == 0 {
        return Err(McpError::invalid_params("cropped size is zero", None));
    }
    let cropped_pixels = image_processing::crop_pixels(
        &pixels,
        width,
        height,
        left_ratio,
        top_ratio,
        right_ratio,
        bottom_ratio,
    );
    let cropped_bytes = image_processing::encode_png(&cropped_pixels, dims[0], dims[1]).map_err(
        |err| {
            McpError::internal_error(
                "encode image failed",
                Some(serde_json::Value::String(err.to_string())),
            )
        },
    )?;

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
