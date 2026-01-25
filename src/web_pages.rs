use std::sync::Arc;

use axum::{
    Json,
    extract::{Multipart, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use chrono::Utc;
use serde::Serialize;

use crate::cache::{LocalFileStorage, compute_hash, get_extension_from_mime_type};

const SECRET_HTML: &str = include_str!("../templates/secret.html");
const UPLOAD_HTML: &str = include_str!("../templates/upload.html");

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Serialize)]
struct UploadResponse {
    url: String,
    key: String,
}

fn json_error(status: StatusCode, message: &str) -> Response {
    (status, Json(ErrorResponse { error: message.to_string() })).into_response()
}

fn extension_from_filename(file_name: &str) -> Option<String> {
    if let Some((_, ext)) = file_name.rsplit_once('.') {
        let trimmed = ext.trim();
        if !trimmed.is_empty() && trimmed != file_name {
            return Some(trimmed.to_lowercase());
        }
    }
    None
}

fn resolve_extension(file_name: &str, content_type: Option<&str>) -> String {
    if let Some(ext) = extension_from_filename(file_name) {
        return ext;
    }
    if let Some(content_type) = content_type {
        let ext = get_extension_from_mime_type(content_type);
        if ext != "bin" {
            return ext.to_string();
        }
    }
    "bin".to_string()
}

pub async fn secret_page() -> Html<&'static str> {
    Html(SECRET_HTML)
}

pub async fn upload_page() -> Html<&'static str> {
    Html(UPLOAD_HTML)
}

pub async fn handle_image_upload(
    State(storage): State<Arc<LocalFileStorage>>,
    mut multipart: Multipart,
) -> Response {
    let mut file_name = None;
    let mut content_type = None;
    let mut bytes = None;
    loop {
        match multipart.next_field().await {
            Ok(Some(field)) => {
                if field.name() == Some("file") {
                    content_type = field.content_type().map(|value| value.to_string());
                    file_name = Some(field.file_name().unwrap_or("").to_string());
                    match field.bytes().await {
                        Ok(data) => {
                            bytes = Some(data);
                        }
                        Err(err) => {
                            return json_error(
                                StatusCode::BAD_REQUEST,
                                &format!("读取文件失败: {err}"),
                            );
                        }
                    }
                    break;
                }
            }
            Ok(None) => break,
            Err(err) => {
                return json_error(
                    StatusCode::BAD_REQUEST,
                    &format!("读取表单失败: {err}"),
                );
            }
        }
    }

    let content_type = match content_type {
        Some(value) => Some(value),
        None => None,
    };
    if let Some(content_type) = content_type.as_deref() {
        if !content_type.starts_with("image/") {
            return json_error(StatusCode::BAD_REQUEST, "文件类型不支持");
        }
    }

    let file_name = match file_name {
        Some(value) => value,
        None => return json_error(StatusCode::BAD_REQUEST, "未找到上传文件"),
    };
    let bytes = match bytes {
        Some(data) => data,
        None => return json_error(StatusCode::BAD_REQUEST, "未找到上传文件"),
    };
    if bytes.is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "上传文件为空");
    }

    let ext = resolve_extension(&file_name, content_type.as_deref());
    let timestamp = Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let cache_key_input = format!("upload:{timestamp}:{file_name}:{}", bytes.len());
    let hash = compute_hash(&cache_key_input);
    let key = format!("uploads/{hash}.{ext}");
    if let Err(err) = storage.put(&key, bytes.as_ref()).await {
        return json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("保存文件失败: {err}"),
        );
    }

    let url = storage.get_public_url(&key);
    (StatusCode::OK, Json(UploadResponse { url, key })).into_response()
}
