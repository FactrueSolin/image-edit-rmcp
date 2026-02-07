use crate::{
    cache::{AiImageRecord, LocalFileStorage, save_ai_image_record},
    modelscope,
    tools::{ToolResponse, validate_http_url},
};
use anyhow::Result;
use chrono::Utc;
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content},
    schemars::JsonSchema,
};
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct EditImageRequest {
    #[schemars(description = "待编辑图片URL")]
    pub image_url: String,
    #[schemars(description = "编辑指令")]
    pub prompt: String,
    #[schemars(description = "输出图片尺寸")]
    pub size: Option<String>,
    #[schemars(description = "采样步数")]
    pub steps: Option<u32>,
}

pub async fn edit_image(
    _storage: &LocalFileStorage,
    Parameters(request): Parameters<EditImageRequest>,
) -> Result<CallToolResult, McpError> {
    let validated_url = validate_http_url(&request.image_url)?;
    let validated_url = validated_url.to_string();
    let api_key = std::env::var("MODELSCOPE_API_KEY")
        .map_err(|_| McpError::internal_error("missing MODELSCOPE_API_KEY", None))?;
    if api_key.trim().is_empty() {
        return Err(McpError::internal_error("missing MODELSCOPE_API_KEY", None));
    }
    let prompt = request.prompt.clone();
    let source_image_url = validated_url.clone();
    let size = request.size.clone();
    let steps = request.steps;
    let image_url = modelscope::edit_image_with_qwen(
        &validated_url,
        &request.prompt,
        request.size.as_deref(),
        request.steps,
        &api_key,
    )
    .await
    .map_err(|err| {
        McpError::internal_error(
            "edit image failed",
            Some(serde_json::Value::String(err.to_string())),
        )
    })?;

    let record = AiImageRecord {
        image_url: image_url.clone(),
        image_type: "edited".to_string(),
        prompt,
        negative_prompt: None,
        aspect_ratio: None,
        resolution: size,
        steps,
        source_image_url: Some(source_image_url),
        created_at: Utc::now().to_rfc3339(),
    };
    let _ = save_ai_image_record(_storage, &record).await;

    let response = ToolResponse {
        url: image_url,
        name: "edited-image".to_string(),
        mime_type: "image/png".to_string(),
        text: "图像已编辑。".to_string(),
    };
    let json = serde_json::to_string(&response).map_err(|err| {
        McpError::internal_error(
            "serialize tool response failed",
            Some(serde_json::Value::String(err.to_string())),
        )
    })?;
    Ok(CallToolResult::success(vec![Content::text(json)]))
}
