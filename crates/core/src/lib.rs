/// Covecto — Dual-engine image vectorization core.
///
/// Provides two vectorization engines:
/// - **PixelExact**: Flood-fill contiguous regions → boundary tracing → rectilinear SVG paths.
///   Best for icons, pixel art, screenshots. Zero information loss.
/// - **Spline** (vtracer-backed): Color quantization → contour tracing → Bézier spline fitting.
///   Best for photos, illustrations, artwork.
/// - **Auto**: Heuristic engine selection based on image characteristics.
mod config;
mod engine;
mod error;
mod optimize;

pub use config::{
    ColorMode, Engine, HierarchicalMode, OptimizeConfig, OptimizePreset, OutputFormat,
    PathSimplifyMode, SplinePreset, VectorizeConfig, VectorizeRequest,
};
pub use engine::{pixel_exact, spline};
pub use error::{Error, Result, load_image, load_image_from_bytes};
pub use optimize::optimize_svg;

use image::RgbaImage;
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Result of a vectorization operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorizeResult {
    /// The generated SVG string.
    pub svg: String,
    /// Which engine was used.
    pub engine_used: Engine,
    /// Metadata about the operation.
    pub metadata: ResultMetadata,
}

/// Metadata about a vectorization result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultMetadata {
    /// Input image dimensions (width, height).
    pub input_size: (u32, u32),
    /// Output SVG byte size.
    pub svg_byte_size: usize,
    /// Number of SVG path elements.
    pub path_count: usize,
    /// Processing time in milliseconds.
    pub processing_time_ms: u64,
    /// Compression ratio (svg_size / raw_pixel_bytes).
    pub compression_ratio: f64,
}

/// Select the best engine based on image characteristics.
fn auto_select_engine(img: &RgbaImage) -> Engine {
    let (w, h) = (img.width(), img.height());
    let total_pixels = (w * h) as f64;

    if total_pixels == 0.0 {
        return Engine::PixelExact;
    }

    // Count unique colors (sample-based for large images)
    let unique_colors = count_unique_colors(img);

    // Heuristic:
    // - Small images (<= 256x256) with few colors (<= 128) → PixelExact
    // - Very high unique color density → Spline (likely a photo)
    // - Otherwise → PixelExact (lossless, fast)
    let color_density = unique_colors as f64 / total_pixels;

    if w <= 256 && h <= 256 && unique_colors <= 128 {
        Engine::PixelExact
    } else if color_density > 0.005 {
        Engine::Spline
    } else {
        Engine::PixelExact
    }
}

/// Count unique RGBA colors in an image.
/// For large images, samples up to 65_536 pixels to keep this fast.
fn count_unique_colors(img: &RgbaImage) -> usize {
    use std::collections::HashSet;
    let raw = img.as_raw();
    let total = raw.len() / 4;
    let step = if total > 65_536 { total / 65_536 } else { 1 };

    let mut colors = HashSet::new();
    for i in (0..total).step_by(step) {
        let offset = i * 4;
        colors.insert([
            raw[offset],
            raw[offset + 1],
            raw[offset + 2],
            raw[offset + 3],
        ]);
    }
    colors.len()
}

/// Count `<path` occurrences in an SVG string (approximate path count).
fn count_svg_paths(svg: &str) -> usize {
    svg.matches("<path").count()
}

/// Main vectorization entry point.
pub fn vectorize(request: &VectorizeRequest) -> Result<VectorizeResult> {
    let engine = match request.config.engine {
        Engine::Auto => auto_select_engine(&request.image),
        other => other,
    };

    let start = Instant::now();
    let raw_svg = match engine {
        Engine::PixelExact => pixel_exact::vectorize(&request.image),
        Engine::Spline => spline::vectorize(&request.image, &request.config),
        Engine::Auto => unreachable!(),
    }?;

    // Post-processing: optimization
    let svg = if request.config.optimize {
        optimize_svg(&raw_svg, &request.config.optimize_config)?
    } else {
        raw_svg
    };

    let elapsed = start.elapsed();
    let (w, h) = (request.image.width(), request.image.height());
    let raw_pixel_bytes = (w as usize * h as usize) * 4;
    let svg_byte_size = svg.len();
    let compression_ratio = if raw_pixel_bytes > 0 {
        svg_byte_size as f64 / raw_pixel_bytes as f64
    } else {
        1.0
    };

    Ok(VectorizeResult {
        engine_used: engine,
        metadata: ResultMetadata {
            input_size: (w, h),
            svg_byte_size,
            path_count: count_svg_paths(&svg),
            processing_time_ms: elapsed.as_millis() as u64,
            compression_ratio,
        },
        svg,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_select_small_icon() {
        // 16x16 icon with 4 colors → PixelExact
        let img = RgbaImage::from_pixel(16, 16, image::Rgba([255, 0, 0, 255]));
        assert!(matches!(auto_select_engine(&img), Engine::PixelExact));
    }

    #[test]
    fn test_auto_select_large_photo() {
        // 512x512 with many colors (gradient) → Spline
        let img = RgbaImage::from_fn(512, 512, |x, y| {
            let r = (x * 255 / 512) as u8;
            let g = (y * 255 / 512) as u8;
            image::Rgba([r, g, 128, 255])
        });
        assert!(matches!(auto_select_engine(&img), Engine::Spline));
    }

    #[test]
    fn test_vectorize_pixel_exact() {
        let img = RgbaImage::from_pixel(8, 8, image::Rgba([255, 0, 0, 255]));
        let req = VectorizeRequest::new(img).with_engine(Engine::PixelExact);
        let result = vectorize(&req).unwrap();
        assert!(result.svg.contains("<svg"));
        assert!(result.svg.contains("<path"));
        assert_eq!(result.engine_used, Engine::PixelExact);
    }

    #[test]
    fn test_vectorize_spline() {
        let img = RgbaImage::from_pixel(8, 8, image::Rgba([255, 0, 0, 255]));
        let req = VectorizeRequest::new(img).with_engine(Engine::Spline);
        let result = vectorize(&req).unwrap();
        assert!(result.svg.contains("<svg"));
        assert_eq!(result.engine_used, Engine::Spline);
    }

    #[test]
    fn test_vectorize_auto() {
        let img = RgbaImage::from_pixel(16, 16, image::Rgba([255, 0, 0, 255]));
        let req = VectorizeRequest::new(img).with_engine(Engine::Auto);
        let result = vectorize(&req).unwrap();
        assert!(result.svg.contains("<svg"));
    }
}
