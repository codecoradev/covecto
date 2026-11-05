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
mod export;
mod optimize;
pub mod options;

pub use config::{
    ColorMode, DEFAULT_SPLINE_MAX_MEGAPIXELS, Engine, HierarchicalMode, OptimizeConfig,
    OptimizePreset, PathSimplifyMode, SplinePreset, VectorizeConfig, VectorizeRequest,
};
pub use engine::{pixel_exact, spline};
pub use error::{
    DEFAULT_MAX_DIMENSION, Error, Result, load_image, load_image_from_bytes,
    load_image_from_bytes_with_limit,
};
pub use export::{OutputFormat, convert_output};
pub use optimize::{MAX_MULTIPASS_ITERATIONS, optimize_svg};

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

/// The Engine that will actually run for `config` on `image`: the configured
/// Engine, or, for [`Engine::Auto`], the one chosen from the image's size and
/// colors. Never returns `Engine::Auto`.
///
/// [`vectorize`] and [`vectorize_to`] use exactly this decision, so callers
/// can preview it (e.g. a dry run) without it drifting from a real run. Cost:
/// free unless the Engine is `Auto`, then one pass over at most ~131k
/// sampled pixels.
pub fn select_engine(config: &VectorizeConfig, image: &RgbaImage) -> Engine {
    match config.engine {
        Engine::Auto => auto_select_engine(image),
        explicit => explicit,
    }
}

/// Select the best engine based on image characteristics.
fn auto_select_engine(img: &RgbaImage) -> Engine {
    let (w, h) = (img.width(), img.height());
    let total_pixels = f64::from(w) * f64::from(h);

    if total_pixels == 0.0 {
        return Engine::PixelExact;
    }

    // Count unique colors (sample-based for large images). Once the count
    // exceeds both thresholds below, the result is Spline regardless.
    let samples = sample_count(img.as_raw().len() / 4);
    let threshold = spline_color_threshold(total_pixels, samples);
    let cap = 129usize.max(threshold as usize + 1);
    let unique_colors = count_unique_colors(img, cap);
    choose_engine(w, h, unique_colors, threshold)
}

/// Pure engine decision.
/// - Small images (<= 256x256) with few colors (<= 128) → PixelExact
/// - More unique colors than `threshold` → Spline (likely a photo)
/// - Otherwise → PixelExact (lossless, fast)
fn choose_engine(w: u32, h: u32, unique_colors: usize, threshold: f64) -> Engine {
    if w <= 256 && h <= 256 && unique_colors <= 128 {
        Engine::PixelExact
    } else if unique_colors as f64 > threshold {
        Engine::Spline
    } else {
        Engine::PixelExact
    }
}

/// Target number of pixels sampled by [`count_unique_colors`]. The integer
/// step means the real count is between this and twice this.
const COLOR_SAMPLE_LIMIT: usize = 65_536;

/// Number of pixels [`count_unique_colors`] looks at for `total` pixels.
fn sample_count(total: usize) -> usize {
    let step = if total > COLOR_SAMPLE_LIMIT {
        total / COLOR_SAMPLE_LIMIT
    } else {
        1
    };
    total.div_ceil(step)
}

/// Unique (sampled) colors needed to pick Spline: 0.5% of the pixel count,
/// capped at a quarter of the samples.
///
/// The count comes from at most ~65k-131k samples, so without the cap the
/// requirement passes the sample count around 13.1 MP and Spline could
/// never be chosen (a large photo went to PixelExact and produced a huge
/// SVG, #60). The cap keeps the requirement continuous in image size: images
/// below ~3.3 MP are decided exactly as before, and above that a photo needs
/// more than 25% of the samples to be distinct, which flat screenshots with
/// antialiased text (hundreds to a few thousand colors) do not reach.
fn spline_color_threshold(total_pixels: f64, samples: usize) -> f64 {
    (total_pixels * 0.005).min(samples as f64 * 0.25)
}

