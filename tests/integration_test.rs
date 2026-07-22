//! Integration tests for covecto-core vectorization engines.

use covecto_core::{
    load_image, optimize_svg, vectorize, Engine, OptimizeConfig, OptimizePreset, VectorizeConfig,
    VectorizeRequest,
};
use std::path::Path;

fn fixtures_dir() -> String {
    format!("{}/tests/fixtures", env!("CARGO_MANIFEST_DIR"))
}

fn fixture_path(name: &str) -> String {
    format!("{}/{}", fixtures_dir(), name)
}

fn vectorize_file(path: &str, config: VectorizeConfig) -> covecto_core::VectorizeResult {
    let img = load_image(Path::new(path)).expect("failed to load fixture");
    let req = VectorizeRequest::new(img).with_config(config);
    vectorize(&req).expect("vectorize failed")
}

fn assert_valid_svg(svg: &str) {
    assert!(svg.contains("<svg"), "missing <svg tag");
    assert!(svg.contains("</svg>"), "missing </svg> tag");
    assert!(svg.contains("<path"), "should have at least one <path>");
}

// ─── Core Vectorization ───────────────────────────────────────────

#[test]
fn test_spline_icon_16x16() {
    let result = vectorize_file(
        &fixture_path("icon-16x16.png"),
        VectorizeConfig {
            engine: Engine::Spline,
            optimize: false,
            ..Default::default()
        },
    );
    assert_valid_svg(&result.svg);
    assert!(result.metadata.path_count > 0);
    assert!(result.metadata.svg_byte_size > 0);
    assert!(result.metadata.processing_time_ms > 0);
    assert_eq!(result.engine_used, "spline");
}

#[test]
fn test_spline_gradient_256x256() {
    let result = vectorize_file(
        &fixture_path("gradient-256x256.png"),
        VectorizeConfig {
            engine: Engine::Spline,
            optimize: false,
            ..Default::default()
        },
    );
    assert_valid_svg(&result.svg);
    assert!(result.metadata.path_count > 10, "gradient should have many paths");
    assert_eq!(result.engine_used, "spline");
}

#[test]
fn test_spline_shapes_512x512() {
    let result = vectorize_file(
        &fixture_path("shapes-512x512.png"),
        VectorizeConfig {
            engine: Engine::Spline,
            optimize: false,
            ..Default::default()
        },
    );
    assert_valid_svg(&result.svg);
    assert!(result.metadata.processing_time_ms < 10000, "should complete in <10s");
}

#[test]
fn test_pixel_exact_icon_32x32() {
    let result = vectorize_file(
        &fixture_path("icon-32x32.png"),
        VectorizeConfig {
            engine: Engine::PixelExact,
            optimize: false,
            ..Default::default()
        },
    );
    assert_valid_svg(&result.svg);
    assert_eq!(result.engine_used, "pixel-exact");
}

#[test]
fn test_pixel_exact_checkerboard_48x48() {
    let result = vectorize_file(
        &fixture_path("checkerboard-48x48.png"),
        VectorizeConfig {
            engine: Engine::PixelExact,
            optimize: false,
            ..Default::default()
        },
    );
    assert_valid_svg(&result.svg);
    assert_eq!(result.engine_used, "pixel-exact");
}

// ─── Auto Engine Selection ────────────────────────────────────────

#[test]
fn test_auto_selects_pixel_exact_for_small_icon() {
    // 16x16 → should select pixel-exact
    let result = vectorize_file(
        &fixture_path("icon-16x16.png"),
        VectorizeConfig {
            engine: Engine::Auto,
            optimize: false,
            ..Default::default()
        },
    );
    assert_eq!(result.engine_used, "pixel-exact");
}

#[test]
fn test_auto_selects_spline_for_gradient() {
    // 256x256 → should select spline
    let result = vectorize_file(
        &fixture_path("gradient-256x256.png"),
        VectorizeConfig {
            engine: Engine::Auto,
            optimize: false,
            ..Default::default()
        },
    );
    assert_eq!(result.engine_used, "spline");
}

#[test]
fn test_auto_selects_spline_for_shapes() {
    // 512x512 → should select spline
    let result = vectorize_file(
        &fixture_path("shapes-512x512.png"),
        VectorizeConfig {
            engine: Engine::Auto,
            optimize: false,
            ..Default::default()
        },
    );
    assert_eq!(result.engine_used, "spline");
}

#[test]
fn test_auto_selects_pixel_exact_for_32x32() {
    // 32x32 → should select pixel-exact (< 64x64)
    let result = vectorize_file(
        &fixture_path("icon-32x32.png"),
        VectorizeConfig {
            engine: Engine::Auto,
            optimize: false,
            ..Default::default()
        },
    );
    assert_eq!(result.engine_used, "pixel-exact");
}

// ─── Edge Cases ───────────────────────────────────────────────────

#[test]
fn test_pixel_1x1() {
    let result = vectorize_file(
        &fixture_path("pixel-1x1.png"),
        VectorizeConfig {
            engine: Engine::Auto,
            optimize: false,
            ..Default::default()
        },
    );
    assert_valid_svg(&result.svg);
}

