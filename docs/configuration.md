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
| Spline input budget | `--spline-max-mp <MP>` | Downscale larger inputs before tracing (default `2`, `0` = off) |

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

## Presets, Profiles and Explicit Options

Options are applied in layers, lowest first. Each layer can change anything
the layers below it set:

1. Defaults
2. `--preset` (vtracer starting point for Spline parameters; never chooses the engine)
3. `--profile` (covecto bundle for a kind of image; may choose the engine)
4. Flags you type yourself (always win)

The order you type the flags in does not matter.

```bash
# Profile sets engine + multiple params; --corner-threshold 120 wins over the profile's 80
covecto input.png --profile icon --corner-threshold 120 -o out.svg

# An explicit engine wins over the profile's choice of spline
covecto input.png --profile icon --engine pixel-exact -o out.svg

# Preset as a starting point, with one parameter changed
covecto photo.jpg --engine spline --preset poster --filter-speckle 8 -o out.svg
```

Unknown names, invalid values and repeated options are errors that name the
option; nothing is silently ignored. The same rules apply to the API's form
fields and to library users of `covecto_core::options::resolve`. See
[`GLOSSARY.md`](https://github.com/codecoradev/covecto/blob/develop/GLOSSARY.md)
and [ADR 0001](https://github.com/codecoradev/covecto/blob/develop/docs/adr/0001-option-precedence-and-strictness.md).