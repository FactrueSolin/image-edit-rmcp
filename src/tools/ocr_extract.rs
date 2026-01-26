use anyhow::Result;
use rmcp::{
    ErrorData as McpError,
    handler::server::tool::Parameters,
    model::{CallToolResult, Content},
    schemars::JsonSchema,
};
use serde::{Deserialize, Serialize};
use tokio::task::JoinSet;
use chrono::Utc;

use crate::{
    cache::{
        LocalFileStorage,
        OcrCacheMetadata,
        compute_hash,
    },
    image_processing,
    modelscope,
    tools::validate_http_url,
};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct OcrExtractRequest {
    #[schemars(description = "图像URL列表")]
    pub urls: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct OcrResult {
    pub image_url: String,
    pub text: String,
    pub cached_text_url: Option<String>,
}

pub async fn ocr_extract(
    storage: &LocalFileStorage,
    Parameters(request): Parameters<OcrExtractRequest>,
) -> Result<CallToolResult, McpError> {
    if request.urls.is_empty() {
        return Err(McpError::invalid_params("urls不能为空", None));
    }

    let total = request.urls.len();
    let mut join_set = JoinSet::new();

    for (index, url) in request.urls.into_iter().enumerate() {
        let storage = storage.clone();
        join_set.spawn(async move {
            let result = ocr_single_image(&storage, &url).await;
            (index, result)
        });
    }

    let mut results: Vec<Option<OcrResult>> = std::iter::repeat_with(|| None)
        .take(total)
        .collect();
    while let Some(task_result) = join_set.join_next().await {
        let (index, result) = task_result.map_err(|err| {
            McpError::internal_error(
                "ocr extract task failed",
                Some(serde_json::Value::String(err.to_string())),
            )
        })?;
        let response = result?;
        results[index] = Some(response);
    }

    let mut responses = Vec::with_capacity(total);
    for item in results {
        match item {
            Some(response) => responses.push(response),
            None => {
                return Err(McpError::internal_error(
                    "ocr extract task missing result",
                    None,
                ))
            }
        }
    }

    let json = serde_json::to_string(&responses).map_err(|err| {
        McpError::internal_error(
            "serialize ocr results failed",
            Some(serde_json::Value::String(err.to_string())),
        )
    })?;
    Ok(CallToolResult::success(vec![Content::text(json)]))
}

async fn ocr_single_image(
    storage: &LocalFileStorage,
    raw_url: &str,
) -> Result<OcrResult, McpError> {
    let validated_url = validate_http_url(raw_url)?;
    let validated_url = validated_url.to_string();
    let cache_key_input = format!("ocr:{}", validated_url);
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
                return Ok(OcrResult {
                    image_url: validated_url,
                    text,
                    cached_text_url: Some(metadata.cached_text_url),
                });
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
    let _mime_type = detected
        .or(mime_from_header)
        .ok_or_else(|| McpError::internal_error("unsupported image type", None))?;

    let text = modelscope::extract_image_text_with_qwen(&validated_url, &api_key)
        .await
        .map_err(|err| {
            McpError::internal_error(
                "ocr extract failed",
                Some(serde_json::Value::String(err.to_string())),
            )
        })?;

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

    Ok(OcrResult {
        image_url: validated_url,
        text,
        cached_text_url: Some(cached_text_url),
    })
}
