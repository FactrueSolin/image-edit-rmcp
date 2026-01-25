pub mod storage;
pub mod metadata;
pub mod hash;
pub mod ai_image_storage;

pub use storage::LocalFileStorage;
pub use metadata::*;
pub use hash::compute_hash;
pub use ai_image_storage::{save_ai_image_record, list_ai_image_records};

pub fn get_extension_from_mime_type(mime_type: &str) -> &str {
    match mime_type.to_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/bmp" => "bmp",
        "image/svg+xml" => "svg",
        "image/avif" => "avif",
        _ => "bin",
    }
}
