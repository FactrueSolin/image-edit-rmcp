use anyhow::Result;
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content},
    schemars::JsonSchema,
};
use serde::Deserialize;
use chrono::Utc;
use crate::{
    cache::{AiImageRecord, LocalFileStorage, save_ai_image_record},
    modelscope::{self, GenerateImageOptions},
    tools::ToolResponse,
};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GenerateImageRequest {
    #[schemars(description = "图片描述")]
    pub prompt: String,
    #[schemars(description = "不希望出现的内容")]
    pub negative_prompt: Option<String>,
    #[schemars(description = "图片比例，可选值：1:1、16:9、9:16、4:3、3:4、3:2、2:3。默认 1:1")]
    pub aspect_ratio: Option<String>,
    #[schemars(description = "图片分辨率，可选值：1k、2k、4k。默认 1k")]
    pub resolution: Option<String>,
    #[schemars(description = "采样步数")]
    pub steps: Option<u32>,
}

pub async fn generate_image(
    _storage: &LocalFileStorage,
    Parameters(request): Parameters<GenerateImageRequest>,
) -> Result<CallToolResult, McpError> {
    let aspect_ratio = request
        .aspect_ratio
        .as_deref()
        .unwrap_or("1:1")
        .trim();
    let resolution = request
        .resolution
        .as_deref()
        .unwrap_or("1k")
        .trim();
    // Z-Image-Turbo 模型的分辨率范围是 [512x512, 2048x2048]
    // 需要确保宽度和高度都不超过最大限制
    const MAX_DIMENSION: f64 = 2048.0;
    
    let base: f64 = match resolution.to_ascii_lowercase().as_str() {
        "1k" => 1024.0,
        "2k" => 2048.0,
        "4k" => 2048.0, // Z-Image-Turbo 最大支持 2048，4k 降级为 2k
        _ => {
            return Err(McpError::invalid_params(
                "resolution 仅支持 1k、2k、4k",
                None,
            ))
        }
    };
    let (ratio_w, ratio_h): (f64, f64) = match aspect_ratio {
        "1:1" => (1.0, 1.0),
        "16:9" => (16.0, 9.0),
        "9:16" => (9.0, 16.0),
        "4:3" => (4.0, 3.0),
        "3:4" => (3.0, 4.0),
        "3:2" => (3.0, 2.0),
        "2:3" => (2.0, 3.0),
        _ => {
            return Err(McpError::invalid_params(
                "aspect_ratio 仅支持 1:1、16:9、9:16、4:3、3:4、3:2、2:3",
                None,
            ))
        }
    };
    
    // 计算尺寸，确保不超过最大限制
    let (width, height) = {
        // 先按照 base 计算
        let scale = base / ratio_w.max(ratio_h);
        let mut w = (ratio_w * scale).round();
        let mut h = (ratio_h * scale).round();
        
        // 如果任一维度超过最大限制，按比例缩小
        if w > MAX_DIMENSION || h > MAX_DIMENSION {
            let scale_down = MAX_DIMENSION / w.max(h);
            w = (w * scale_down).round();
            h = (h * scale_down).round();
        }
        
        (w as u32, h as u32)
    };
    
    // 注意：API 文档示例使用 'x' 作为分隔符，如 "1024x1024"
    let size = format!("{}x{}", width, height);
    
    // 调试日志：打印计算出的尺寸
    eprintln!("[DEBUG] generate_image: aspect_ratio={}, resolution={}, calculated size={}", aspect_ratio, resolution, size);
    let api_key = std::env::var("MODELSCOPE_API_KEY")
        .map_err(|_| McpError::internal_error("missing MODELSCOPE_API_KEY", None))?;
    if api_key.trim().is_empty() {
        return Err(McpError::internal_error(
            "missing MODELSCOPE_API_KEY",
            None,
        ));
    }
    let prompt = request.prompt.clone();
    let negative_prompt = request.negative_prompt.clone();
    let aspect_ratio = request.aspect_ratio.clone();
    let resolution = request.resolution.clone();
    let steps = request.steps;
    let result = modelscope::generate_image_with_zturbo(
        GenerateImageOptions {
            prompt: prompt.clone(),
            negative_prompt: negative_prompt.clone(),
            size: Some(size),
            steps,
        },
        &api_key,
    )
    .await
    .map_err(|err| {
        McpError::internal_error(
            "generate image failed",
            Some(serde_json::Value::String(err.to_string())),
        )
    })?;

    let record = AiImageRecord {
        image_url: result.image_url.clone(),
        image_type: "generated".to_string(),
        prompt,
        negative_prompt,
        aspect_ratio,
        resolution,
        steps,
        source_image_url: None,
        created_at: Utc::now().to_rfc3339(),
    };
    let _ = save_ai_image_record(_storage, &record).await;

    let response = ToolResponse {
        url: result.image_url,
        name: "generated-image".to_string(),
        mime_type: "image/png".to_string(),
        text: "图像已生成。".to_string(),
    };
    let json = serde_json::to_string(&response).map_err(|err| {
        McpError::internal_error(
            "serialize tool response failed",
            Some(serde_json::Value::String(err.to_string())),
        )
    })?;
    Ok(CallToolResult::success(vec![Content::text(json)]))
}
