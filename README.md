<div align="center">

# Covecto

**Dual-engine image vectorization — pixel-exact for icons, smooth curves for art.**

[![CI](https://github.com/codecoradev/covecto/actions/workflows/ci.yml/badge.svg)](https://github.com/codecoradev/covecto/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/covecto.svg)](https://crates.io/crates/covecto)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Docker](https://img.shields.io/badge/docker-ghcr.io-blue)](https://github.com/codecoradev/covecto/pkgs/container/covecto)

</div>

## Why Covecto?

Most vectorizers are one-trick — good at photos or good at icons, never both. Covecto ships two engines and auto-selects the right one:

| Engine | Best For | Algorithm |
|--------|----------|-----------|
| **PixelExact** | Icons, pixel art, screenshots | Contiguous region flood-fill → boundary tracing → rectilinear SVG |
| **Spline** | Photos, illustrations, artwork | Color quantization → contour tracing → Bézier spline fitting |
| **Auto** *(default)* | Everything | Heuristic selection based on image characteristics |

## Install

```bash
# Cargo
cargo install covecto

# Docker
docker pull ghcr.io/codecoradev/covecto:latest

# Pre-built binary (Linux/macOS/Windows)
# → See https://github.com/codecoradev/covecto/releases
```

## Quick Start

```bash
# Auto-detect best engine
covecto input.png -o output.svg

# Force pixel-exact (great for icons)
covecto icon.png --engine pixel-exact -o icon.svg

# Spline with photo preset
covecto photo.jpg --engine spline --preset photo --optimize -o photo.svg

# Use a built-in profile (tunes multiple params)
covecto logo.png --profile logo -o logo.svg

# Batch: entire directory
covecto ./icons/ --engine pixel-exact -o ./output/

# Batch: glob pattern
covecto "screenshots/*.png" -o ./vectorized/

# Output formats
covecto input.png --format pdf -o input.pdf
covecto input.png --format eps -o input.eps
```

## CLI Reference

### Global

```bash
covecto [COMMAND]

Commands:
  vectorize  Vectorize one or more images to SVG/PDF/EPS
  serve      Start HTTP API server

Options:
  -V, --version  Print version
  -h, --help     Print help
```

### Vectorize

```bash
covecto vectorize [OPTIONS] <INPUT>
```

| Flag | Description | Default |
|------|-------------|---------|
| `<INPUT>` | File, directory, or glob pattern | — |
| `-o, --output` | Output file (single) or directory (batch) | stdout |
| `--output-dir` | Output directory (batch alt) | — |
| `--output-template` | Filename template: `{stem}`, `{name}`, `{ext}` | `{stem}.{ext}` |
| `-F, --format` | Output format: `svg`, `pdf`, `eps` | `svg` |
| `-e, --engine` | Engine: `auto`, `spline`, `pixel-exact` | `auto` |
| `--preset` | vtracer preset: `bw`, `poster`, `photo` | — |
| `--profile` | Profile: `icon`, `logo`, `photo`, `lineart` | — |
| `--color-precision` | Color quantization precision (1–32) | — |
| `--filter-speckle` | Filter speckle noise threshold | — |
| `--corner-threshold` | Corner detection (0–180) | — |
| `--splice-threshold` | Path splice threshold (0–100) | — |
| `--color-mode` | `color` or `binary` | — |
| `--hierarchical` | `stacked` or `cutout` | — |
| `--path-simplify` | `spline`, `polygon`, or `none` | — |
| `--optimize` | Run SVG optimization | `false` |
| `--optimize-preset` | `default`, `safe`, or `none` | `default` |
| `--multipass` | Multiple optimization passes | `false` |
| `-R, --recursive` | Recurse into subdirectories | `false` |
| `--json` | JSON output to stdout | `false` |
| `--json-pretty` | Pretty-printed JSON | `false` |
| `--no-progress` | Disable progress bar | `false` |
| `--dry-run` | Preview without processing | `false` |

### Profiles

| Profile | Engine | Color | Use Case |
|---------|--------|-------|----------|
| `icon` | spline | 4 colors | Small icons, favicons |
| `logo` | spline | 8 colors, cutout | Logos with transparency |
| `photo` | spline | 10 colors, stacked | Photographs |
| `lineart` | spline | binary, cutout | Line drawings, sketches |

### Serve

```bash
covecto serve --port 3000
```

## HTTP API

Start the server, then send requests:

```bash
# Vectorize
curl -F "file=@input.png" -F "engine=auto" http://localhost:3000/v1/vectorize

# Vectorize to PDF
curl -F "file=@photo.jpg" -F "format=pdf" http://localhost:3000/v1/vectorize

# Optimize existing SVG
curl -F "file=@input.svg" -F "preset=safe" http://localhost:3000/v1/optimize

# Health check
curl http://localhost:3000/v1/health

# Metrics
curl http://localhost:3000/v1/metrics
```

Full API spec: [openapi.yaml](openapi.yaml)

## Docker

```bash
# Pull
docker pull ghcr.io/codecoradev/covecto:latest

# Run with docker compose
docker compose up -d

# Or directly
docker run -p 3000:3000 ghcr.io/codecoradev/covecto:latest

# Vectorize a local file
docker run -v $(pwd):/data ghcr.io/codecoradev/covecto:latest \
  vectorize /data/input.png -o /data/output.svg
```

## Rust Library

```rust
use covecto_core::{vectorize, VectorizeRequest, Engine, VectorizeConfig};
use image::RgbaImage;

let img = image::open("input.png").unwrap().to_rgba8();
let request = VectorizeRequest::new(img)
    .with_config(VectorizeConfig {
        engine: Engine::Auto,
        ..Default::default()
    });

let result = vectorize(&request).unwrap();
println!("{} paths in {}ms", result.metadata.path_count, result.metadata.processing_time_ms);
```

## Documentation

Full docs at [docs.covecto.dev](https://docs.covecto.dev)

## License

MIT
