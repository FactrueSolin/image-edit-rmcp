use anyhow::Result;
use rmcp::{
    ErrorData as McpError,
    handler::server::tool::Parameters,
    model::{CallToolResult, Content},
    schemars::JsonSchema,
};
use serde::Deserialize;
use chrono::Utc;

use crate::{
    cache::{
        LocalFileStorage,
        OcrCacheMetadata,
        compute_hash,
        get_extension_from_mime_type,
    },
    image_processing,
    modelscope,
    tools::ToolResponse,
};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct OcrExtractRequest {
    #[schemars(description = "图像URL")]
    pub image_url: String,
}

pub async fn ocr_extract(
    storage: &LocalFileStorage,
    Parameters(request): Parameters<OcrExtractRequest>,
) -> Result<CallToolResult, McpError> {
    let cache_key_input = format!("ocr:{}", request.image_url);
    let hash = compute_hash(&cache_key_input);
    let prefix = format!("ocr/{hash}");
    let meta_key = LocalFileStorage::get_meta_key(&prefix);
    if let Ok(Some(meta_bytes)) = storage.get(&meta_key).await {
        if let Ok(metadata) = serde_json::from_slice::<OcrCacheMetadata>(&meta_bytes) {
            let text_key = metadata.cached_text_key.clone();
            let text = storage
                .get(&text_key)
                .await
                .ok()
                .flatten()
                .map(|bytes| String::from_utf8_lossy(&bytes).to_string());
            if let Some(text) = text {
                let response = ToolResponse {
                    url: metadata.cached_image_url,
                    name: "ocr-image".to_string(),
                    mime_type: metadata.mime_type,
                    text,
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
    }
    let api_key = std::env::var("MODELSCOPE_API_KEY")
        .map_err(|_| McpError::internal_error("missing MODELSCOPE_API_KEY", None))?;
    if api_key.trim().is_empty() {
        return Err(McpError::internal_error(
            "missing MODELSCOPE_API_KEY",
            None,
        ));
    }
    let response = reqwest::get(&request.image_url).await.map_err(|err| {
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

    let text = modelscope::extract_image_text_with_qwen(&request.image_url, &api_key)
        .await
        .map_err(|err| {
            McpError::internal_error(
                "ocr extract failed",
                Some(serde_json::Value::String(err.to_string())),
            )
        })?;

    let ext = get_extension_from_mime_type(&mime_type);
    let cached_image_key = LocalFileStorage::get_original_key(&prefix, ext);
    storage.put(&cached_image_key, bytes.as_ref()).await.map_err(|err| {
        McpError::internal_error(
            "cache image failed",
            Some(serde_json::Value::String(err.to_string())),
        )
    })?;
    let cached_image_url = storage.get_public_url(&cached_image_key);
    let text_key = format!("{prefix}/ocr.txt");
    storage.put(&text_key, text.as_bytes()).await.map_err(|err| {
        McpError::internal_error(
            "cache text failed",
            Some(serde_json::Value::String(err.to_string())),
        )
    })?;
    let cached_text_url = storage.get_public_url(&text_key);
    let metadata = OcrCacheMetadata {
        cache_key_input,
        cached_image_key,
        cached_image_url: cached_image_url.clone(),
        mime_type: mime_type.clone(),
        cached_text_key: text_key,
        cached_text_url: cached_text_url.clone(),
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
        name: "ocr-image".to_string(),
        mime_type,
        text,
    };
    let json = serde_json::to_string(&response).map_err(|err| {
        McpError::internal_error(
            "serialize tool response failed",
            Some(serde_json::Value::String(err.to_string())),
        )
    })?;
    Ok(CallToolResult::success(vec![Content::text(json)]))
}
