# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-07-22

### Added
- Dual-engine vectorization: pixel-exact and spline (vtracer)
- CLI with recursive scan, glob patterns, output templates, dry-run
- JSON output (`--json`, `--json-pretty`)
- Progress bar (indicatif)
- Rayon parallel batch processing
- Streaming SVG output (write directly to disk)
- REST API with `/v1/vectorize`, `/v1/optimize`, `/v1/health`, `/v1/metrics`
- Docker multi-stage build + CI-based binary image
- GitHub Actions CI (check, format, clippy, test, build)
- GitHub Actions release (cross-compile, crates.io, Docker)

[Unreleased]: https://github.com/codecoradev/covecto/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/codecoradev/covecto/releases/tag/v0.1.0
