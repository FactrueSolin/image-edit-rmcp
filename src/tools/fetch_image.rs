use anyhow::Result;
use rmcp::{
    ErrorData as McpError,
    handler::server::tool::Parameters,
    model::{CallToolResult, Content},
    schemars::JsonSchema,
};
use serde::{Deserialize, Serialize};

use chrono::Utc;

use crate::{
    cache::{
        ImageCacheMetadata,
        LocalFileStorage,
        compute_hash,
    },
    image_processing,
    modelscope,
    tools::{ToolResponse, validate_http_url},
};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FetchImageRequest {
    #[schemars(description = "图像URL")]
    pub url: String,
    #[schemars(description = "需要关注的重要内容")]
    pub focus: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct ImageInfo {
    pub width: u32,
    pub height: u32,
    pub total_pixels: u64,
    pub mime_type: String,
    pub size: usize,
    pub aspect_ratio: Option<f64>,
}

pub async fn fetch_image(
    storage: &LocalFileStorage,
    Parameters(request): Parameters<FetchImageRequest>,
) -> Result<CallToolResult, McpError> {
    let validated_url = validate_http_url(&request.url)?;
    let validated_url = validated_url.to_string();
    let cache_key_input = match request.focus.as_deref() {
        Some(focus) if !focus.trim().is_empty() => {
            format!("{}::{}", validated_url, focus.trim())
        }
        _ => validated_url.clone(),
    };
    let hash = compute_hash(&cache_key_input);
    let prefix = LocalFileStorage::get_image_prefix(&hash);
    let meta_key = LocalFileStorage::get_meta_key(&prefix);
    if let Ok(Some(meta_bytes)) = storage.get(&meta_key).await {
        if let Ok(metadata) = serde_json::from_slice::<ImageCacheMetadata>(&meta_bytes) {
            let width = metadata.width.unwrap_or_default();
            let height = metadata.height.unwrap_or_default();
            let info = ImageInfo {
                width,
                height,
                total_pixels: (width as u64).saturating_mul(height as u64),
                mime_type: metadata.mime_type.clone(),
                size: metadata.size.unwrap_or_default(),
                aspect_ratio: metadata.aspect_ratio,
            };
            let info_json = serde_json::to_string(&info).map_err(|err| {
                McpError::internal_error(
                    "serialize image info failed",
                    Some(serde_json::Value::String(err.to_string())),
                )
            })?;
            let response = ToolResponse {
                url: metadata.original_url,
                name: metadata.name,
                mime_type: metadata.mime_type,
                text: format!("{}\n\n图像信息: {}", metadata.description, info_json),
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

    let mut title = "Fetched Image".to_string();
    let mut description = "请分析图片内容。".to_string();
    let mut name = "fetched-image".to_string();

    if let Ok(api_key) = std::env::var("MODELSCOPE_API_KEY") {
        if !api_key.trim().is_empty() {
            if let Ok((desc_name, desc_text)) = modelscope::describe_image_with_qwen(
                &validated_url,
                &api_key,
                request.focus.as_deref(),
            )
            .await
            {
                if !desc_name.trim().is_empty() {
                    name = desc_name.trim().to_string();
                    title = name.clone();
                }
                if !desc_text.trim().is_empty() {
                    description = desc_text.trim().to_string();
                }
            }
        }
    }

    let (width, height, aspect_ratio) = image_processing::get_dimensions(bytes.as_ref(), &mime_type)
        .map(|(width, height)| {
            let aspect_ratio = if height == 0 {
                None
            } else {
                Some(width as f64 / height as f64)
            };
            (width, height, aspect_ratio)
        })
        .unwrap_or((0, 0, None));
    let info = ImageInfo {
        width,
        height,
        total_pixels: (width as u64).saturating_mul(height as u64),
        mime_type: mime_type.clone(),
        size: bytes.len(),
        aspect_ratio,
    };
    let info_json = serde_json::to_string(&info).map_err(|err| {
        McpError::internal_error(
            "serialize image info failed",
            Some(serde_json::Value::String(err.to_string())),
        )
    })?;
    let metadata = ImageCacheMetadata {
        original_url: validated_url.clone(),
        mime_type: mime_type.clone(),
        name: name.clone(),
        title: title.clone(),
        description: description.clone(),
        created_at: Utc::now().to_rfc3339(),
        width: if width == 0 { None } else { Some(width) },
        height: if height == 0 { None } else { Some(height) },
        size: Some(bytes.len()),
        aspect_ratio,
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
        url: validated_url,
        name,
        mime_type,
        text: format!("{}\n\n图像信息: {}", description, info_json),
    };
    let json = serde_json::to_string(&response).map_err(|err| {
        McpError::internal_error(
            "serialize tool response failed",
            Some(serde_json::Value::String(err.to_string())),
        )
    })?;
    Ok(CallToolResult::success(vec![Content::text(json)]))
}
