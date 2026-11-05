//! Spline vectorization engine (vtracer-backed).
//!
//! Uses the vtracer crate for color quantization → contour tracing →
//! Bézier spline fitting. Produces smooth, visually appealing SVG output
//! suitable for photos and illustrations.

use crate::config::{
    ColorMode, DEFAULT_SPLINE_MAX_MEGAPIXELS, HierarchicalMode, PathSimplifyMode, VectorizeConfig,
};
use crate::{Error, Result};
use image::RgbaImage;
use vtracer::{ColorMode as VColorMode, Config as VConfig, Hierarchical as VHierarchical};

/// Vectorize an image using vtracer's spline algorithm.
///
/// Inputs larger than the configured budget
/// ([`VectorizeConfig::spline_max_megapixels`]) are downscaled first; the
/// returned SVG still has the original `width`/`height`, with a `viewBox`
/// of the traced size.
pub fn vectorize(img: &RgbaImage, config: &VectorizeConfig) -> Result<String> {
    let v_config = build_vtracer_config(config);
    let (w, h) = img.dimensions();
    let Some((tw, th)) = traced_size(w, h, config.spline_max_megapixels) else {
        let svg_file = vtracer::convert(to_color_image(img), v_config).map_err(Error::Vtracer)?;
        return Ok(svg_file.to_string());
    };
    tracing::debug!("spline: downscaling {w}x{h} to {tw}x{th} before tracing");
    let small = image::imageops::thumbnail(img, tw, th);
    let svg = vtracer::convert(to_color_image(&small), v_config)
        .map_err(Error::Vtracer)?
        .to_string();
    restore_display_size(svg, (tw, th), (w, h))
}

/// Size to trace at, or `None` when the image fits the budget (or the
/// budget is disabled with a value <= 0 / non-finite).
pub(crate) fn traced_size(w: u32, h: u32, max_megapixels: Option<f64>) -> Option<(u32, u32)> {
    let budget = max_megapixels.unwrap_or(DEFAULT_SPLINE_MAX_MEGAPIXELS);
    if !(budget.is_finite() && budget > 0.0) {
        return None;
    }
    let max_pixels = budget * 1_000_000.0;
    let pixels = f64::from(w) * f64::from(h);
    if pixels <= max_pixels {
        return None;
    }
    let scale = (max_pixels / pixels).sqrt();
    let tw = ((f64::from(w) * scale).floor() as u32).clamp(1, w);
    let th = ((f64::from(h) * scale).floor() as u32).clamp(1, h);
    Some((tw, th))
}

