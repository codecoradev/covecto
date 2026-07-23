---
title: Installation
---

# Installation

## Cargo (Recommended)

```bash
cargo install covecto
```

Requires Rust 1.85+.

## Pre-built Binary

Download from [GitHub Releases](https://github.com/codecoradev/covecto/releases):

```bash
# Linux (x86_64)
curl -sL https://github.com/codecoradev/covecto/releases/latest/download/covecto-x86_64-unknown-linux-gnu.tar.gz | tar xz
mv covecto ~/.local/bin/

# macOS (Apple Silicon)
curl -sL https://github.com/codecoradev/covecto/releases/latest/download/covecto-aarch64-apple-darwin.tar.gz | tar xz
mv covecto ~/.local/bin/

# Windows
# Download covecto-x86_64-pc-windows-msvc.zip from the releases page
```

## Docker

```bash
# Pull the image
docker pull ghcr.io/codecoradev/covecto:latest

# Run with docker compose
docker compose up -d

# Or run directly
docker run -p 3000:3000 ghcr.io/codecoradev/covecto:latest
```

See the [Docker guide](/docker) for more details.