#[test]
fn test_solid_color_64x64() {
    let result = vectorize_file(
        &fixture_path("solid-64x64.png"),
        VectorizeConfig {
            engine: Engine::Auto,
            optimize: false,
            ..Default::default()
        },
    );
    assert_valid_svg(&result.svg);
    assert_eq!(result.engine_used, "pixel-exact"); // 64x64 < 64x64 is false, so spline
}

#[test]
fn test_transparent_32x32() {
    let result = vectorize_file(
        &fixture_path("transparent-32x32.png"),
        VectorizeConfig {
            engine: Engine::PixelExact,
            optimize: false,
            ..Default::default()
        },
    );
    // Transparent image should still produce valid SVG (may be empty/minimal)
    assert!(result.svg.contains("<svg"), "transparent image should still produce SVG");
}

// ─── Optimization ─────────────────────────────────────────────────

#[test]
fn test_optimize_reduces_size() {
    let result = vectorize_file(
        &fixture_path("shapes-512x512.png"),
        VectorizeConfig {
            engine: Engine::Spline,
            optimize: false,
            ..Default::default()
        },
    );
    let original_size = result.svg.len();

    let optimized = optimize_svg(
        &result.svg,
        &OptimizeConfig {
            preset: OptimizePreset::Default,
            multipass: false,
            multipass_iterations: 10,
        },
    )
    .expect("optimize failed");

    assert!(optimized.len() <= original_size, "optimized should not be larger");
    // At least some reduction for a 512x512 image
    assert!(
        optimized.len() < original_size,
        "optimized SVG ({} bytes) should be smaller than original ({} bytes)",
        optimized.len(),
        original_size
    );
}

#[test]
fn test_optimize_safe_preserves_structure() {
    let result = vectorize_file(
        &fixture_path("icon-32x32.png"),
        VectorizeConfig {
            engine: Engine::PixelExact,
            optimize: false,
            ..Default::default()
        },
    );

    let optimized = optimize_svg(
        &result.svg,
        &OptimizeConfig {
            preset: OptimizePreset::Safe,
            multipass: false,
            multipass_iterations: 10,
        },
    )
    .expect("optimize failed");

    assert!(optimized.contains("<svg"), "safe optimize preserves svg tag");
    assert!(optimized.contains("</svg>"), "safe optimize preserves closing tag");
}

#[test]
fn test_optimize_none_is_noop() {
    let result = vectorize_file(
        &fixture_path("icon-16x16.png"),
        VectorizeConfig {
            engine: Engine::Spline,
            optimize: false,
            ..Default::default()
        },
    );

    let optimized = optimize_svg(
        &result.svg,
        &OptimizeConfig {
            preset: OptimizePreset::None,
            multipass: false,
            multipass_iterations: 10,
        },
    )
    .expect("optimize failed");

    assert_eq!(optimized, result.svg, "none preset should return identical SVG");
}

// ─── Performance Benchmarks ───────────────────────────────────────

#[test]
fn test_perf_small_image_under_100ms() {
    let start = std::time::Instant::now();
    let _ = vectorize_file(
        &fixture_path("icon-16x16.png"),
        VectorizeConfig {
            engine: Engine::Auto,
            optimize: false,
            ..Default::default()
        },
    );
    let elapsed = start.elapsed().as_millis();
    assert!(elapsed < 100, "small image should vectorize in <100ms, took {elapsed}ms");
}

#[test]
fn test_perf_medium_image_under_5s() {
    let start = std::time::Instant::now();
    let _ = vectorize_file(
        &fixture_path("gradient-256x256.png"),
        VectorizeConfig {
            engine: Engine::Spline,
            optimize: false,
            ..Default::default()
        },
    );
    let elapsed = start.elapsed().as_secs_f64();
    assert!(
        elapsed < 5.0,
        "medium image should vectorize in <5s, took {elapsed:.2}s"
    );
}

// ─── SVG Quality ──────────────────────────────────────────────────

#[test]
fn test_spline_output_has_curves() {
    let result = vectorize_file(
        &fixture_path("gradient-256x256.png"),
        VectorizeConfig {
            engine: Engine::Spline,
            optimize: false,
            ..Default::default()
        },
    );
    // Spline engine should produce Bézier curves (C command)
    assert!(
        result.svg.contains('C') || result.svg.contains('c'),
        "spline output should contain cubic Bézier curves"
    );
}

#[test]
fn test_pixel_exact_output_no_curves() {
    let result = vectorize_file(
        &fixture_path("checkerboard-48x48.png"),
        VectorizeConfig {
            engine: Engine::PixelExact,
            optimize: false,
            ..Default::default()
        },
    );
    // Pixel exact should only have M/h/v/Z (no curves)
    let has_curve = result.svg.contains('C') || result.svg.contains('c');
    assert!(!has_curve, "pixel-exact should not have Bézier curves");
}