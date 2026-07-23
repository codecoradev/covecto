---
layout: home

hero:
  name: covecto
  text: Image Vectorization, Simplified
  tagline: Dual-engine CLI — pixel-exact for icons, smooth Bézier curves for art. SVG, PDF, EPS output. Zero dependencies.
  actions:
    - theme: brand
      text: Get Started
      link: /getting-started
    - theme: alt
      text: Install
      link: /install
    - theme: alt
      text: GitHub
      link: https://github.com/codecoradev/covecto

features:
  - icon: 🎯
    title: Dual Engine
    details: PixelExact for crisp icons and pixel art. Spline (vtracer) for smooth photos and illustrations. Auto-selects the best engine.
  - icon: 🖼️
    title: Multi-Format Output
    details: SVG (default), PDF, and EPS. Built-in optimization with SVGO integration. Streaming output for large images.
  - icon: 📦
    title: Single Binary
    details: No Python, no Node, no Docker required. cargo install or download pre-built binaries for Linux, macOS, Windows.
  - icon: ⚡
    title: Batch Processing
    details: Recursive directory scan, glob patterns, parallel processing with Rayon. Custom filename templates.
  - icon: 🎨
    title: Built-in Profiles
    details: Icon, logo, photo, lineart — each profile tunes color count, layering, and path simplification automatically.
  - icon: 🔌
    title: REST API
    details: Start with `covecto serve`. Upload, vectorize, optimize, and health-check via OpenAPI 3.1 endpoints.
  - icon: 🐳
    title: Docker
    details: Pull from GHCR. Standalone or API server mode. Minimal image built from CI binaries.
  - icon: 📚
    title: Rust Library
    details: Use covecto-core as a crate. Programmatic API with VectorizeRequest/VectorizeConfig.
  - icon: 🔧
    title: Fine-Grained Control
    details: Color precision, corner threshold, path splicing, SVG optimization presets, multipass compression.
---

## Engine Comparison

| Engine | Best For | Algorithm |
|--------|----------|-----------|
| **PixelExact** | Icons, pixel art, screenshots | Contiguous region flood-fill → boundary tracing → rectilinear SVG |
| **Spline** | Photos, illustrations, artwork | Color quantization → contour tracing → Bézier spline fitting |
| **Auto** *(default)* | Everything | Heuristic selection based on image characteristics |
