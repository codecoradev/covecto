/// Covecto error types.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Invalid engine: {0}")]
    InvalidEngine(String),

    #[error("Image decode error: {0}")]
    ImageDecode(#[from] image::ImageError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("vtracer error: {0}")]
    Vtracer(String),

    #[error("SVG optimization error: {0}")]
    Optimization(String),

    #[error("Unsupported image format: {0}")]
    UnsupportedFormat(String),

    #[error("Image too large: {width}x{height} exceeds maximum {max_w}x{max_h}")]
    ImageTooLarge {
        width: u32,
        height: u32,
        max_w: u32,
        max_h: u32,
    },

    #[error("Invalid request: {0}")]
    BadRequest(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Convenience: read and decode an image file.
pub fn load_image(path: &std::path::Path) -> Result<image::RgbaImage> {
    let img = image::open(path)?;
    Ok(img.into_rgba8())
}

/// Convenience: load from bytes with optional format hint.
pub fn load_image_from_bytes(data: &[u8], format_hint: Option<&str>) -> Result<image::RgbaImage> {
    let img = if let Some(fmt) = format_hint {
        let format = image::ImageFormat::from_extension(fmt)
            .ok_or_else(|| Error::UnsupportedFormat(fmt.to_string()))?;
        image::load_from_memory_with_format(data, format)?
    } else {
        image::load_from_memory(data)?
    };
    Ok(img.into_rgba8())
}
