//! Engine and optimizer benchmarks (#57).
//!
//! Run with `cargo bench -p covecto-core`. Inputs are generated in code so
//! results are deterministic and no binary fixtures are needed.

use covecto_core::{
    Engine, OptimizeConfig, VectorizeRequest, optimize_svg, pixel_exact, vectorize, vectorize_to,
};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use image::{Rgba, RgbaImage};
use std::hint::black_box;

/// Flat-color blocks: typical icon / screenshot input.
fn blocky(size: u32) -> RgbaImage {
    const PALETTE: [[u8; 4]; 6] = [
        [230, 57, 70, 255],
        [241, 250, 238, 255],
        [168, 218, 220, 255],
        [69, 123, 157, 255],
        [29, 53, 87, 255],
        [255, 183, 3, 128],
    ];
    RgbaImage::from_fn(size, size, |x, y| {
        Rgba(PALETTE[((x / 13) * 7 + (y / 9) * 3) as usize % PALETTE.len()])
    })
}

/// Smooth gradient with mild deterministic noise: photo-like input.
fn photo(size: u32) -> RgbaImage {
    RgbaImage::from_fn(size, size, |x, y| {
        let n = (x.wrapping_mul(2_654_435_761) ^ y.wrapping_mul(40_503)) >> 29;
        Rgba([
            (x * 255 / size) as u8 ^ n as u8,
            (y * 255 / size) as u8,
            ((x + y) * 127 / size) as u8,
            255,
        ])
    })
}

/// Per-pixel noise: worst case for PixelExact.
fn noise(size: u32) -> RgbaImage {
    let mut state = 0x9E37_79B9u32;
    RgbaImage::from_fn(size, size, |_, _| {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        Rgba([state as u8, (state >> 8) as u8, (state >> 16) as u8, 255])
    })
}

fn bench_engines(c: &mut Criterion) {
    let inputs = [
        ("blocky-512", blocky(512)),
        ("photo-256", photo(256)),
        ("noise-128", noise(128)),
    ];
    let mut group = c.benchmark_group("vectorize");
    group.sample_size(10);
    for (name, img) in &inputs {
        group.throughput(Throughput::Elements(u64::from(img.width() * img.height())));
        for engine in [Engine::PixelExact, Engine::Spline] {
            for optimize in [false, true] {
                let req = VectorizeRequest::new(img.clone())
                    .with_engine(engine)
                    .with_optimize(optimize);
                let id = BenchmarkId::new(format!("{engine}/opt={optimize}"), name);
                group.bench_with_input(id, &req, |b, req| {
                    b.iter(|| vectorize(black_box(req)).unwrap())
                });
            }
        }
        let req = VectorizeRequest::new(img.clone()).with_engine(Engine::PixelExact);
        let id = BenchmarkId::new("pixel-exact/stream-to-sink", name);
        group.bench_with_input(id, &req, |b, req| {
            b.iter(|| vectorize_to(&mut std::io::sink(), black_box(req)).unwrap())
        });
    }
    group.finish();
}

fn bench_optimizer(c: &mut Criterion) {
    let mut group = c.benchmark_group("optimize_svg");
    let config = OptimizeConfig::default();
    let pixel_svg = pixel_exact::vectorize(&noise(128)).unwrap();
    let spline_svg = vectorize(&VectorizeRequest::new(photo(256)).with_engine(Engine::Spline))
        .unwrap()
        .svg;
    for (name, svg) in [
        ("pixel-exact-noise-128", &pixel_svg),
        ("spline-photo-256", &spline_svg),
    ] {
        group.throughput(Throughput::Bytes(svg.len() as u64));
        group.bench_with_input(name, svg, |b, svg| {
            b.iter(|| optimize_svg(black_box(svg), &config).unwrap())
        });
    }
    group.finish();
}

criterion_group!(benches, bench_engines, bench_optimizer);
criterion_main!(benches);