/// Count unique RGBA colors in an image, up to `cap`.
/// For large images, samples roughly 65_536 to 131_071 pixels (see
/// [`sample_count`]) to keep this fast.
/// Stops early once `cap` distinct colors are seen: past that point the
/// engine decision no longer depends on the exact count.
fn count_unique_colors(img: &RgbaImage, cap: usize) -> usize {
    use std::collections::HashSet;
    let raw = img.as_raw();
    let total = raw.len() / 4;
    let step = if total > COLOR_SAMPLE_LIMIT {
        total / COLOR_SAMPLE_LIMIT
    } else {
        1
    };

    let mut colors: HashSet<u32> = HashSet::with_capacity(cap.min(4096));
    for px in raw.as_chunks::<4>().0.iter().step_by(step) {
        colors.insert(u32::from_ne_bytes([px[0], px[1], px[2], px[3]]));
        if colors.len() >= cap {
            break;
        }
    }
    colors.len()
}

/// Count `<path` occurrences in an SVG string (approximate path count).
fn count_svg_paths(svg: &str) -> usize {
    svg.matches("<path").count()
}

/// Streaming vectorize metadata (returned instead of the full SVG string).
#[derive(Debug, Clone)]
pub struct StreamResult {
    /// Which engine was used.
    pub engine_used: Engine,
    /// Metadata about the operation.
    pub metadata: ResultMetadata,
}

/// Whether the post-vectorize optimizer should run for this engine.
///
/// PixelExact output is already minimal (integer coordinates, no prolog,
/// comments, metadata or empty groups), so every optimizer pass is a no-op on
/// it. Skipping it keeps `vectorize_to` truly streaming (#54).
fn should_optimize(engine: Engine, config: &VectorizeConfig) -> bool {
    config.optimize && engine != Engine::PixelExact
}

/// Warn when PixelExact produced pathological output (roughly one path per
/// few pixels), which typically means a photo was forced through it.
fn warn_if_pathological(engine: Engine, (w, h): (u32, u32), path_count: usize) {
    let pixels = w as usize * h as usize;
    if engine == Engine::PixelExact && pixels >= 65_536 && path_count > pixels / 8 {
        tracing::warn!(
            "PixelExact produced {path_count} paths for a {w}x{h} image; \
             the spline engine is usually far smaller for photo-like input"
        );
    }
}

