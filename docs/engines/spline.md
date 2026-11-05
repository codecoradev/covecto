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
- **Input budget** — images above 2 MP are downscaled before tracing (see below)

## Large Images

vtracer's cost grows faster than the pixel count (on a noisy photo: 1 MP ≈ 10 s,
2 MP ≈ 38 s, 4 MP ≈ 108 s, 16 MP did not finish in 150 s). Spline output also
discards fine detail anyway, so inputs above the budget are downscaled
(area averaging) before tracing. The SVG keeps the original `width`/`height`
and maps the traced coordinates with `viewBox`, so it displays at full size.

```bash
covecto big.jpg --engine spline --spline-max-mp 1 -o big.svg   # faster, smaller
covecto big.jpg --engine spline --spline-max-mp 0 -o big.svg   # trace at full size
```

PixelExact is never downscaled.

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