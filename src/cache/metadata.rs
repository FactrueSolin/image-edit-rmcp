use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ImageCacheMetadata {
    pub original_url: String,
    pub cached_image_key: String,
    pub cached_image_url: String,
    pub mime_type: String,
    pub name: String,
    pub title: String,
    pub description: String,
    pub created_at: String,
}

#[derive(Serialize, Deserialize)]
pub struct ProcessedImageCacheMetadata {
    pub cache_key_input: String,
    pub cached_image_key: String,
    pub cached_image_url: String,
    pub mime_type: String,
    pub created_at: String,
}

#[derive(Serialize, Deserialize)]
pub struct OcrCacheMetadata {
    pub cache_key_input: String,
    pub cached_image_key: String,
    pub cached_image_url: String,
    pub mime_type: String,
    pub cached_text_key: String,
    pub cached_text_url: String,
    pub created_at: String,
}

#[derive(Serialize, Deserialize)]
pub struct GeneratedImageCacheMetadata {
    pub cache_key_input: String,
    pub cached_image_key: String,
    pub cached_image_url: String,
    pub mime_type: String,
    pub created_at: String,
}

#[derive(Serialize, Deserialize)]
pub struct EditedImageCacheMetadata {
    pub cache_key_input: String,
    pub cached_image_key: String,
    pub cached_image_url: String,
    pub mime_type: String,
    pub created_at: String,
}

#[derive(Serialize, Deserialize)]
pub struct AiImageRecord {
    pub image_url: String,
    pub image_type: String,
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub aspect_ratio: Option<String>,
    pub resolution: Option<String>,
    pub steps: Option<u32>,
    pub source_image_url: Option<String>,
    pub created_at: String,
}
