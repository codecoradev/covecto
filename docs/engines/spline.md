---
title: Spline Engine (vtracer)
---

# Spline Engine

Smooth Bézier curves via [vtracer](https://github.com/visioncortex/vtracer). Best for photos and illustrations.

## Algorithm

1. **Color quantization** — reduce to N colors (configurable)
2. **Contour tracing** — find boundaries between color regions
3. **Bézier fitting** — fit smooth cubic Bézier curves to contours
4. **Layering** — optionally stack or cutout layers

## Characteristics

- **Smooth curves** — cubic Bézier splines
- **Configurable** — many parameters for fine-tuning
- **Presets** — `bw`, `poster`, `photo` for quick setup
- **Larger files** — more control points than PixelExact

## Example

```bash
# Default
covecto a photo
covecto photo.jpg --engine spline -o photo.svg

# Black and white
covecto photo.jpg --engine spline --preset bw -o bw.svg

# Poster effect
covecto photo.jpg --engine spline --preset poster -o poster.svg
```

## vtracer Presets

| Preset | Colors | Use Case |
|--------|--------|----------|
| `bw` | 2 (binary) | Line art, silhouettes |
| `poster` | ~10 | Posterization effect |
| `photo` | ~30 | Natural photographs |

## Tuning Parameters

```bash
covecto input.png --engine spline \
  --color-precision 8 \
  --corner-threshold 90 \
  --splice-threshold 45 \
  -o output.svg
```

See [Configuration](/configuration) for all parameters.