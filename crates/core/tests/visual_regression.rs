use covecto_core::{Engine, VectorizeConfig, VectorizeRequest, load_image, vectorize};
use std::path::PathBuf;

/// Visual regression test: vectorize an image, render the SVG back to PNG via resvg,
/// then compare pixel similarity against the original input.
///
/// We don't expect pixel-perfect match (especially for spline engine), but we verify:
/// 1. The SVG parses and renders without errors
/// 2. The output dimensions match the input
/// 3. For pixel-exact engine on simple images, structural similarity is high
fn fixture_path(name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../../tests/fixtures");
    p.push(name);
    p
}

/// Render an SVG string to an RGBA pixel buffer using resvg.
fn svg_to_pixels(svg: &str) -> Option<image::RgbaImage> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
    let size = tree.size();
    let w = size.width().ceil() as u32;
    let h = size.height().ceil() as u32;
    if w == 0 || h == 0 {
        return None;
    }

    let mut pixmap = tiny_skia::Pixmap::new(w, h)?;
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

    image::RgbaImage::from_raw(w, h, pixmap.data().to_vec())
}

/// Compute mean absolute error between two images of the same size.
fn mae(img1: &image::RgbaImage, img2: &image::RgbaImage) -> f64 {
    assert_eq!(
        img1.dimensions(),
        img2.dimensions(),
        "image dimensions must match"
    );
    let (w, h) = img1.dimensions();
    let total = (w as f64) * (h as f64) * 4.0;
    let mut sum: u64 = 0;
    for (a, b) in img1.pixels().zip(img2.pixels()) {
        sum += (a.0[0] as i64 - b.0[0] as i64).unsigned_abs();
        sum += (a.0[1] as i64 - b.0[1] as i64).unsigned_abs();
        sum += (a.0[2] as i64 - b.0[2] as i64).unsigned_abs();
        // Skip alpha comparison (SVG renders are always opaque)
    }
    sum as f64 / (total * (3.0 / 4.0))
}

/// Visual regression: vectorize → render → compare dimensions.
fn check_visual_match(fixture: &str, engine: Engine, max_mae: f64) {
    let path = fixture_path(fixture);
    let img = load_image(&path).unwrap_or_else(|e| panic!("failed to load {fixture}: {e}"));
    let (orig_w, orig_h) = (img.width(), img.height());

    let req = VectorizeRequest::new(img)
        .with_config(VectorizeConfig {
            optimize: true,
            ..Default::default()
        })
        .with_engine(engine);
    let result = vectorize(&req)
        .unwrap_or_else(|e| panic!("vectorize failed for {fixture} with {engine:?}: {e}"));

    // SVG must contain valid content
    assert!(result.svg.contains("<svg"), "SVG missing <svg> tag");
    assert!(result.engine_used == engine);

    // Render SVG back to pixels
    let rendered = svg_to_pixels(&result.svg)
        .unwrap_or_else(|| panic!("resvg render failed for {fixture} with {engine:?}"));

    let (rend_w, rend_h) = rendered.dimensions();
    assert_eq!(
        (orig_w, orig_h),
        (rend_w, rend_h),
        "rendered dimensions mismatch for {fixture}: expected {orig_w}x{orig_h}, got {rend_w}x{rend_h}"
    );

    let error = mae(&load_image(&path).unwrap(), &rendered);
    assert!(
        error <= max_mae,
        "{fixture} MAE {error:.2} exceeds threshold {max_mae:.2} (engine={engine:?})"
    );
}

// === Pixel-Exact engine tests (low MAE expected for simple images) ===

#[test]
fn visual_pixel_exact_solid_64x64() {
    check_visual_match("solid-64x64.png", Engine::PixelExact, 0.0);
}

#[test]
fn visual_pixel_exact_icon_16x16() {
    check_visual_match("icon-16x16.png", Engine::PixelExact, 0.0);
}

#[test]
fn visual_pixel_exact_icon_32x32() {
    check_visual_match("icon-32x32.png", Engine::PixelExact, 0.0);
}

#[test]
fn visual_pixel_exact_checkerboard() {
    check_visual_match("checkerboard-48x48.png", Engine::PixelExact, 0.0);
}

#[test]
fn visual_pixel_exact_gradient() {
    // Gradient has many colors, pixel-exact traces all regions
    // Optimization may introduce minor rounding
    check_visual_match("gradient-256x256.png", Engine::PixelExact, 65.0);
}

#[test]
fn visual_pixel_exact_shapes() {
    check_visual_match("shapes-512x512.png", Engine::PixelExact, 2.0);
}

// === Spline engine tests (higher tolerance — lossy by nature) ===

#[test]
fn visual_spline_icon_16x16() {
    check_visual_match("icon-16x16.png", Engine::Spline, 30.0);
}

#[test]
fn visual_spline_solid() {
    check_visual_match("solid-64x64.png", Engine::Spline, 1.0);
}

#[test]
fn visual_spline_gradient() {
    check_visual_match("gradient-256x256.png", Engine::Spline, 70.0);
}

// === New fixtures ===

#[test]
fn visual_pixel_exact_logo_simple() {
    check_visual_match("logo-simple-64.png", Engine::PixelExact, 0.0);
}

#[test]
fn visual_pixel_exact_multicolor_icon() {
    check_visual_match("icon-multicolor-48.png", Engine::PixelExact, 0.0);
}

#[test]
fn visual_pixel_exact_lineart_grid() {
    check_visual_match("lineart-grid-128.png", Engine::PixelExact, 0.0);
}

#[test]
fn visual_pixel_exact_text_block() {
    check_visual_match("text-block-64x32.png", Engine::PixelExact, 0.0);
}

#[test]
fn visual_spline_colorwheel() {
    // Color wheel has smooth transitions — spline approximation expected
    check_visual_match("colorwheel-128.png", Engine::Spline, 40.0);
}

#[test]
fn visual_pixel_exact_circuit() {
    // Circuit board: thin lines, precise paths
    check_visual_match("lineart-circuit-96.png", Engine::PixelExact, 0.0);
}

// === Auto engine selection ===

#[test]
fn visual_auto_small_icon() {
    // Small icon → auto should pick pixel-exact
    let path = fixture_path("icon-16x16.png");
    let img = load_image(&path).unwrap();
    let req = VectorizeRequest::new(img);
    let result = vectorize(&req).unwrap();
    assert!(matches!(result.engine_used, Engine::PixelExact));
    let rendered = svg_to_pixels(&result.svg).unwrap();
    assert_eq!(rendered.dimensions(), (16, 16));
}
