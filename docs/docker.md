---
title: Docker
---

# Docker

## Quick Start

```bash
docker pull ghcr.io/codecoradev/covecto:latest
docker run -p 3000:3000 ghcr.io/codecoradev/covecto:latest
```

The container starts the API server on port 3000 by default.

## Docker Compose

```yaml
services:
  covecto:
    image: ghcr.io/codecoradev/covecto:latest
    ports:
      - "3000:3000"
    environment:
      - RUST_LOG=info
      # Optional resource limits (see API reference)
      # - COVECTO_MAX_CONCURRENCY=2
      # - COVECTO_MAX_QUEUE=8
      # - COVECTO_QUEUE_TIMEOUT_SECS=30
      # - COVECTO_UPLOAD_TIMEOUT_SECS=60
      # - COVECTO_MAX_DIMENSION=8192
      # - COVECTO_SPLINE_MAX_MP=2
    restart: unless-stopped
```

```bash
docker compose up -d
```

## CLI in a Container

Mount a local directory to vectorize files:

```bash
docker run -v $(pwd):/data ghcr.io/codecoradev/covecto:latest \
  vectorize /data/input.png -o /data/output.svg
```

## Health Check

The Docker image exposes a health endpoint:

```bash
curl http://localhost:3000/v1/health
```

## Image Variants

| Tag | Description |
|-----|-------------|
| `latest` | Latest stable release |
| `v0.1.0` | Pinned version |
| `v0.1` | Latest patch for 0.1.x |

Images are available on [GHCR](https://github.com/codecoradev/covecto/pkgs/container/covecto) and [Docker Hub](https://hub.docker.com/r/codecoradev/covecto).

## Build from Source

```bash
# Multi-stage build (compiles Rust in Docker)
docker build -t covecto .

# Run
docker run -p 3000:3000 covecto
```