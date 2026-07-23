---
title: Using as a Crate
---

# Using Covecto as a Rust Crate

`covecto-core` exposes the vectorization engine as a library.

## Add Dependency

```toml
[dependencies]
covecto-core = "0.1"
image = "0.25"
```

## Basic Usage

```rust
use covecto_core::{vectorize, VectorizeRequest, Engine, VectorizeConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let img = image::open("input.png")?.to_rgba8();

    let request = VectorizeRequest::new(img)
        .with_config(VectorizeConfig {
            engine: Engine::Auto,
            ..Default::default()
        });

    let result = vectorize(&request)?;
    println!("Engine: {}", result.engine_used);
    println!("Paths: {}", result.metadata.path_count);
    println!("Time: {}ms", result.metadata.processing_time_ms);

    // Write SVG to file
    std::fs::write("output.svg", &result.svg)?;

    Ok(())
}
```

## Streaming to a Writer

For large images, use `vectorize_to` to write directly to a file without buffering the entire SVG in memory:

```rust
use covecto_core::{vectorize_to, VectorizeRequest, Engine, VectorizeConfig};
use std::fs::File;
use std::io::BufWriter;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let img = image::open("large-photo.jpg")?.to_rgba8();

    let request = VectorizeRequest::new(img)
        .with_config(VectorizeConfig {
            engine: Engine::Spline,
            ..Default::default()
        });

    let file = BufWriter::new(File::create("output.svg")?);
    let metadata = vectorize_to(file, &request)?;

    println!("Wrote {} bytes, {} paths", metadata.svg_byte_size, metadata.path_count);
    Ok(())
}
```

## SVG Optimization

```rust
use covecto_core::{optimize_svg, OptimizeConfig, OptimizePreset};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let svg = std::fs::read_to_string("input.svg")?;
    let optimized = optimize_svg(&svg, &OptimizeConfig {
        preset: OptimizePreset::Default,
        multipass: false,
        multipass_iterations: 10,
    })?;
    std::fs::write("optimized.svg", optimized)?;
    Ok(())
}
```
