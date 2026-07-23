---
title: Vectorization Options
---

# Vectorization Options

Fine-tune the vectorization process with individual parameters.

## Color Controls

| Parameter | CLI Flag | Range | Description |
|-----------|----------|-------|-------------|
| Color precision | `--color-precision` | 1–32 | Higher = more colors in output |
| Color mode | `--color-mode` | `color`, `binary` | Full color or black & white |

```bash
# 16-color output
covecto photo.jpg --color-precision 16 -o out.svg

# Binary (black & white)
covecto sketch.png --color-mode binary -o out.svg
```

## Path Controls

| Parameter | CLI Flag | Range | Description |
|-----------|----------|-------|-------------|
| Corner threshold | `--corner-threshold` | 0–180 | Higher = fewer corners detected |
| Splice threshold | `--splice-threshold` | 0–100 | When to splice paths |
| Length threshold | `--length-threshold` | 0+ | Discard paths shorter than this |
| Path precision | `--path-precision` | 0+ | Decimal places in coordinates |
| Path simplify | `--path-simplify` | `spline`, `polygon`, `none` | Simplification algorithm |

```bash
# Fewer corners, smoother curves
covecto input.png --corner-threshold 100 --splice-threshold 60 -o out.svg

# Simplify to polygons (smaller file)
covecto input.png --path-simplify polygon -o out.svg
```

## Noise & Quality

| Parameter | CLI Flag | Description |
|-----------|----------|-------------|
| Filter speckle | `--filter-speckle <N>` | Remove regions smaller than N pixels |
| Max iterations | `--max-iterations <N>` | Max color quantization iterations |

## Layer Controls

| Parameter | CLI Flag | Values | Description |
|-----------|----------|--------|-------------|
| Hierarchical | `--hierarchical` | `stacked`, `cutout` | Layer compositing mode |
| Layer difference | `--layer-difference <N>` | int | Layer grouping threshold |

```bash
# Stacked layers (good for photos)
covecto photo.jpg --hierarchical stacked --layer-difference 5 -o out.svg

# Cutout layers (good for logos with transparency)
covecto logo.png --hierarchical cutout --layer-difference 10 -o out.svg
```

## Presets vs Individual Params

Presets and profiles override individual params. Order of precedence:

1. Individual CLI flags (highest)
2. `--preset` / `--profile`
3. Defaults (lowest)

```bash
# Profile sets engine + multiple params, --corner-threshold overrides
covecto input.png --profile icon --corner-threshold 120 -o out.svg
```