/// Rewrite vtracer's `<svg ... width="tw" height="th">` header so the
/// document displays at the original size, mapping the traced coordinates
/// through `viewBox`.
fn restore_display_size(svg: String, (tw, th): (u32, u32), (w, h): (u32, u32)) -> Result<String> {
    let traced = format!(r#"width="{tw}" height="{th}">"#);
    let Some(at) = svg.find(&traced) else {
        return Err(Error::Vtracer(format!(
            "unexpected vtracer SVG header (looked for {traced})"
        )));
    };
    let mut out = String::with_capacity(svg.len() + 32);
    out.push_str(&svg[..at]);
    // Each axis is scaled independently back to the original size; rounding
    // the traced size can shift the aspect ratio slightly, so do not let the
    // default `xMidYMid meet` letterbox it.
    out.push_str(&format!(
        r#"width="{w}" height="{h}" viewBox="0 0 {tw} {th}" preserveAspectRatio="none">"#
    ));
    out.push_str(&svg[at + traced.len()..]);
    Ok(out)
}

/// Stream vtracer output to a writer. Falls back to in-memory String since vtracer
/// doesn't support streaming, but avoids a second copy when writing to disk.
pub fn vectorize_to<W: std::io::Write>(
    writer: &mut W,
    img: &RgbaImage,
    config: &VectorizeConfig,
) -> Result<super::EngineOutput> {
    let svg = vectorize(img, config)?;
    let bytes = svg.as_bytes();
    writer.write_all(bytes)?;
    Ok(super::EngineOutput {
        byte_count: bytes.len(),
        path_count: svg.matches("<path").count(),
    })
}

fn to_color_image(img: &RgbaImage) -> vtracer::ColorImage {
    let (w, h) = (img.width() as usize, img.height() as usize);
    // vtracer ColorImage uses the same flat RGBA layout: one memcpy.
    vtracer::ColorImage {
        pixels: img.as_raw().to_vec(),
        width: w,
        height: h,
    }
}

/// Map a resolved config onto vtracer's. A Preset was already expanded into
/// these fields by `options::resolve`, so this is a plain field mapping.
fn build_vtracer_config(config: &VectorizeConfig) -> VConfig {
    let mut vc = VConfig::default();

    if let Some(cm) = config.color_mode {
        vc.color_mode = match cm {
            ColorMode::Color => VColorMode::Color,
            ColorMode::Binary => VColorMode::Binary,
        };
    }
    if let Some(v) = config.color_precision {
        vc.color_precision = v;
    }
    if let Some(v) = config.filter_speckle {
        vc.filter_speckle = v;
    }
    if let Some(v) = config.corner_threshold {
        vc.corner_threshold = v;
    }
    if let Some(v) = config.splice_threshold {
        vc.splice_threshold = v;
    }
    if let Some(v) = config.layer_difference {
        vc.layer_difference = v;
    }
    if let Some(v) = config.length_threshold {
        vc.length_threshold = v;
    }
    if let Some(v) = config.max_iterations {
        vc.max_iterations = v;
    }
    if let Some(v) = config.path_precision {
        vc.path_precision = Some(v);
    }
    if let Some(h) = config.hierarchical {
        vc.hierarchical = match h {
            HierarchicalMode::Stacked => VHierarchical::Stacked,
            HierarchicalMode::Cutout => VHierarchical::Cutout,
        };
    }
    // Was never mapped before, so `--path-simplify` was silently ignored.
    if let Some(m) = config.path_simplify_mode {
        vc.mode = match m {
            PathSimplifyMode::Spline => visioncortex::PathSimplifyMode::Spline,
            PathSimplifyMode::Polygon => visioncortex::PathSimplifyMode::Polygon,
            PathSimplifyMode::None => visioncortex::PathSimplifyMode::None,
        };
    }

    vc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traced_size_budget() {
        // Fits: no downscale. Default budget is 2 MP.
        assert_eq!(traced_size(1000, 1000, None), None);
        assert_eq!(traced_size(100, 100, Some(0.01)), None);
        // Disabled.
        assert_eq!(traced_size(8000, 8000, Some(0.0)), None);
        assert_eq!(traced_size(8000, 8000, Some(-1.0)), None);
        assert_eq!(traced_size(8000, 8000, Some(f64::NAN)), None);
        // 4000x4000 at 2 MP keeps the aspect ratio and stays within budget.
        let (w, h) = traced_size(4000, 4000, None).unwrap();
        assert_eq!((w, h), (1414, 1414));
        assert!(u64::from(w) * u64::from(h) <= 2_000_000);
        // Extreme aspect ratio never yields a zero side.
        let (w, h) = traced_size(100_000, 2, Some(0.000_01)).unwrap();
        assert!(w >= 1 && (1..=2).contains(&h));
    }

    #[test]
    fn downscaled_svg_keeps_display_size() {
        let img = RgbaImage::from_fn(100, 60, |x, y| {
            image::Rgba([(x * 2) as u8, (y * 4) as u8, ((x + y) % 256) as u8, 255])
        });
        let config = VectorizeConfig {
            spline_max_megapixels: Some(0.0015), // 1500 px budget
            ..Default::default()
        };
        let svg = vectorize(&img, &config).unwrap();
        let (tw, th) = traced_size(100, 60, Some(0.0015)).unwrap();
        assert!(
            svg.contains(&format!(
                r#"width="100" height="60" viewBox="0 0 {tw} {th}" preserveAspectRatio="none">"#
            )),
            "{}",
            &svg[..svg.len().min(300)]
        );
        let tree = usvg::Tree::from_str(&svg, &usvg::Options::default()).unwrap();
        assert_eq!((tree.size().width(), tree.size().height()), (100.0, 60.0));

        // Disabled limit: traced at full size, no viewBox added.
        let full = VectorizeConfig {
            spline_max_megapixels: Some(0.0),
            ..Default::default()
        };
        let svg = vectorize(&img, &full).unwrap();
        assert!(svg.contains(r#"width="100" height="60">"#));
        assert!(!svg.contains("viewBox"));
    }
}
