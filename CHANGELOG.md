# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed (engine preview)
- `--dry-run` shows the engine a real run would use: it calls core's new `select_engine` (also used by `vectorize`/`vectorize_to`) instead of a size-only guess that ignored `--engine`, `--profile` and the color heuristic. With an `auto` engine each image is decoded (in parallel) and marked `(auto)`; this replaces the "never decode" rule of #21
- **Breaking (CLI):** a batch with failed files, and a dry run with unreadable files, now exits with code 1 (it exited 0 after printing the errors)

### Added (engine preview)
- `covecto_core::select_engine(config, image)`: the Engine that will actually run

### Changed (Options, ADR 0001)
- **Breaking:** Options are resolved in one place, `covecto_core::options::resolve`, for the CLI, the API and library users. Precedence is defaults < Preset < Profile < Explicit options, in any order, as `docs/configuration.md` always said: `--profile icon --corner-threshold 120` now gives 120 (was 80), and `--engine pixel-exact --profile icon` runs PixelExact (the profile used to overwrite it)
- **Breaking:** a Preset is a starting point: `--preset poster --filter-speckle 8` keeps 8 (individual parameters used to be discarded). `VectorizeConfig::spline_preset` is removed; `apply_profile` is no longer public
- **Breaking (API):** unknown fields, invalid values, unknown profiles/presets and repeated fields return `400` naming the field (they were silently ignored). `/v1/optimize` rejects invalid `preset`/`multipass`/`multipass_iterations` values and unknown fields too
- CLI: invalid option values and unknown profiles/presets are reported with the flag name; no flag has a hidden default that counts as explicit

### Fixed (Options)
- `--path-simplify` / `path_simplify` was never passed to the Spline engine and had no effect

### Added (Options)
- `covecto_core::options` (`resolve`, `OPTIONS` catalog), `GLOSSARY.md`, `docs/adr/0001-option-precedence-and-strictness.md`

