use anyhow::{anyhow, Result};
use image::{DynamicImage, ImageFormat, RgbaImage, GenericImageView};

const BYTES_PER_PIXEL: usize = 4;

pub fn rotate_pixels(pixels: &[u8], width: u32, height: u32, angle: i32) -> Vec<u8> {
    let expected_len = width
        .saturating_mul(height)
        .saturating_mul(BYTES_PER_PIXEL as u32) as usize;

    if pixels.len() != expected_len {
        return Vec::new();
    }

    let (new_width, new_height) = match angle {
        90 | -90 => (height, width),
        180 => (width, height),
        _ => (width, height),
    };

    let mut output = vec![0u8; (new_width * new_height * BYTES_PER_PIXEL as u32) as usize];

    for y in 0..height {
        for x in 0..width {
            let (new_x, new_y) = match angle {
                90 => (height - 1 - y, x),
                -90 => (y, width - 1 - x),
                180 => (width - 1 - x, height - 1 - y),
                _ => (x, y),
            };

            let src_index = ((y * width + x) * BYTES_PER_PIXEL as u32) as usize;
            let dst_index = ((new_y * new_width + new_x) * BYTES_PER_PIXEL as u32) as usize;

            output[dst_index..dst_index + BYTES_PER_PIXEL]
                .copy_from_slice(&pixels[src_index..src_index + BYTES_PER_PIXEL]);
        }
    }

    output
}

pub fn get_rotated_dimensions(width: u32, height: u32, angle: i32) -> Vec<u32> {
    let (new_width, new_height) = match angle {
        90 | -90 => (height, width),
        180 => (width, height),
        _ => (width, height),
    };

    vec![new_width, new_height]
}

fn normalize_bounds(left: f32, top: f32, right: f32, bottom: f32) -> (f32, f32, f32, f32) {
    (
        left.clamp(0.0, 1.0),
        top.clamp(0.0, 1.0),
        right.clamp(0.0, 1.0),
        bottom.clamp(0.0, 1.0),
    )
}

pub fn crop_pixels(
    pixels: &[u8],
    width: u32,
    height: u32,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
) -> Vec<u8> {
    let expected_len = width
        .saturating_mul(height)
        .saturating_mul(BYTES_PER_PIXEL as u32) as usize;

    if pixels.len() != expected_len {
        return Vec::new();
    }

    let (left, top, right, bottom) = normalize_bounds(left, top, right, bottom);
    if left >= right || top >= bottom {
        return Vec::new();
    }

    let start_x = (width as f32 * left).round() as u32;
    let start_y = (height as f32 * top).round() as u32;
    let end_x = (width as f32 * right).round() as u32;
    let end_y = (height as f32 * bottom).round() as u32;

    let new_width = end_x.saturating_sub(start_x);
    let new_height = end_y.saturating_sub(start_y);
    if new_width == 0 || new_height == 0 {
        return Vec::new();
    }

    let mut output = vec![0u8; (new_width * new_height * BYTES_PER_PIXEL as u32) as usize];

    for y in 0..new_height {
        for x in 0..new_width {
            let src_x = start_x + x;
            let src_y = start_y + y;

            let src_index = ((src_y * width + src_x) * BYTES_PER_PIXEL as u32) as usize;
            let dst_index = ((y * new_width + x) * BYTES_PER_PIXEL as u32) as usize;

            output[dst_index..dst_index + BYTES_PER_PIXEL]
                .copy_from_slice(&pixels[src_index..src_index + BYTES_PER_PIXEL]);
        }
    }

    output
}

pub fn get_cropped_dimensions(
    width: u32,
    height: u32,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
) -> Vec<u32> {
    let (left, top, right, bottom) = normalize_bounds(left, top, right, bottom);
    if left >= right || top >= bottom {
        return vec![0, 0];
    }

    let start_x = (width as f32 * left).round() as u32;
    let start_y = (height as f32 * top).round() as u32;
    let end_x = (width as f32 * right).round() as u32;
    let end_y = (height as f32 * bottom).round() as u32;

    let new_width = end_x.saturating_sub(start_x);
    let new_height = end_y.saturating_sub(start_y);

    vec![new_width, new_height]
}

pub fn decode_image(bytes: &[u8], mime_type: &str) -> Result<(Vec<u8>, u32, u32)> {
    let format = mime_to_format(mime_type)?;
    let image = image::load_from_memory_with_format(bytes, format)
        .map_err(|err| anyhow!("decode image failed: {err}"))?
        .to_rgba8();
    let (width, height) = image.dimensions();
    Ok((image.into_raw(), width, height))
}

pub fn encode_png(pixels: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let rgba = RgbaImage::from_raw(width, height, pixels.to_vec())
        .ok_or_else(|| anyhow!("invalid rgba buffer"))?;
    let mut output = Vec::new();
    DynamicImage::ImageRgba8(rgba)
        .write_to(&mut std::io::Cursor::new(&mut output), ImageFormat::Png)
        .map_err(|err| anyhow!("encode png failed: {err}"))?;
    Ok(output)
}

pub fn get_dimensions(bytes: &[u8], mime_type: &str) -> Result<(u32, u32)> {
    let format = mime_to_format(mime_type)?;
    let image = image::load_from_memory_with_format(bytes, format)
        .map_err(|err| anyhow!("decode image failed: {err}"))?;
    Ok(image.dimensions())
}

pub fn detect_mime_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some("image/png");
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("image/jpeg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    if bytes.starts_with(b"BM") {
        return Some("image/bmp");
    }
    None
}

pub fn mime_to_format(mime_type: &str) -> Result<ImageFormat> {
    match mime_type {
        "image/png" => Ok(ImageFormat::Png),
        "image/jpeg" | "image/jpg" => Ok(ImageFormat::Jpeg),
        "image/gif" => Ok(ImageFormat::Gif),
        "image/webp" => Ok(ImageFormat::WebP),
        "image/bmp" => Ok(ImageFormat::Bmp),
        _ => Err(anyhow!("unsupported mime type: {mime_type}")),
    }
}
