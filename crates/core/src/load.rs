//! Loading images, with limits applied before any pixel data is decoded.
//!
//! One function, [`load_image`], for every caller (CLI, HTTP API, library):
//! the source is a file path or bytes, and [`ImageLimits`] rejects images
//! whose width or height is too large from the header alone, so a small,
//! highly compressible file cannot expand into a huge allocation.

use std::io::{BufRead, Seek};
use std::path::{Path, PathBuf};

use image::{ImageReader, RgbaImage};

use crate::{Error, Result};

/// Default per-side limit (8192 × 8192 ≈ 67 MP, 256 MiB as RGBA).
pub const DEFAULT_MAX_DIMENSION: u32 = 8192;

/// Limits checked before decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageLimits {
    /// Maximum width or height in pixels; `0` means no limit.
    ///
    /// This bounds, but does not make small, the work per image: at the
    /// default 8192 a 16-bit source decodes to up to 512 MiB and the 8-bit
    /// RGBA copy adds 256 MiB, before engine copies and output. The image
    /// crate's own allocation limit (512 MiB) applies as well.
    pub max_dimension: u32,
}

impl Default for ImageLimits {
    fn default() -> Self {
        Self {
            max_dimension: DEFAULT_MAX_DIMENSION,
        }
    }
}

impl ImageLimits {
    /// No dimension limit (the image crate's allocation limit still applies).
    pub const UNLIMITED: Self = Self { max_dimension: 0 };

    fn check(&self, width: u32, height: u32) -> Result<()> {
        let max = self.max_dimension;
        if max != 0 && (width > max || height > max) {
            return Err(Error::ImageTooLarge {
                width,
                height,
                max_w: max,
                max_h: max,
            });
        }
        Ok(())
    }
}

/// Where an image comes from.
#[derive(Debug, Clone, Copy)]
pub enum ImageSource<'a> {
    /// A file; its format is detected from the content, falling back to the
    /// extension.
    Path(&'a Path),
    /// Encoded bytes (e.g. an upload); format detected from the content.
    Bytes(&'a [u8]),
}

impl<'a> From<&'a Path> for ImageSource<'a> {
    fn from(p: &'a Path) -> Self {
        ImageSource::Path(p)
    }
}

impl<'a> From<&'a PathBuf> for ImageSource<'a> {
    fn from(p: &'a PathBuf) -> Self {
        ImageSource::Path(p)
    }
}

impl<'a> From<&'a [u8]> for ImageSource<'a> {
    fn from(b: &'a [u8]) -> Self {
        ImageSource::Bytes(b)
    }
}

impl<'a> From<&'a Vec<u8>> for ImageSource<'a> {
    fn from(b: &'a Vec<u8>) -> Self {
        ImageSource::Bytes(b)
    }
}

/// Decode an image into RGBA, rejecting it before decoding if it exceeds
/// `limits`.
///
/// # Errors
/// - [`Error::ImageTooLarge`] when a side exceeds `limits.max_dimension`.
/// - [`Error::ImageDecode`]: unknown format (see
///   [`Error::is_unsupported_format`]), corrupt data, or the decoder's own
///   memory limit (see [`Error::is_too_large`]).
/// - [`Error::Io`] when a file cannot be read.
pub fn load_image<'a>(
    source: impl Into<ImageSource<'a>>,
    limits: ImageLimits,
) -> Result<RgbaImage> {
    match source.into() {
        ImageSource::Path(path) => decode(
            || Ok(ImageReader::open(path)?.with_guessed_format()?),
            limits,
        ),
        ImageSource::Bytes(data) => decode(
            || Ok(ImageReader::new(std::io::Cursor::new(data)).with_guessed_format()?),
            limits,
        ),
    }
}

fn decode<R: BufRead + Seek>(
    mut reader: impl FnMut() -> Result<ImageReader<R>>,
    limits: ImageLimits,
) -> Result<RgbaImage> {
    let (width, height) = reader()?.into_dimensions()?;
    limits.check(width, height)?;
    Ok(reader()?.decode()?.into_rgba8())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, image::Rgba([1, 2, 3, 255]));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    fn limit(max_dimension: u32) -> ImageLimits {
        ImageLimits { max_dimension }
    }

    #[test]
    fn limit_rejects_before_decode() {
        let err = load_image(&png(300, 20), limit(256)).unwrap_err();
        assert!(matches!(
            err,
            Error::ImageTooLarge {
                width: 300,
                height: 20,
                ..
            }
        ));
        assert!(err.is_too_large());
    }

    #[test]
    fn limit_accepts_within_bounds_and_unlimited() {
        assert_eq!(
            load_image(&png(256, 256), limit(256)).unwrap().dimensions(),
            (256, 256)
        );
        assert_eq!(
            load_image(&png(300, 20), ImageLimits::UNLIMITED)
                .unwrap()
                .dimensions(),
            (300, 20)
        );
        assert_eq!(ImageLimits::default().max_dimension, DEFAULT_MAX_DIMENSION);
    }

    #[test]
    fn garbage_is_unsupported_format() {
        let err = load_image(b"not an image".as_slice(), ImageLimits::default()).unwrap_err();
        assert!(matches!(err, Error::ImageDecode(_)), "{err:?}");
        assert!(err.is_unsupported_format(), "{err:?}");
    }

    #[test]
    fn truncated_image_is_decode_error_not_unsupported() {
        let mut data = png(32, 32);
        data.truncate(data.len() / 2);
        let err = load_image(&data, ImageLimits::default()).unwrap_err();
        assert!(matches!(err, Error::ImageDecode(_)), "{err:?}");
        assert!(!err.is_unsupported_format(), "{err:?}");
    }

    #[test]
    fn path_and_bytes_agree_and_limits_apply_to_files() {
        let dir = std::env::temp_dir().join(format!(
            "covecto-load-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.png");
        let data = png(40, 30);
        std::fs::write(&file, &data).unwrap();

        let from_file = load_image(&file, ImageLimits::default()).unwrap();
        let from_bytes = load_image(&data, ImageLimits::default()).unwrap();
        assert_eq!(from_file, from_bytes);
        assert!(load_image(&file, limit(32)).unwrap_err().is_too_large());

        let missing = load_image(dir.join("missing.png").as_path(), ImageLimits::default());
        assert!(matches!(missing, Err(Error::Io(_))), "{missing:?}");
        std::fs::remove_dir_all(&dir).ok();
    }
}
