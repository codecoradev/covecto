---
title: Upgrading to 0.2
---

# Upgrading from 0.1.x to 0.2

0.2 is a performance and robustness release. For most images the output is
byte-identical to 0.1.2. The breaking changes are in the `covecto-core` library
interface and in how the CLI and the API treat options and errors. This page
lists each one with its replacement. The full list is in the
[CHANGELOG](https://github.com/codecoradev/covecto/blob/main/CHANGELOG.md).

## Checklist

| You use… | What to do |
|----------|------------|
| The CLI with SVG output and no `--profile`/`--preset` mixing | Usually nothing. Images over 8192 px per side now need `--max-dimension`, and scripts that check the exit code see `1` when a file fails (see [CLI](#cli)) |
| The CLI or the API with `--profile`/`--preset` plus individual options | Check the results: explicit options now win (see [Options](#options)) |
| The HTTP API | Send only documented fields, once each, and handle `400`, `408`, `413`, `415` and `503` (see [HTTP API](#http-api)) |
| `covecto-core` as a library | Bump to `covecto-core = "0.2"` and apply the [library changes](#library-covecto-core) |
| Photos or other images above 2 MP with the Spline engine | Expect a different, much faster output. Use `--spline-max-mp 0` for the old one (see [Output](#output)) |

## Library (`covecto-core`)

```toml
[dependencies]
covecto-core = "0.2"
```

### Loading images

`load_image(path)` and `load_image_from_bytes(bytes, hint)` are replaced by one
function that also checks the size from the image header before decoding:

```rust
// 0.1
let img = covecto_core::load_image(Path::new("in.png"))?;
let img = covecto_core::load_image_from_bytes(&bytes, Some("png"))?;

// 0.2
use covecto_core::{load_image, ImageLimits};
let img = load_image(Path::new("in.png"), ImageLimits::default())?; // max 8192 px per side
let img = load_image(&bytes[..], ImageLimits::default())?;          // format from the content
let img = load_image(Path::new("in.png"), ImageLimits::UNLIMITED)?; // the old, unchecked behaviour
```

A file is now recognised by its content first and its extension second, so a
JPEG saved as `.png` loads. Bytes are recognised by their content only: the
format hint is gone, so formats without a signature (such as TGA) no longer
load from bytes. Write them to a file with the right extension, or decode them
with the `image` crate yourself. An image over the limit is
`Error::ImageTooLarge`.

### Engine modules

`covecto_core::pixel_exact` and `covecto_core::spline` are private. Choose the
engine in the config and call the public entry points:

```rust
// 0.1
let svg = covecto_core::pixel_exact::vectorize(&img)?;

// 0.2
use covecto_core::{vectorize, Engine, VectorizeRequest};
let result = vectorize(
    &VectorizeRequest::new(img)
        .with_engine(Engine::PixelExact)
        .with_optimize(false), // raw engine output, as the module returned it
)?;
let svg = result.svg;
```

The metadata that `pixel_exact::VectorizeMeta` gave is in
`StreamResult::metadata` (from `vectorize_to`) or `VectorizeResult::metadata`.
To find out which engine `Engine::Auto` will pick without running it, call
`select_engine(&config, &image)`.

### Output formats

`convert_output` returns only the bytes. The MIME type comes from the format:

```rust
// 0.1
let (bytes, mime) = convert_output(&svg, OutputFormat::Pdf)?;

// 0.2
let bytes = convert_output(&svg, OutputFormat::Pdf)?;
let mime = OutputFormat::Pdf.content_type();
```

To go straight from an image to SVG, PDF or EPS, use `render_to`. It streams
SVG, and for PDF/EPS it writes nothing if the conversion fails:

```rust
let mut out = Vec::new();
let result = covecto_core::render_to(&mut out, &request, OutputFormat::Pdf)?;
```

### Presets, profiles and user options

`VectorizeConfig::spline_preset` is removed and `apply_profile` is private.
Build the config with `options::resolve`, which applies the same rules as the
CLI and the API (defaults < Preset < Profile < explicit options):

```rust
// 0.1
let mut config = VectorizeConfig { spline_preset: Some(SplinePreset::Poster), ..Default::default() };
covecto_core::apply_profile("icon", &mut config);
config.corner_threshold = Some(120);

// 0.2
let config = covecto_core::options::resolve([
    ("preset", "poster"),
    ("profile", "icon"),
    ("corner_threshold", "120"),
])?;
```

A serialized 0.1 config that still contains `spline_preset` deserializes
without an error, but the preset is ignored. Move it to `("preset", ...)`.

Unknown names, bad values and repeated options are errors
(`Error::UnknownName`, `Error::InvalidOption`, `Error::DuplicateOption`).
`options::OPTIONS` lists every option with its type and a help line.

### Other changes

- `VectorizeConfig` has a new field, `spline_max_megapixels`. Struct literals
  need it or `..Default::default()`.
- `Error` has three new variants (`InvalidOption`, `UnknownName`,
  `DuplicateOption`). Add an arm or a `_` arm to exhaustive `match`es.

## Options

These rules apply to the CLI, the API and `options::resolve`
([ADR 0001](https://github.com/codecoradev/covecto/blob/main/docs/adr/0001-option-precedence-and-strictness.md)).

- **Explicit options beat profiles.** `--profile icon --corner-threshold 120`
  gives 120 (0.1 gave the profile's 80). `--engine pixel-exact --profile icon`
  runs PixelExact (0.1 let the profile overwrite it).
- **A preset is a starting point.** `--preset poster --filter-speckle 8` keeps
  8 (0.1 dropped individual options when a preset was given).
- `--path-simplify` now has an effect; 0.1 never passed it to the engine.

If you relied on a profile or preset overriding your own options, remove those
options.

## CLI

- Exit code `1` when any file in a batch fails, or when a `--dry-run` with the
  default `auto` engine cannot read a file (0.1 printed the error and exited
  `0`). With an explicit `--engine`, a dry run does not open the files.
- Images over 8192 px per side are rejected before decoding. Raise the limit
  with `--max-dimension <PX>` or turn it off with `--max-dimension 0`.
- `--dry-run` shows the engine a real run would use. With the default `auto`
  engine it decodes each image, so it is slower than in 0.1.
- Output is written to a hidden temporary file next to the target and renamed
  into place. A failed run never truncates or deletes an existing file. An
  interrupted run may leave a `.covecto-<pid>-<n>.tmp` file behind.
- New: `--spline-max-mp`, `--max-dimension`, `--optimize=false` /
  `--no-optimize`. `covecto input.png -o out.svg` works without the
  `vectorize` subcommand.

## HTTP API

| Status | When |
|--------|------|
| `400` | Unknown field, invalid value, unknown profile/preset, repeated field, or a corrupt image. This includes `/v1/optimize` |
| `408` | The upload took longer than `COVECTO_UPLOAD_TIMEOUT_SECS` (default 60) |
| `413` | The image is over `COVECTO_MAX_DIMENSION` (default 8192 px per side) or the decoder's memory limits |
| `415` | The upload is not a supported image format |
| `503` + `Retry-After` | All job slots and queue places are taken, or the request waited longer than `COVECTO_QUEUE_TIMEOUT_SECS` |

0.1 ignored unknown fields and bad values, and answered most failures with
`500`. Clients that sent extra fields (for example, a form that posts every
input) must stop sending them. Clients under load should retry on `503`.

New server settings: `COVECTO_MAX_CONCURRENCY`, `COVECTO_MAX_QUEUE`,
`COVECTO_QUEUE_TIMEOUT_SECS`, `COVECTO_UPLOAD_TIMEOUT_SECS`,
`COVECTO_MAX_DIMENSION` and `COVECTO_SPLINE_MAX_MP`. See [API](./api.md).

## Output

- **Spline above 2 MP:** the image is traced at about 2 MP and scaled back up
  with `viewBox`, so the SVG has the same display size. A 16 MP photo takes
  about 37 s and gives a 4.8 MB SVG, where 0.1 took over 150 s. For the old
  full-size tracing, pass `--spline-max-mp 0` (CLI) or
  `spline_max_megapixels: Some(0.0)` (library). On the API a client can only
  lower the budget; the server operator sets `COVECTO_SPLINE_MAX_MP=0`.
- **Auto above ~13 MP:** photo-like images now go to Spline instead of always
  to PixelExact (which produced SVGs of hundreds of MB). Images below about
  3.3 MP are decided as before.
- Everything else is byte-identical to 0.1.2, apart from the optimizer fixes
  listed in the CHANGELOG (0.1 could corrupt `<g>` groups and path data).
