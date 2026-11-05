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

## Writing any output format

`render_to` vectorizes straight into a writer in SVG, PDF or EPS. SVG is
streamed; PDF/EPS are converted in memory and written with one `write_all`, so
a failed conversion writes nothing to the writer:

```rust
use covecto_core::{render_to, OutputFormat, VectorizeRequest};

let mut out = Vec::new();
let result = render_to(&mut out, &VectorizeRequest::new(img), OutputFormat::Pdf)?;
std::fs::write("out.pdf", &out)?; // only reached on success
println!("{} bytes, engine {}, MIME {}", result.bytes_written, result.engine_used, result.format.content_type());
```

When writing a file directly, create it only after you are ready to replace
any existing one (the CLI writes to a temporary file and renames it on success).

## Loading images safely

`load_image` decodes a file or bytes to RGBA and checks the size from the
image header first, so a small, highly compressible file cannot expand into a
huge allocation:

```rust
use covecto_core::{load_image, ImageLimits};

let img = load_image(std::path::Path::new("input.png"), ImageLimits::default())?; // 8192 px per side
let upload = load_image(&bytes[..], ImageLimits { max_dimension: 4096 })?;
let trusted = load_image(std::path::Path::new("big.tif"), ImageLimits::UNLIMITED)?;
```

## Building a config from user options

If your app takes options from users (a form, a config file, an FFI map),
let core apply the same rules as the CLI and the API: defaults < Preset <
Profile < explicit options, with errors for unknown names and bad values.

```rust
use covecto_core::{options, vectorize, VectorizeRequest};

let config = options::resolve([("profile", "icon"), ("corner_threshold", "120")])?;
assert_eq!(config.corner_threshold, Some(120)); // explicit option beats the profile

let result = vectorize(&VectorizeRequest::new(img).with_config(config))?;
```

`options::OPTIONS` lists every option with its type and a help line.

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
