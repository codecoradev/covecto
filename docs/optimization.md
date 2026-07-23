---
title: Optimization
---

# SVG Optimization

Covecto can optimize SVG output to reduce file size.

## Basic Optimization

```bash
covecto input.png --optimize -o output.svg
```

## Presets

| Preset | Description | Risk |
--------|-------------|------|
| `default` | Balanced compression | Low visual change |
| `safe` | Conservative — preserves visual fidelity | Minimal change |
| `none` | No optimization | — |

```bash
covecto input.png --optimize --optimize-preset safe -o output.svg
```

## Multipass

Run multiple optimization passes for better compression:

```bash
# Default 10 passes
covecto input.png --optimize --multipass -o output.svg

# Custom iteration count
covecto input.png --optimize --multipass --multipass-iterations 20 -o output.svg
```

## Standalone Optimization

Optimize an existing SVG without re-vectorizing:

```bash
# CLI
covecto optimize input.svg -o output.svg

# API
curl -F "file=@input.svg" -F "preset=safe" http://localhost:3000/v1/optimize
```

The response includes size reduction metrics:

```json
{
  "svg": "<optimized svg>",
  "original_size": 15240,
  "optimized_size": 8120,
  "reduction_pct": 46.7
}
```