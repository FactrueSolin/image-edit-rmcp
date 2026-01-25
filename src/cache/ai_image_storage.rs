use std::path::PathBuf;

use anyhow::Result;
use tokio::fs;

use crate::cache::AiImageRecord;

const AI_IMAGE_DIR: &str = "ai_images";

pub async fn save_ai_image_record(storage: &crate::cache::LocalFileStorage, record: &AiImageRecord) -> Result<()> {
    let created_at = record.created_at.replace(':', "-");
    let hash_source = format!("{}:{}:{}", record.image_type, record.image_url, record.prompt);
    let hash = crate::cache::compute_hash(&hash_source);
    let file_key = format!("{AI_IMAGE_DIR}/{created_at}_{hash}.json");
    let payload = serde_json::to_vec_pretty(record)?;
    storage.put(&file_key, &payload).await?;
    Ok(())
}

pub async fn list_ai_image_records(
    storage: &crate::cache::LocalFileStorage,
    limit: usize,
    image_type: &str,
) -> Result<Vec<AiImageRecord>> {
    let dir_path = storage.resolve_path(AI_IMAGE_DIR);
    let mut entries: Vec<PathBuf> = Vec::new();
    let mut dir = match fs::read_dir(&dir_path).await {
        Ok(dir) => dir,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err.into()),
    };
    while let Some(entry) = dir.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            entries.push(path);
        }
    }
    entries.sort_by(|a, b| b.cmp(a));

    let mut records = Vec::new();
    for path in entries.into_iter() {
        if records.len() >= limit {
            break;
        }
        let bytes = fs::read(&path).await?;
        if let Ok(record) = serde_json::from_slice::<AiImageRecord>(&bytes) {
            if image_type != "all" && record.image_type != image_type {
                continue;
            }
            records.push(record);
        }
    }
    Ok(records)
}
