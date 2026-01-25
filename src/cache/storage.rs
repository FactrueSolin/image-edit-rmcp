use std::path::{Path, PathBuf};

use anyhow::Result;
use tokio::fs;

#[derive(Clone, Debug)]
pub struct LocalFileStorage {
    base_dir: PathBuf,
    base_url: String,
}

impl LocalFileStorage {
    pub fn new(base_dir: PathBuf, base_url: String) -> Self {
        Self { base_dir, base_url }
    }

    pub async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let path = self.resolve_path(key);
        match fs::read(&path).await {
            Ok(bytes) => Ok(Some(bytes)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err.into()),
        }
    }


    pub async fn put(&self, key: &str, data: &[u8]) -> Result<()> {
        let path = self.resolve_path(key);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(path, data).await?;
        Ok(())
    }

    pub async fn exists(&self, key: &str) -> Result<bool> {
        let path = self.resolve_path(key);
        match fs::metadata(path).await {
            Ok(_) => Ok(true),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(err) => Err(err.into()),
        }
    }

    pub fn get_public_url(&self, key: &str) -> String {
        let mut trimmed = self.base_url.trim_end_matches('/').to_string();
        while trimmed.starts_with("http://http://") {
            trimmed = trimmed.replacen("http://http://", "http://", 1);
        }
        while trimmed.starts_with("https://https://") {
            trimmed = trimmed.replacen("https://https://", "https://", 1);
        }
        while trimmed.starts_with("http://https://") {
            trimmed = trimmed.replacen("http://https://", "https://", 1);
        }
        while trimmed.starts_with("https://http://") {
            trimmed = trimmed.replacen("https://http://", "http://", 1);
        }
        let key = key.trim_start_matches('/');
        format!("{trimmed}/{key}")
    }

    pub fn get_image_prefix(hash: &str) -> String {
        format!("images/{hash}")
    }

    pub fn get_meta_key(prefix: &str) -> String {
        format!("{prefix}/meta.json")
    }

    pub fn get_original_key(prefix: &str, ext: &str) -> String {
        format!("{prefix}/original.{ext}")
    }

    pub fn get_result_key(prefix: &str, ext: &str) -> String {
        format!("{prefix}/result.{ext}")
    }

    pub fn resolve_path(&self, key: &str) -> PathBuf {
        let normalized = key.trim_start_matches('/');
        self.base_dir.join(Path::new(normalized))
    }
}
