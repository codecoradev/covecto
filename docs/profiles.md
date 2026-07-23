---
title: Profiles
---

# Profiles

Profiles are pre-configured parameter sets for common use cases. They override individual parameters.

## Built-in Profiles

### icon

```bash
covecto favicon.png --profile icon -o favicon.svg
```

| Parameter | Value | Why |
-----------|-------|-----|
| engine | spline | Smooth curves for small icons |
| color-precision | 4 | Few colors for clean paths |
| filter-speckle | 2 | Remove tiny artifacts |
| corner-threshold | 80 | Moderate corner detection |
| path-simplify | spline | Bézier smoothing |

Best for: favicons, app icons, UI icons (16–64px).

### logo

```bash
covecto logo.png --profile logo -o logo.svg
```

| Parameter | Value | Why |
-----------|-------|-----|
| engine | spline | Smooth curves |
| color-precision | 8 | Medium color palette |
| hierarchical | cutout | Handle transparency layers |
| layer-difference | 5 | Tight layer grouping |

Best for: brand logos, wordmarks, emblems with transparency.

### photo

```bash
covecto landscape.jpg --profile photo -o landscape.svg
```

| Parameter | Value | Why |
-----------|-------|-----|
| engine | spline | Smooth curves |
| color-precision | 10 | More colors for detail |
| hierarchical | stacked | Natural layer stacking |
| corner-threshold | 40 | More corners for detail |
| max-iterations | 2 | Fast quantization |

Best for: photographs, natural scenes, complex illustrations.

### lineart

```bash
covecto sketch.png --profile lineart -o sketch.svg
```

| Parameter | Value | Why |
-----------|-------|-----|
| engine | spline | — |
| color-mode | binary | Black and white only |
| filter-speckle | 8 | Aggressive noise removal |
| corner-threshold | 30 | Detect more corners |
| splice-threshold | 60 | Fewer path splices |
| hierarchical | cutout | Clean layer separation |

Best for: line drawings, sketches, signatures, handwriting.
