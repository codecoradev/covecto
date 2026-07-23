---
title: Quick Start
---

# Quick Start

## Your First Vectorization

```bash
# Auto-detect the best engine
covecto input.png -o output.svg
```

Covecto analyzes your image and picks the optimal engine automatically.

## Choosing an Engine

```bash
# Icons and pixel art — crisp rectilinear paths
covecto icon.png --engine pixel-exact -o icon.svg

# Photos — smooth Bézier curves
covecto photo.jpg --engine spline -o photo.svg

# Let Covecto decide (default)
covecto image.png -o image.svg
```

## Using Profiles

Profiles tune multiple parameters at once for common use cases:

```bash
# Small icons (4 colors, tight corners)
covecto favicon.png --profile icon -o favicon.svg

# Logos with transparency (8 colors, cutout layering)
covecto logo.png --profile logo -o logo.svg

# Photographs (10 colors, stacked layers)
covecto photo.jpg --profile photo -o photo.svg

# Line drawings (binary, cutout)
covecto sketch.png --profile lineart -o sketch.svg
```

See the [Profiles guide](/profiles) for all options.

## Batch Processing

```bash
# Entire directory
covecto ./icons/ -o ./output/

# Glob pattern
covecto "screenshots/*.png" -o ./vectorized/

# Recursive scan
covecto ./assets/ --recursive -o ./svg-output/

# Custom filename template
covecto ./images/ --output-template "vectorized-{stem}.svg" -o ./out/
```

See the [Batch Processing guide](/batch) for more.

## Output Formats

```bash
# SVG (default)
covecto input.png -o output.svg

# PDF
covecto input.png --format pdf -o output.pdf

# EPS
covecto input.png --format eps -o output.eps
```

## Optimization

```bash
# Optimize after vectorization
covecto input.png --optimize -o output.svg

# Safe optimization (preserves visual fidelity)
covecto input.png --optimize --optimize-preset safe -o output.svg

# Multipass for maximum compression
covecto input.png --optimize --multipass --multipass-iterations 20 -o output.svg
```

## Start the API Server

```bash
covecto serve --port 3000

curl -F "file=@input.png" http://localhost:3000/v1/vectorize
```

See the [API Reference](/api) for all endpoints.