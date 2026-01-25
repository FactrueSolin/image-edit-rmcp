use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use tokio::time::{sleep, Duration, Instant};

const MODELSCOPE_API_ROOT: &str = "https://api-inference.modelscope.cn";
const MODELSCOPE_BASE_URL: &str = "https://api-inference.modelscope.cn/v1";
const MODELSCOPE_MODEL: &str = "Qwen/Qwen3-VL-8B-Instruct";
const OCR_PROMPT: &str = "请识别并提取图片中的所有文字内容，保持原有格式和结构。只输出识别到的文字，不要添加任何解释。";
const IMAGE_DESCRIPTION_PROMPT: &str = concat!(
    "请分析这张图片，并以JSON格式回复，包含以下字段：\n",
    "1. name: 图片的简短名称（不超过10个字，直接描述主体）\n",
    "2. description: 图片的详细描述（包括主要对象、场景、颜色、氛围等）\n\n",
    "请只返回JSON，不要包含其他文字。示例格式：\n",
    "{\"name\": \"校园动漫场景\", \"description\": \"这是一张...\"}"
);

const DEFAULT_POLL_INTERVAL_MS: u64 = 5_000;
const DEFAULT_TIMEOUT_MS: u64 = 5 * 60 * 1_000;
const Z_TURBO_MODEL: &str = "Tongyi-MAI/Z-Image-Turbo";
const QWEN_IMAGE_EDIT_MODEL: &str = "Qwen/Qwen-Image-Edit-2511";

fn build_image_description_prompt(focus: Option<&str>) -> String {
    match focus {
        Some(focus) if !focus.trim().is_empty() => {
            format!(
                "请分析这张图片，并以JSON格式回复，包含以下字段：\n\
1. name: 图片的简短名称（不超过10个字，直接描述主体）\n\
2. description: 图片的详细描述（包括主要对象、场景、颜色、氛围等）\n\n\
【特别关注】：{}\n\n\
请只返回JSON，不要包含其他文字。",
                focus
            )
        }
        _ => IMAGE_DESCRIPTION_PROMPT.to_string(),
    }
}

async fn assert_ok_response(response: reqwest::Response) -> Result<reqwest::Response> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    Err(anyhow!(
        "ModelScope 请求失败: {status} {text}"
    ))
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Option<Vec<ChatChoice>>,
    error: Option<ModelScopeError>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: Option<ChatMessage>,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ModelScopeError {
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ImageGenerationTaskResponse {
    task_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ImageGenerationStatusResponse {
    task_status: Option<String>,
    output_images: Option<Vec<String>>,
    error: Option<TaskError>,
}

#[derive(Debug, Deserialize)]
struct TaskError {
    code: Option<String>,
    message: Option<String>,
}

pub struct GenerateImageOptions {
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub size: Option<String>,
    pub steps: Option<u32>,
}

pub struct GenerateImageResult {
    pub image_url: String,
    pub task_id: String,
}

pub async fn extract_image_text_with_qwen(image_url: &str, api_key: &str) -> Result<String> {
    let client = Client::new();
    let response = client
        .post(format!("{MODELSCOPE_BASE_URL}/chat/completions"))
        .bearer_auth(api_key)
        .json(&json!({
            "model": MODELSCOPE_MODEL,
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {"type": "text", "text": OCR_PROMPT},
                        {"type": "image_url", "image_url": {"url": image_url}}
                    ]
                }
            ],
            "stream": false
        }))
        .send()
        .await?;

    let response = assert_ok_response(response).await?;
    let payload: ChatCompletionResponse = response.json().await?;
    if let Some(error) = payload.error.and_then(|err| err.message) {
        return Err(anyhow!("ModelScope 返回错误: {error}"));
    }
    let content = payload
        .choices
        .and_then(|choices| choices.into_iter().next())
        .and_then(|choice| choice.message)
        .and_then(|msg| msg.content)
        .ok_or_else(|| anyhow!("ModelScope 未返回 OCR 内容"))?;

    Ok(content.trim().to_string())
}

pub async fn describe_image_with_qwen(
    image_url: &str,
    api_key: &str,
    focus: Option<&str>,
) -> Result<(String, String)> {
    let client = Client::new();
    let prompt = build_image_description_prompt(focus);
    let response = client
        .post(format!("{MODELSCOPE_BASE_URL}/chat/completions"))
        .bearer_auth(api_key)
        .json(&json!({
            "model": MODELSCOPE_MODEL,
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {"type": "text", "text": prompt},
                        {"type": "image_url", "image_url": {"url": image_url}}
                    ]
                }
            ],
            "stream": false
        }))
        .send()
        .await?;

    let response = assert_ok_response(response).await?;
    let payload: ChatCompletionResponse = response.json().await?;
    if let Some(error) = payload.error.and_then(|err| err.message) {
        return Err(anyhow!("ModelScope 返回错误: {error}"));
    }
    let content = payload
        .choices
        .and_then(|choices| choices.into_iter().next())
        .and_then(|choice| choice.message)
        .and_then(|msg| msg.content)
        .ok_or_else(|| anyhow!("ModelScope 未返回图片描述内容"))?;

    let raw = content.trim();
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(raw) {
        let name = parsed.get("name").and_then(|value| value.as_str()).unwrap_or("");
        let description = parsed
            .get("description")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        if !name.is_empty() && !description.is_empty() {
            return Ok((name.trim().to_string(), description.trim().to_string()));
        }
    }

    Ok(("fetched-image".to_string(), raw.to_string()))
}

