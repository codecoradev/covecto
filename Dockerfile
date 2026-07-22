# ── Stage 1: Builder ────────────────────────────────────────────────────
FROM rust:1.96-slim AS builder

ARG VERSION=dev

LABEL org.opencontainers.image.version=${VERSION}
LABEL org.opencontainers.image.source=https://github.com/codecoradev/covecto

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Cache dependencies (layer that changes less often)
COPY Cargo.toml Cargo.lock ./

# Create dummy crate sources to pre-build deps
RUN mkdir -p crates/core/src crates/cli/src crates/api/src
RUN echo 'pub fn _placeholder() {}' > crates/core/src/lib.rs
RUN echo 'fn main() {}' > crates/cli/src/main.rs
RUN echo 'pub fn _placeholder() {}' > crates/api/src/lib.rs

# Copy Cargo.toml for each crate (workspace needs these to resolve)
COPY crates/core/Cargo.toml crates/core/Cargo.toml
COPY crates/cli/Cargo.toml crates/cli/Cargo.toml
COPY crates/api/Cargo.toml crates/api/Cargo.toml

# Build dependencies only (will fail on missing code, but deps get cached)
RUN cargo build --release 2>/dev/null || true

# Real source code
COPY crates/ ./crates/

# Touch real sources to invalidate dummy builds and trigger real compilation
RUN touch crates/core/src/lib.rs crates/cli/src/main.rs crates/api/src/lib.rs

RUN cargo build --release --bin covecto

# ── Stage 2: Runtime ────────────────────────────────────────────────────
FROM debian:bookworm-slim

ARG VERSION=dev

LABEL org.opencontainers.image.version=${VERSION}
LABEL org.opencontainers.image.title=covecto
LABEL org.opencontainers.image.description="Dual-engine image-to-SVG vectorization"

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl libssl3 && \
    rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd --system --gid 1000 covecto && \
    useradd --system --uid 1000 --gid covecto --home /data covecto

COPY --from=builder /build/target/release/covecto /usr/local/bin/covecto
RUN chmod +x /usr/local/bin/covecto

ENV COVECTO_PORT=3000

RUN mkdir -p /data && chown covecto:covecto /data

USER covecto

EXPOSE 3000

ENTRYPOINT ["covecto"]
CMD ["serve", "--port", "3000"]
