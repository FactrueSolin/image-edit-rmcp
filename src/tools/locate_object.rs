use rmcp::{
    ErrorData as McpError,
    handler::server::tool::Parameters,
    model::{CallToolResult, Content},
    schemars::JsonSchema,
};
use serde::Deserialize;

use crate::{modelscope, tools::validate_http_url};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LocateObjectRequest {
    #[schemars(description = "图像URL")]
    pub image_url: String,
    #[schemars(description = "需要定位的物体名称")]
    pub object_name: String,
}

pub async fn locate_object(
    Parameters(request): Parameters<LocateObjectRequest>,
) -> Result<CallToolResult, McpError> {
    let validated_url = validate_http_url(&request.image_url)?;
    let validated_url = validated_url.to_string();
    let api_key = std::env::var("MODELSCOPE_API_KEY")
        .map_err(|_| McpError::internal_error("missing MODELSCOPE_API_KEY", None))?;
    if api_key.trim().is_empty() {
        return Err(McpError::internal_error(
            "missing MODELSCOPE_API_KEY",
            None,
        ));
    }
    let boxes = modelscope::locate_object_with_qwen(
        &validated_url,
        &request.object_name,
        &api_key,
    )
    .await
    .map_err(|err| {
        McpError::internal_error(
            "locate object failed",
            Some(serde_json::Value::String(err.to_string())),
        )
    })?;

    let mut lines = Vec::new();
    lines.push(format!("定位目标：{}", request.object_name));
    lines.push(format!("共找到 {} 个目标。", boxes.len()));
    for (index, bbox) in boxes.iter().enumerate() {
        lines.push(format!(
            "{}. x1={:.2}, y1={:.2}, x2={:.2}, y2={:.2}",
            index + 1,
            bbox.x1,
            bbox.y1,
            bbox.x2,
            bbox.y2
        ));
    }

    Ok(CallToolResult::success(vec![Content::text(
        lines.join("\n"),
    )]))
}
