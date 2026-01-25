use std::env;
use std::path::PathBuf;

use anyhow::Result;

use image_edit_rmcp::{
    cache::LocalFileStorage,
    mcp_server::ImageEditorServer,
    web_pages,
};
use std::sync::Arc;
use axum::routing::get;
use axum::extract::DefaultBodyLimit;
use rmcp::transport::streamable_http_server::{
    StreamableHttpService, session::local::LocalSessionManager,
};
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let port = env::var("MCP_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(3000);
    let bind_address = format!("0.0.0.0:{}", port);

    let secret_key = env::var("SECRET_KEY").ok().filter(|value| !value.trim().is_empty());
    let mcp_path = match secret_key.as_deref() {
        Some(value) => format!("/{}/mcp", value),
        None => "/mcp".to_string(),
    };
    let upload_path = match secret_key.as_deref() {
        Some(value) => format!("/{}/upload", value),
        None => "/upload".to_string(),
    };

    let cache_dir = resolve_cache_dir();
    let cache_base_url = resolve_cache_base_url(&bind_address);
    let storage = Arc::new(LocalFileStorage::new(cache_dir.clone(), cache_base_url));
    let storage_for_service = storage.clone();
    let service = StreamableHttpService::new(
        move || Ok(ImageEditorServer::new(storage_for_service.clone())),
        LocalSessionManager::default().into(),
        Default::default(),
    );
    let router = axum::Router::new()
        .route("/secret", get(web_pages::secret_page))
        .route(
            &upload_path,
            get(web_pages::upload_page)
                .post(web_pages::handle_image_upload)
                .layer(DefaultBodyLimit::max(50 * 1024 * 1024)),
        )
        .nest_service(&mcp_path, service)
        .nest_service("/cache", ServeDir::new(cache_dir))
        .with_state(storage.clone());
    let tcp_listener = tokio::net::TcpListener::bind(&bind_address).await?;

    println!(
        "Image Edit MCP HTTP server started at http://{}{}",
        bind_address, mcp_path
    );

    let _ = axum::serve(tcp_listener, router)
        .with_graceful_shutdown(async { let _ = tokio::signal::ctrl_c().await; })
        .await;
    Ok(())
}

fn resolve_cache_dir() -> PathBuf {
    let cache_dir = env::var("CACHE_DIR")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from);
    if let Some(dir) = cache_dir {
        return dir;
    }
    let mut base = dirs::cache_dir().unwrap_or_else(|| PathBuf::from("."));
    base.push("image-edit-rmcp");
    base
}

fn resolve_cache_base_url(bind_address: &str) -> String {
    if let Ok(cache_url) = env::var("CACHE_URL") {
        let trimmed = cache_url.trim();
        if !trimmed.is_empty() {
            return format!("{}/cache", trimmed.trim_end_matches('/'));
        }
    }
    let raw_domain = env::var("DOMAIN")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| bind_address.to_string());
    let trimmed = raw_domain.trim();
    let mut base = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.trim_end_matches('/').to_string()
    } else {
        format!("http://{}", trimmed.trim_end_matches('/'))
    };
    while base.starts_with("http://http://") {
        base = base.replacen("http://http://", "http://", 1);
    }
    while base.starts_with("https://https://") {
        base = base.replacen("https://https://", "https://", 1);
    }
    while base.starts_with("http://https://") {
        base = base.replacen("http://https://", "https://", 1);
    }
    while base.starts_with("https://http://") {
        base = base.replacen("https://http://", "http://", 1);
    }
    format!("{base}/cache")
}