pub async fn generate_image_with_zturbo(
    options: GenerateImageOptions,
    api_key: &str,
) -> Result<GenerateImageResult> {
    let client = Client::new();
    
    // 构建请求体，只包含非空字段
    let mut body = json!({
        "model": Z_TURBO_MODEL,
        "prompt": options.prompt,
    });
    
    if let Some(ref neg) = options.negative_prompt {
        if !neg.trim().is_empty() {
            body["negative_prompt"] = json!(neg);
        }
    }
    if let Some(ref size) = options.size {
        body["size"] = json!(size);
    }
    if let Some(steps) = options.steps {
        body["steps"] = json!(steps);
    }
    
    // 调试日志：打印请求体
    eprintln!("[DEBUG] generate_image_with_zturbo request body: {}", serde_json::to_string_pretty(&body).unwrap_or_default());
    
    let response = client
        .post(format!("{MODELSCOPE_API_ROOT}/v1/images/generations"))
        .bearer_auth(api_key)
        .header("X-ModelScope-Async-Mode", "true")
        .json(&body)
        .send()
        .await?;

    // 调试日志：打印响应状态
    eprintln!("[DEBUG] generate_image_with_zturbo response status: {}", response.status());
    
    let response = assert_ok_response(response).await?;
    let response_text = response.text().await?;
    
    // 调试日志：打印响应内容
    eprintln!("[DEBUG] generate_image_with_zturbo response body: {}", response_text);
    
    let payload: ImageGenerationTaskResponse = serde_json::from_str(&response_text)
        .map_err(|e| anyhow!("解析任务响应失败: {}, 原始响应: {}", e, response_text))?;
    let task_id = payload
        .task_id
        .ok_or_else(|| anyhow!("ModelScope 未返回 task_id"))?;

    let (image_url, _) = poll_generation_task(&client, api_key, &task_id).await?;
    Ok(GenerateImageResult { image_url, task_id })
}

pub async fn edit_image_with_qwen(
    image_url: &str,
    prompt: &str,
    size: Option<&str>,
    steps: Option<u32>,
    api_key: &str,
) -> Result<String> {
    let client = Client::new();
    let response = client
        .post(format!("{MODELSCOPE_API_ROOT}/v1/images/generations"))
        .bearer_auth(api_key)
        .header("X-ModelScope-Async-Mode", "true")
        .json(&json!({
            "model": QWEN_IMAGE_EDIT_MODEL,
            "image_url": [image_url],
            "prompt": prompt,
            "size": size,
            "steps": steps,
        }))
        .send()
        .await?;

    let response = assert_ok_response(response).await?;
    let payload: ImageGenerationTaskResponse = response.json().await?;
    let task_id = payload
        .task_id
        .ok_or_else(|| anyhow!("ModelScope 未返回 task_id"))?;
    let (image_url, _) = poll_generation_task(&client, api_key, &task_id).await?;
    Ok(image_url)
}

async fn poll_generation_task(
    client: &Client,
    api_key: &str,
    task_id: &str,
) -> Result<(String, String)> {
    let deadline = Instant::now() + Duration::from_millis(DEFAULT_TIMEOUT_MS);
    let mut poll_count = 0u32;
    
    eprintln!("[DEBUG] poll_generation_task: starting poll for task_id={}", task_id);
    
    while Instant::now() <= deadline {
        poll_count += 1;
        let response = client
            .get(format!("{MODELSCOPE_API_ROOT}/v1/tasks/{task_id}"))
            .bearer_auth(api_key)
            .header("X-ModelScope-Task-Type", "image_generation")
            .send()
            .await?;

        let response = assert_ok_response(response).await?;
        let response_text = response.text().await?;
        
        // 调试日志：打印轮询响应
        eprintln!("[DEBUG] poll_generation_task: poll_count={}, response={}", poll_count, response_text);
        
        let payload: ImageGenerationStatusResponse = serde_json::from_str(&response_text)
            .map_err(|e| anyhow!("解析任务状态响应失败: {}, 原始响应: {}", e, response_text))?;
        let status = payload
            .task_status
            .ok_or_else(|| anyhow!("ModelScope 未返回任务状态, 原始响应: {}", response_text))?;
        
        eprintln!("[DEBUG] poll_generation_task: task_status={}", status);
        
        match status.as_str() {
            "SUCCEED" => {
                let image_url = payload
                    .output_images
                    .and_then(|images| images.into_iter().next())
                    .ok_or_else(|| anyhow!("ModelScope 未返回图片地址"))?;
                eprintln!("[DEBUG] poll_generation_task: success, image_url={}", image_url);
                return Ok((image_url, task_id.to_string()));
            }
            "FAILED" => {
                // 提取详细错误信息
                let error_msg = payload
                    .error
                    .map(|e| {
                        format!(
                            "code={}, message={}",
                            e.code.unwrap_or_default(),
                            e.message.unwrap_or_default()
                        )
                    })
                    .unwrap_or_else(|| "未知错误".to_string());
                eprintln!("[DEBUG] poll_generation_task: FAILED, error={}", error_msg);
                return Err(anyhow!("ModelScope 图片生成失败: {}", error_msg));
            }
            _ => {
                eprintln!("[DEBUG] poll_generation_task: status={}, waiting...", status);
                sleep(Duration::from_millis(DEFAULT_POLL_INTERVAL_MS)).await;
            }
        }
    }

    Err(anyhow!(
        "ModelScope 图片生成超时 (task_id={task_id}, poll_count={poll_count})"
    ))
}
