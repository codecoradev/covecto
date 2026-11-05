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

    /// An Option's value does not parse.
    #[error("Invalid value '{value}' for option '{option}': expected {expected}")]
    InvalidOption {
        option: &'static str,
        value: String,
        expected: String,
    },

    /// An unknown Option, Profile or Preset name.
    #[error("Unknown {what} '{name}'. Valid: {}", valid.join(", "))]
    UnknownName {
        /// `"option"`, `"profile"` or `"preset"`.
        what: &'static str,
        name: String,
        valid: Vec<&'static str>,
    },

    /// The same Option was supplied more than once.
    #[error("Option '{0}' was supplied more than once")]
    DuplicateOption(&'static str),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// The image exceeds a size or memory limit (ours or the decoder's), as
    /// opposed to being corrupt.
    pub fn is_too_large(&self) -> bool {
        matches!(
            self,
            Error::ImageTooLarge { .. } | Error::ImageDecode(image::ImageError::Limits(_))
        )
    }

    /// The input is not in an image format covecto can decode (unknown or
    /// unsupported format), as opposed to a recognized but corrupt image.
    pub fn is_unsupported_format(&self) -> bool {
        matches!(
            self,
            Error::UnsupportedFormat(_) | Error::ImageDecode(image::ImageError::Unsupported(_))
        )
    }
}
