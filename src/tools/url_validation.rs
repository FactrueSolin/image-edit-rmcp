use rmcp::ErrorData as McpError;
use serde_json::Value;
use url::Url;

pub fn validate_http_url(raw: &str) -> Result<Url, McpError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(McpError::invalid_params("url不能为空", None));
    }
    let parsed = Url::parse(trimmed).map_err(|err| {
        McpError::invalid_params(
            "URL格式无效",
            Some(Value::String(err.to_string())),
        )
    })?;
    match parsed.scheme() {
        "http" | "https" => Ok(parsed),
        scheme => Err(McpError::invalid_params(
            "仅允许http或https协议",
            Some(Value::String(format!("当前协议: {scheme}"))),
        )),
    }
}
