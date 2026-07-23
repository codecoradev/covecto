---
title: Engine Comparison
---

# Engine Comparison

Covecto ships two vectorization engines, each optimized for different image types.

## At a Glance

| | **PixelExact** | **Spline** |
|---|---|---|
| **Algorithm** | Flood-fill → boundary trace → rectilinear SVG | Color quantization → contour trace → Bézier splines |
| **Best for** | Icons, pixel art, screenshots, UI elements | Photos, illustrations, artwork, gradients |
| **Output style** | Sharp right angles, exact pixel boundaries | Smooth curves, organic shapes |
| **Color handling** | Exact color regions | Quantized color palette |
| **Complexity** | O(n) flood fill | Iterative color quantization + spline fitting |

## When to Use Which

### Use PixelExact when:
- The input is a small icon (≤64px)
- You need crisp, pixel-perfect boundaries
- The image has flat colors with sharp edges
- Vectorizing screenshots or UI mockups

### Use Spline when:
- The input is a photograph or illustration
- You need smooth, organic curves
- The image has gradients or many colors
- File size matters more than pixel-perfect accuracy

### Use Auto when:
- You're not sure which engine to pick
- Processing mixed image types in batch
- You want the best default behavior

## Auto-Engine Heuristics

The `auto` engine analyzes the image and selects based on:
1. **Image size** — small images (≤64px) lean toward PixelExact
2. **Color count** — low color count (≤16) suggests PixelExact
3. **Edge density** — high edge density favors Spline
4. **Aspect ratio** — very tall/wide images favor Spline

The heuristic is conservative — when in doubt, it picks Spline.