/// Main vectorization entry point.
pub fn vectorize(request: &VectorizeRequest) -> Result<VectorizeResult> {
    let engine = select_engine(&request.config, &request.image);

    let start = Instant::now();
    let raw_svg = match engine {
        Engine::PixelExact => pixel_exact::vectorize(&request.image),
        Engine::Spline => spline::vectorize(&request.image, &request.config),
        Engine::Auto => unreachable!(),
    }?;

    // Post-processing: optimization
    let svg = if should_optimize(engine, &request.config) {
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
    let path_count = count_svg_paths(&svg);
    warn_if_pathological(engine, (w, h), path_count);

    Ok(VectorizeResult {
        engine_used: engine,
        metadata: ResultMetadata {
            input_size: (w, h),
            svg_byte_size,
            path_count,
            processing_time_ms: elapsed.as_millis() as u64,
            compression_ratio,
        },
        svg,
    })
}

fn render<W: std::io::Write>(
    writer: &mut W,
    engine: Engine,
    request: &VectorizeRequest,
) -> Result<pixel_exact::VectorizeMeta> {
    match engine {
        Engine::PixelExact => pixel_exact::vectorize_to(writer, &request.image),
        Engine::Spline => spline::vectorize_to(writer, &request.image, &request.config),
        Engine::Auto => unreachable!(),
    }
}

/// Streaming vectorization — writes SVG directly to a writer instead of returning a String.
/// For PixelExact: SVG is written incrementally during assembly.
/// For Spline: vtracer builds in memory, but result is written directly to the writer.
pub fn vectorize_to<W: std::io::Write>(
    writer: &mut W,
    request: &VectorizeRequest,
) -> Result<StreamResult> {
    let engine = select_engine(&request.config, &request.image);

    let start = Instant::now();
    let (w, h) = (request.image.width(), request.image.height());
    let raw_pixel_bytes = (w as usize * h as usize) * 4;

    // Optimization needs the whole SVG, so buffer it and write the result once.
    let (svg_byte_size, path_count) = if should_optimize(engine, &request.config) {
        let mut buf = Vec::new();
        render(&mut buf, engine, request)?;
        let raw = String::from_utf8(buf).map_err(|e| Error::Optimization(e.to_string()))?;
        let svg = optimize_svg(&raw, &request.config.optimize_config)?;
        writer.write_all(svg.as_bytes())?;
        (svg.len(), count_svg_paths(&svg))
    } else {
        let m = render(writer, engine, request)?;
        (m.byte_count, m.path_count)
    };
    warn_if_pathological(engine, (w, h), path_count);

    let elapsed = start.elapsed();
    let compression_ratio = if raw_pixel_bytes > 0 {
        svg_byte_size as f64 / raw_pixel_bytes as f64
    } else {
        1.0
    };

    Ok(StreamResult {
        engine_used: engine,
        metadata: ResultMetadata {
            input_size: (w, h),
            svg_byte_size,
            path_count,
            processing_time_ms: elapsed.as_millis() as u64,
            compression_ratio,
        },
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
    fn test_vectorize_to_applies_optimization() {
        let img = RgbaImage::from_fn(16, 16, |x, y| {
            image::Rgba([(x % 4 * 60) as u8, (y % 4 * 60) as u8, 0, 255])
        });
        let run = |optimize| {
            let req = VectorizeRequest::new(img.clone())
                .with_engine(Engine::Spline)
                .with_optimize(optimize);
            let mut buf = Vec::new();
            let res = vectorize_to(&mut buf, &req).unwrap();
            (buf, res.metadata)
        };
        let (raw, raw_meta) = run(false);
        let (opt, opt_meta) = run(true);
        assert!(
            opt.len() < raw.len() || opt_meta.path_count < raw_meta.path_count,
            "optimized {}B/{} paths vs raw {}B/{} paths",
            opt.len(),
            opt_meta.path_count,
            raw.len(),
            raw_meta.path_count
        );
        assert_eq!(opt_meta.svg_byte_size, opt.len());
        assert_eq!(
            opt_meta.path_count,
            count_svg_paths(std::str::from_utf8(&opt).unwrap())
        );
    }

    #[test]
    fn test_parity_vectorize_vs_vectorize_to() {
        // Anti-divergence lock (issue #45): both public entry points must
        // produce identical SVG bytes and equal metadata for the same request.
        // This is the failure class behind #42 (streaming skipped optimize).
        let rgba = image::Rgba;
        let mixed = RgbaImage::from_fn(24, 18, |x, y| {
            rgba([
                (x * 7 % 256) as u8,
                (y * 11 % 256) as u8,
                ((x + y) % 256) as u8,
                if y < 4 { 128 } else { 255 },
            ])
        });
        // several translucent alpha groups
        let multi_alpha = RgbaImage::from_fn(16, 16, |x, y| {
            let a = [64, 128, 200, 255][(x / 4) as usize % 4];
            rgba([(x * 16) as u8, (y * 16) as u8, 90, a])
        });
        // fully transparent pixels (a == 0) mixed with opaque ones
        let holes = RgbaImage::from_fn(12, 12, |x, y| {
            if (x + y) % 3 == 0 {
                rgba([9, 9, 9, 0])
            } else {
                rgba([(x * 20) as u8, (y * 20) as u8, 40, 255])
            }
        });
        let one = RgbaImage::from_pixel(1, 1, rgba([10, 200, 30, 255]));
        let icon = RgbaImage::from_fn(16, 16, |x, y| {
            rgba([if (x / 4 + y / 4) % 2 == 0 { 255 } else { 0 }, 0, 0, 255])
        });

        type Tweak = fn(&mut VectorizeConfig);
        let tweaks: [(&str, Tweak); 7] = [
            ("default", |_| {}),
            ("binary", |c| c.color_mode = Some(ColorMode::Binary)),
            ("cutout", |c| {
                c.hierarchical = Some(HierarchicalMode::Cutout)
            }),
            ("speckle", |c| c.filter_speckle = Some(8)),
            ("poster", |c| options::apply_preset(c, SplinePreset::Poster)),
            ("opt-safe", |c| {
                c.optimize_config.preset = OptimizePreset::Safe
            }),
            ("opt-none", |c| {
                c.optimize_config.preset = OptimizePreset::None
            }),
        ];
        let images = [
            ("mixed", &mixed),
            ("multi_alpha", &multi_alpha),
            ("holes", &holes),
            ("1x1", &one),
        ];

        let mut cases = Vec::new();
        for (iname, img) in images {
            for engine in [Engine::PixelExact, Engine::Spline] {
                for (tname, tweak) in tweaks {
                    for optimize in [false, true] {
                        let name = format!("{iname}/{engine:?}/{tname}/opt={optimize}");
                        cases.push((name, img, engine, tweak, optimize));
                    }
                }
            }
        }
        // Engine::Auto (resolves to PixelExact for the small icon)
        for optimize in [false, true] {
            let name = format!("icon/Auto/default/opt={optimize}");
            cases.push((name, &icon, Engine::Auto, tweaks[0].1, optimize));
        }

        for (name, img, engine, tweak, optimize) in cases {
            let mut req = VectorizeRequest::new(img.clone())
                .with_engine(engine)
                .with_optimize(optimize);
            tweak(&mut req.config);

            let buffered = vectorize(&req).unwrap();
            let mut sink = Vec::new();
            let streamed = vectorize_to(&mut sink, &req).unwrap();
            let (bm, sm) = (&buffered.metadata, &streamed.metadata);

            assert_eq!(
                buffered.svg.as_bytes(),
                sink.as_slice(),
                "{name}: SVG bytes"
            );
            assert_eq!(buffered.engine_used, streamed.engine_used, "{name}: engine");
            assert_eq!(bm.svg_byte_size, sm.svg_byte_size, "{name}: svg_byte_size");
            assert_eq!(bm.path_count, sm.path_count, "{name}: path_count");
            assert_eq!(bm.compression_ratio, sm.compression_ratio, "{name}: ratio");
            assert_eq!(bm.input_size, sm.input_size, "{name}: input_size");

            // Ground truth: metadata must match what was actually written.
            assert_eq!(
                sm.svg_byte_size,
                sink.len(),
                "{name}: size != written bytes"
            );
            assert_eq!(
                sm.path_count,
                count_svg_paths(std::str::from_utf8(&sink).unwrap()),
                "{name}: path_count != '<path' in written bytes"
            );
        }
    }

    #[test]
    fn test_optimizer_is_noop_on_pixel_exact_output() {
        // Justifies skipping the optimizer for PixelExact (#54): every preset
        // must leave its output byte-identical, including translucent groups
        // (#59), and the output must be well-formed XML.
        let rgba = image::Rgba;
        let images = [
            RgbaImage::from_fn(16, 16, |x, y| {
                let a = [0, 64, 128, 200, 255][((x + y) / 4) as usize % 5];
                rgba([(x * 16) as u8, (y * 16) as u8, 90, a])
            }),
            RgbaImage::from_fn(20, 12, |x, y| {
                rgba([(x * 13 % 256) as u8, (y * 21 % 256) as u8, 7, 255])
            }),
            RgbaImage::from_pixel(1, 1, rgba([1, 2, 3, 4])),
        ];
        for img in images {
            let svg = pixel_exact::vectorize(&img).unwrap();
            roxmltree::Document::parse(&svg).expect("pixel-exact SVG is well-formed");
            for preset in [OptimizePreset::Default, OptimizePreset::Safe] {
                let config = OptimizeConfig {
                    preset,
                    multipass: true,
                    multipass_iterations: 3,
                };
                assert_eq!(optimize_svg(&svg, &config).unwrap(), svg, "{preset:?}");
            }
        }
    }

    #[test]
    fn test_auto_select_large_images() {
        let threshold =
            |w: usize, h: usize| spline_color_threshold((w * h) as f64, sample_count(w * h));
        // Above ~13.1 MP the old rule needed more unique colors than samples,
        // so even an all-unique photo chose PixelExact.
        let samples = sample_count(4000 * 4000);
        assert!(samples < 2 * COLOR_SAMPLE_LIMIT, "{samples}");
        assert_eq!(
            choose_engine(4000, 4000, samples, threshold(4000, 4000)),
            Engine::Spline
        );
        // A 5K screenshot with antialiased text (a few thousand colors) stays
        // lossless, like the same content at 4K.
        assert_eq!(
            choose_engine(5120, 2880, 3_000, threshold(5120, 2880)),
            Engine::PixelExact
        );
        assert_eq!(
            choose_engine(3840, 2160, 3_000, threshold(3840, 2160)),
            Engine::PixelExact
        );
        // Small and mid-size images are decided exactly as before (0.5%).
        assert_eq!(threshold(1000, 1000), 5_000.0);
        assert_eq!(threshold(512, 512), 512.0 * 512.0 * 0.005);
        // No cliff around 13.1 MP: the requirement changes smoothly.
        let (a, b) = (threshold(3600, 3600), threshold(3640, 3640));
        assert!((a - b).abs() / a < 0.1, "{a} vs {b}");
    }

    #[test]
    fn test_select_engine_matches_what_runs() {
        let icon = RgbaImage::from_pixel(16, 16, image::Rgba([255, 0, 0, 255]));
        let photo = RgbaImage::from_fn(512, 512, |x, y| {
            image::Rgba([(x * 255 / 512) as u8, (y * 255 / 512) as u8, 128, 255])
        });
        let auto = VectorizeConfig::default();
        assert_eq!(select_engine(&auto, &icon), Engine::PixelExact);
        assert_eq!(select_engine(&auto, &photo), Engine::Spline);
        // A configured Engine is returned as is (no image analysis needed).
        let forced = options::resolve([("engine", "pixel-exact")]).unwrap();
        assert_eq!(select_engine(&forced, &photo), Engine::PixelExact);
        // A Profile that chooses the Engine is honoured too.
        let icon_profile = options::resolve([("profile", "icon")]).unwrap();
        assert_eq!(select_engine(&icon_profile, &icon), Engine::Spline);
        // ...and the real run agrees with the preview.
        for (config, img) in [(&auto, &icon), (&auto, &photo), (&icon_profile, &icon)] {
            let req = VectorizeRequest::new(img.clone()).with_config(config.clone());
            assert_eq!(
                vectorize(&req).unwrap().engine_used,
                select_engine(config, img)
            );
        }
    }

    #[test]
    fn test_count_unique_colors_cap() {
        let img = RgbaImage::from_fn(64, 64, |x, y| image::Rgba([x as u8, y as u8, 0, 255]));
        assert_eq!(count_unique_colors(&img, usize::MAX), 4096);
        assert_eq!(count_unique_colors(&img, 100), 100);
    }

    #[test]
    fn test_vectorize_auto() {
        let img = RgbaImage::from_pixel(16, 16, image::Rgba([255, 0, 0, 255]));
        let req = VectorizeRequest::new(img).with_engine(Engine::Auto);
        let result = vectorize(&req).unwrap();
        assert!(result.svg.contains("<svg"));
    }
}
