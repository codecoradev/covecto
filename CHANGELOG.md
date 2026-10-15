# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
