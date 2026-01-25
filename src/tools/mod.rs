pub mod crop_image;
pub mod edit_image;
pub mod fetch_image;
pub mod generate_image;
pub mod locate_object;
pub mod ocr_extract;
pub mod rotate_image;
pub mod url_validation;
// pub mod list_ai_images;

use serde::Serialize;

#[derive(Serialize)]
pub struct ToolResponse {
    pub url: String,
    pub name: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    pub text: String,
}

pub use crop_image::{crop_image, CropImageRequest};
pub use edit_image::{edit_image, EditImageRequest};
pub use fetch_image::{fetch_image, FetchImageRequest};
pub use generate_image::{generate_image, GenerateImageRequest};
pub use locate_object::{locate_object, LocateObjectRequest};
pub use ocr_extract::{ocr_extract, OcrExtractRequest};
pub use rotate_image::{rotate_image, RotateImageRequest, RotateDirection};
pub use url_validation::validate_http_url;
// pub use list_ai_images::{list_ai_images, ListAiImagesRequest};