### Fixed
- API: undecodable uploads now return `415` (unsupported format), `400` (corrupt image) or `413` (over the decoder's memory limits) instead of `500` (#60)
- API: the job queue is bounded: beyond `COVECTO_MAX_CONCURRENCY + COVECTO_MAX_QUEUE` admitted requests, or after `COVECTO_QUEUE_TIMEOUT_SECS` waiting, requests get `503` with `Retry-After`, and a rejected request's upload is never read. Reading the upload is bounded by `COVECTO_UPLOAD_TIMEOUT_SECS` (default 60, `408`), so one slow client holds a place for a bounded time; use proxy-level connection limits against many slow clients (#60)
- Optimizer: removing an element no longer eats the whitespace after it, which may be text (`Hello<title/> world` became `Helloworld`); indentation is still collapsed by the whitespace pass (#60)
- Optimizer: empty `<g>` and `<defs>` are removed in one pass, so `<defs><g/></defs>` no longer survives as `<defs></defs>` without multipass (#60)
- CLI: a mistyped subcommand (`covecto serv`) is reported by clap with a suggestion instead of being treated as an input path (#60)
- SVG optimizer deleted non-empty `<g>`/`<defs>` groups up to the first child's `/>`: translucent PixelExact output lost its opacity group and first path and became malformed XML under the default config (#59)
- `remove_comments` dropped the rest of the document after an unclosed `<!--`; it is now kept as-is (#59)
- `<title>`/`<desc>`/`<metadata>` removal no longer matches tags that merely start with those names (#59)
- Path-number shortening no longer rewrites numbers inside `id="..."` (or any attribute ending in `d`); only a standalone `d` attribute is path data
- API: a panic inside a vectorization job now returns `500` instead of dropping the connection, and frees its concurrency slot
- Optimizer: empty-group and metadata removal are now single linear passes; deeply nested `<g>` input (e.g. via `/v1/optimize`) was quadratic
- Optimizer: path-number shortening no longer garbles non-ASCII text (bytes were copied as Latin-1 chars) and keeps compact path data like `1.0.5` (= 1.0, .5) from merging into `1.5`
- Optimizer: `clean_empty_attributes` only removes standalone `id=""`/`class=""`, no longer leaving a broken `data-` behind
- Optimizer: numbers starting with `.` (`.5`, `-.5`) are parsed as numbers, so compact path data like `M.5.0` no longer loses a coordinate
- PixelExact and engine auto-selection use 64-bit pixel index math (u32 overflowed above ~32k px per side)
- Optimizer: unterminated markup (`<` without `>`) no longer makes empty-element removal quadratic; non-ASCII characters inside `d` values no longer panic; added a deterministic fuzz test over all passes
- Optimizer: multipass iterations are capped at `MAX_MULTIPASS_ITERATIONS` (16); a client-supplied `multipass_iterations` could hold an API job slot for a very long time on nested-comment input
- Optimizer: path numbers inside arc commands (`a`/`A`) are left as-is, since unseparated flags like `0,01.0` cannot be shortened safely
- Optimizer: path-number shortening follows the SVG number grammar (an exponent has no `.`, so `1e1.0` is two numbers); a property test checks with usvg that shortened path data keeps identical geometry
- Optimizer: XML prolog removal no longer strips `<?xml-stylesheet ...?>` processing instructions
- CLI `--optimize` could not be turned off; added `--optimize=false` and `--no-optimize` (#58)
- CLI: `vectorize` is now the default subcommand, so the documented `covecto input.png -o out.svg` works (#58)

### Changed
- **Breaking (library):** `VectorizeConfig` has a new public field `spline_max_megapixels`; struct literals need it or `..Default::default()`
- Spline output for inputs above 2 MP now differs from 0.1.2 (traced downscaled, `viewBox` added); pass `--spline-max-mp 0` / `spline_max_megapixels: Some(0.0)` for the old behaviour
- Auto engine selection no longer always picks PixelExact above ~13 MP: the Spline threshold (0.5% of pixels as unique colors) is capped at 25% of the sampled pixels, so it stays reachable and continuous; images below ~3.3 MP are decided as before. A 16 MP photo now goes to Spline (37 s, 4.8 MB) instead of PixelExact (22 s, 270 MB SVG) (#60)
- Optimizer is skipped for PixelExact output (already minimal; byte-identical result), restoring true streaming in `vectorize_to` (#54)
- Optimizer passes no longer copy the document when they have nothing to change (#54)
- API: vectorize/optimize run on the blocking pool behind a concurrency limit (`COVECTO_MAX_CONCURRENCY`, default CPU count); uploads larger than `COVECTO_MAX_DIMENSION` (default 8192 px per side) are rejected with `413` before decoding (#55)
- Fewer copies: spline input is one memcpy, uploads are not copied, PixelExact uses a bitset visited-set and a reused element buffer; engine auto-selection stops counting colors once the decision is fixed; PixelExact warns on pathological (photo-like) output (#56)

### Added
- Spline input budget (#60): images above `spline_max_megapixels` (default 2 MP) are downscaled before tracing and keep their display size via `viewBox` (with `preserveAspectRatio="none"`). CLI `--spline-max-mp`, API `spline_max_mp` (can only lower the server cap `COVECTO_SPLINE_MAX_MP`). A 16 MP photo: 37 s / 4.8 MB SVG instead of >150 s; `0` restores full-size tracing
- `load_image_from_bytes_with_limit` and `DEFAULT_MAX_DIMENSION` in `covecto-core`; `Limits` and `create_app_state_with_limits` in `covecto-api` (#55)
- Criterion benchmark harness: `cargo bench -p covecto-core` (#57)

## [0.1.2] - 2026-10-15

### Fixed
- Streaming path (`vectorize_to`) now applies SVG optimization when `optimize` is enabled; it was silently skipped, so CLI SVG output was unoptimized (#43)
- PixelExact streaming metadata: `svg_byte_size` off-by-one and `path_count` double-counting translucent paths (#47)

### Changed
- CI (check, clippy, test, build), release workflow and Dockerfile build with `--locked`; `Cargo.lock` synced with the workspace version (#46, #50)
- Parity test between `vectorize()` and `vectorize_to()`: 58 cases, config matrix, ground-truth counter assertions (#47, #49)

## [0.1.1] - 2026-07-24

### Added
- One-line install script: `curl -fsSL https://codecora.dev/covecto/install | sh`
- Documentation site at https://codecora.dev/covecto/docs/
- Cloudflare Worker routing (docs + install)

## [0.1.0] - 2026-07-23

### Added
- Dual-engine vectorization: pixel-exact and spline (vtracer)
- CLI with recursive scan, glob patterns, output templates, dry-run
- JSON output (`--json`, `--json-pretty`)
- Progress bar (indicatif)
- Rayon parallel batch processing
- Streaming SVG output (write directly to disk)
- REST API with `/v1/vectorize`, `/v1/optimize`, `/v1/health`, `/v1/metrics`
- PDF and EPS export formats
- SVG optimization (standalone and post-vectorize)
- Built-in profiles: icon, logo, photo, lineart
- Docker multi-stage build + CI-based binary image (GHCR)
- GitHub Actions CI (check, format, clippy, test, build)
- GitHub Actions release (cross-compile, crates.io, Docker)
- OpenAPI 3.1 specification
- VitePress documentation site (14 pages)
- Cloudflare Pages auto-deploy workflow

### Contributors
- @ajianaz — initial release (#23, #24, #25, #26, #27, #28)

[Unreleased]: https://github.com/codecoradev/covecto/compare/v0.1.2...HEAD
[0.1.2]: https://github.com/codecoradev/covecto/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/codecoradev/covecto/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/codecoradev/covecto/releases/tag/v0.1.0
