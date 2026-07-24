<div align="center">

# Covecto

**Dual-engine image vectorization — pixel-exact for icons, smooth curves for art.**

[![CI](https://github.com/codecoradev/covecto/actions/workflows/ci.yml/badge.svg)](https://github.com/codecoradev/covecto/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/covecto.svg)](https://crates.io/crates/covecto)
[![crates.io downloads](https://img.shields.io/crates/d/covecto.svg)](https://crates.io/crates/covecto)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Docker](https://img.shields.io/badge/docker-ghcr.io-blue)](https://github.com/codecoradev/covecto/pkgs/container/covecto)

[Documentation](https://codecora.dev/covecto/docs) · [Getting Started](https://codecora.dev/covecto/docs/getting-started) · [Changelog](CHANGELOG.md)

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
# One-line install (Linux/macOS/Windows)
curl -fsSL https://codecora.dev/covecto/install | sh

# Cargo
cargo install covecto

# Docker
docker pull ghcr.io/codecoradev/covecto:latest
```

## Quick Start

```bash
# Auto-detect best engine
covecto input.png -o output.svg

# Force pixel-exact (great for icons)
covecto icon.png --engine pixel-exact -o icon.svg

# Spline for photos with optimization
covecto photo.jpg --engine spline --optimize -o photo.svg

# Built-in profile (tunes multiple params at once)
covecto logo.png --profile logo -o logo.svg

# Batch: entire directory
covecto ./icons/ --engine pixel-exact -o ./output/

# Recursive scan with glob
covecto "screenshots/*.png" --recursive -o ./vectorized/

# Output formats: SVG (default), PDF, EPS
covecto input.png --format pdf -o input.pdf
```

## REST API

```bash
# Start the server
covecto serve --port 3000

# Vectorize
curl -F "file=@input.png" -F "engine=auto" http://localhost:3000/v1/vectorize

# Optimize existing SVG
curl -F "file=@input.svg" http://localhost:3000/v1/optimize

# Health check
curl http://localhost:3000/v1/health
```

Full API spec: [openapi.yaml](openapi.yaml)

## Docker

```bash
# API server mode
docker run -p 3000:3000 ghcr.io/codecoradev/covecto:latest

# Vectorize a local file
docker run -v $(pwd):/data ghcr.io/codecoradev/covecto:latest \
  vectorize /data/input.png -o /data/output.svg
```

## Rust Library

```rust
use covecto_core::{vectorize, VectorizeRequest, Engine, VectorizeConfig};

let img = image::open("input.png").unwrap().to_rgba8();
let request = VectorizeRequest::new(img)
    .with_config(VectorizeConfig {
        engine: Engine::Auto,
        ..Default::default()
    });

let result = vectorize(&request).unwrap();
println!("{} paths in {}ms", result.metadata.path_count, result.metadata.processing_time_ms);
```

## License

MIT
