use anyhow::Result;
use rmcp::{
    ErrorData as McpError,
    handler::server::tool::Parameters,
    model::{CallToolResult, Content},
    schemars::JsonSchema,
};
use serde::Deserialize;

use crate::cache::{list_ai_image_records, LocalFileStorage};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListAiImagesRequest {
    #[schemars(description = "返回数量限制，默认 10")]
    pub limit: Option<u32>,
    #[schemars(description = "筛选类型：generated、edited 或 all（默认）")]
    pub image_type: Option<String>,
}

pub async fn list_ai_images(
    storage: &LocalFileStorage,
    Parameters(request): Parameters<ListAiImagesRequest>,
) -> Result<CallToolResult, McpError> {
    let limit = request.limit.unwrap_or(10).max(1) as usize;
    let image_type = request
        .image_type
        .as_deref()
        .unwrap_or("all")
        .trim();
    if image_type != "all" && image_type != "generated" && image_type != "edited" {
        return Err(McpError::invalid_params(
            "image_type 仅支持 generated、edited 或 all",
            None,
        ));
    }
    let records = list_ai_image_records(storage, limit, image_type)
        .await
        .map_err(|err| {
            McpError::internal_error(
                "list ai images failed",
                Some(serde_json::Value::String(err.to_string())),
            )
        })?;
    let json = serde_json::to_string(&records).map_err(|err| {
        McpError::internal_error(
            "serialize ai image records failed",
            Some(serde_json::Value::String(err.to_string())),
        )
    })?;
    Ok(CallToolResult::success(vec![Content::text(json)]))
}
