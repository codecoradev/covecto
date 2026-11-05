/// Covecto error types.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Invalid engine: {0}")]
    InvalidEngine(String),

    #[error("Invalid config: {0}")]
    InvalidConfig(String),

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

    #[error("Conversion error: {0}")]
    Conversion(String),
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

/// Default per-side limit used by [`load_image_from_bytes_with_limit`] callers
/// that have no explicit configuration (8192 × 8192 ≈ 67 MP, 256 MiB RGBA).
pub const DEFAULT_MAX_DIMENSION: u32 = 8192;

/// Like [`load_image_from_bytes`], but rejects images whose width or height
/// exceeds `max_dimension` **before** decoding the pixel data, so a small,
/// highly compressible upload cannot expand past `max_dimension²` pixels.
///
/// This bounds, but does not make small, the work per image: at the default
/// 8192, a 16-bit source decodes to up to 512 MiB and the 8-bit RGBA copy
/// adds 256 MiB, before engine copies and output (~768 MiB peak per job).
/// Size `max_dimension` together with the job concurrency limit to fit the
/// memory available.
pub fn load_image_from_bytes_with_limit(
    data: &[u8],
    format_hint: Option<&str>,
    max_dimension: u32,
) -> Result<image::RgbaImage> {
    let reader = match format_hint {
        Some(fmt) => {
            let format = image::ImageFormat::from_extension(fmt)
                .ok_or_else(|| Error::UnsupportedFormat(fmt.to_string()))?;
            image::ImageReader::with_format(std::io::Cursor::new(data), format)
        }
        None => image::ImageReader::new(std::io::Cursor::new(data)).with_guessed_format()?,
    };
    let format = reader.format();
    let (width, height) = reader.into_dimensions()?;
    if width > max_dimension || height > max_dimension {
        return Err(Error::ImageTooLarge {
            width,
            height,
            max_w: max_dimension,
            max_h: max_dimension,
        });
    }

    let img = match format {
        Some(format) => image::load_from_memory_with_format(data, format)?,
        None => image::load_from_memory(data)?,
    };
    Ok(img.into_rgba8())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba([1, 2, 3, 255]));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn limit_rejects_before_decode() {
        let data = png(300, 20);
        let err = load_image_from_bytes_with_limit(&data, None, 256).unwrap_err();
        assert!(matches!(
            err,
            Error::ImageTooLarge {
                width: 300,
                height: 20,
                ..
            }
        ));
    }

    #[test]
    fn limit_accepts_within_bounds() {
        let data = png(256, 256);
        let img = load_image_from_bytes_with_limit(&data, Some("png"), 256).unwrap();
        assert_eq!(img.dimensions(), (256, 256));
    }

    #[test]
    fn limit_reports_garbage_as_decode_error() {
        let err = load_image_from_bytes_with_limit(b"not an image", None, 256).unwrap_err();
        assert!(matches!(err, Error::ImageDecode(_)), "{err:?}");
    }
}
