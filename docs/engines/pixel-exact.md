---
title: PixelExact Engine
---

# PixelExact Engine

Crisp, rectilinear SVG from pixel data. Best for icons and pixel art.

## Algorithm

1. **Color quantization** — reduce to unique colors
2. **Flood fill** — find contiguous regions of same color
3. **Boundary tracing** — trace the outline of each region
4. **SVG generation** — emit rectilinear `<path>` elements

## Characteristics

- **Exact pixel boundaries** — no interpolation, no smoothing
- **Right angles only** — all paths use orthogonal segments
- **Small file sizes** — fewer control points than spline output
- **Fast** — O(n) flood fill, no iterative optimization

## Example

```bash
covecto icon-16x16.png --engine pixel-exact -o icon.svg
```

## Limitations

- Not suitable for photographs or gradient images
- Large images can produce many small paths
- No curve smoothing — output is always rectilinear