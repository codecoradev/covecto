---
title: CLI Reference
---

# CLI Reference

```
covecto [COMMAND]

Commands:
  vectorize  Vectorize one or more images to SVG/PDF/EPS
  serve      Start HTTP API server

Options:
  -V, --version  Print version
  -h, --help     Print help
```

## vectorize

```
covecto vectorize [OPTIONS] <INPUT>
```

### Positional

| Arg | Description |
|-----|-------------|
| `<INPUT>` | Input file, directory, or glob pattern (`"icons/*.png"`) |

### Output

| Flag | Description | Default |
|------|-------------|---------|
| `-o, --output <PATH>` | Output file (single) or directory (batch) | stdout |
| `--output-dir <DIR>` | Output directory (batch alternative) | — |
| `--output-template <TPL>` | Filename template: `{stem}`, `{name}`, `{ext}` | `{stem}.{ext}` |
| `-F, --format <FMT>` | `svg`, `pdf`, `eps` | `svg` |

### Engine

| Flag | Description | Default |
|------|-------------|---------|
| `-e, --engine <E>` | `auto`, `spline`, `pixel-exact` | `auto` |
| `--preset <P>` | Preset (vtracer starting point for Spline parameters): `bw`, `poster`, `photo` | — |
| `--profile <P>` | Profile (may also choose the engine): `icon`, `logo`, `photo`, `lineart`. Flags you type win over it | — |

### Vectorization Parameters

| Flag | Type | Description |
|------|------|-------------|
| `--color-precision <N>` | 1–32 | Color quantization precision (higher = more colors) |
| `--filter-speckle <N>` | int | Filter speckle noise smaller than N |
| `--corner-threshold <N>` | 0–180 | Corner detection (higher = fewer corners) |
| `--splice-threshold <N>` | 0–100 | Path splice threshold |
| `--color-mode <M>` | enum | `color` or `binary` |
| `--hierarchical <M>` | enum | `stacked` or `cutout` |
| `--path-simplify <M>` | enum | `spline`, `polygon`, or `none` |
| `--layer-difference <N>` | int | Layer difference threshold |
| `--length-threshold <F>` | float | Minimum path length |
| `--max-iterations <N>` | int | Max color quantization iterations |
| `--path-precision <N>` | int | Path coordinate decimal places |
| `--spline-max-mp <MP>` | float | Spline: downscale inputs above this many megapixels before tracing (default `2`, `0` = full size) |

### Optimization

| Flag | Description | Default |
|------|-------------|---------|
| `--optimize[=BOOL]` | Run SVG optimization (`--optimize=false` to disable) | `true` |
| `--no-optimize` | Disable SVG optimization | — |
| `--optimize-preset <P>` | Optimization level: `default`, `safe`, `none` | `default` |
| `--multipass` | Multiple optimization passes | `false` |
| `--multipass-iterations <N>` | Pass count (1+, capped at 16) | `10` |

The optimizer is skipped for PixelExact output: it is already minimal, so
streaming to disk stays fully incremental.

### Batch & UX

| Flag | Description | Default |
|------|-------------|---------|
| `-R, --recursive` | Recurse into subdirectories | `false` |
| `--json` | JSON results to stdout | `false` |
| `--json-pretty` | Pretty-printed JSON | `false` |
| `--no-progress` | Disable progress bar/spinner | `false` |
| `--dry-run` | Preview without processing | `false` |

## serve

```
covecto serve [OPTIONS]
```

| Flag | Description | Default |
|------|-------------|---------|
| `-p, --port <PORT>` | Listen port | `3000` |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success: every file was converted (with `--dry-run`: listed; files are only read, and so checked, for the `auto` engine) |
| 1 | Error: invalid input or options, or at least one file in a batch failed (the others are still converted and listed) |