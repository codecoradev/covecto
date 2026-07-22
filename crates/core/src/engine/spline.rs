//! Spline vectorization engine (vtracer-backed).
//!
//! Uses the vtracer crate for color quantization → contour tracing →
//! Bézier spline fitting. Produces smooth, visually appealing SVG output
//! suitable for photos and illustrations.

use crate::config::{ColorMode, HierarchicalMode, SplinePreset, VectorizeConfig};
use crate::{Error, Result};
use image::RgbaImage;
use vtracer::{ColorMode as VColorMode, Config as VConfig, Hierarchical as VHierarchical};

/// Vectorize an image using vtracer's spline algorithm.
pub fn vectorize(img: &RgbaImage, config: &VectorizeConfig) -> Result<String> {
    let color_image = to_color_image(img);
    let v_config = build_vtracer_config(config);
    let svg_file = vtracer::convert(color_image, v_config).map_err(Error::Vtracer)?;
    Ok(svg_file.to_string())
}

fn to_color_image(img: &RgbaImage) -> vtracer::ColorImage {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let raw = img.as_raw();
    // vtracer ColorImage uses flat Vec<u8> in RGBA order
    let mut pixels = Vec::with_capacity(w * h * 4);
    for chunk in raw.chunks(4) {
        pixels.push(chunk[0]); // r
        pixels.push(chunk[1]); // g
        pixels.push(chunk[2]); // b
        pixels.push(chunk[3]); // a
    }
    vtracer::ColorImage {
        pixels,
        width: w,
        height: h,
    }
}

fn build_vtracer_config(config: &VectorizeConfig) -> VConfig {
    if let Some(preset) = config.spline_preset {
        let v_preset = match preset {
            SplinePreset::Bw => vtracer::Preset::Bw,
            SplinePreset::Poster => vtracer::Preset::Poster,
            SplinePreset::Photo => vtracer::Preset::Photo,
        };
        return VConfig::from_preset(v_preset);
    }

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

    vc
}
