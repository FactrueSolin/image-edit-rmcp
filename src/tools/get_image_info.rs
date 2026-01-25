use anyhow::Result;
use rmcp::{
    ErrorData as McpError,
    handler::server::tool::Parameters,
    model::{CallToolResult, Content},
    schemars::JsonSchema,
};
use serde::{Deserialize, Serialize};

use crate::{image_processing, tools::validate_http_url};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetImageInfoRequest {
    #[schemars(description = "图像URL")]
    pub url: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
    pub total_pixels: u64,
    pub mime_type: String,
    pub size: usize,
    pub aspect_ratio: Option<f64>,
}

pub async fn get_image_info(
    Parameters(request): Parameters<GetImageInfoRequest>,
) -> Result<CallToolResult, McpError> {
    let validated_url = validate_http_url(&request.url)?;
    let validated_url = validated_url.to_string();
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
        .unwrap_or_else(|| "unknown".to_string());
    let (width, height) = image_processing::get_dimensions(bytes.as_ref(), &mime_type)
        .map_err(|err| {
            McpError::internal_error(
                "decode image failed",
                Some(serde_json::Value::String(err.to_string())),
            )
        })?;
    let total_pixels = (width as u64).saturating_mul(height as u64);
    let aspect_ratio = if height == 0 {
        None
    } else {
        Some(width as f64 / height as f64)
    };

    let info = ImageInfo {
        width,
        height,
        total_pixels,
        mime_type,
        size: bytes.len(),
        aspect_ratio,
    };
    let json = serde_json::to_string(&info).map_err(|err| {
        McpError::internal_error(
            "serialize info failed",
            Some(serde_json::Value::String(err.to_string())),
        )
    })?;
    Ok(CallToolResult::success(vec![Content::text(json)]))
}
