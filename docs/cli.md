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
| `--preset <P>` | vtracer preset: `bw`, `poster`, `photo` | — |
| `--profile <P>` | Profile: `icon`, `logo`, `photo`, `lineart` | — |

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

### Optimization

| Flag | Description | Default |
|------|-------------|---------|
| `--optimize` | Run SVG optimization | `false` |
| `--optimize-preset <P>` | `default`, `safe`, `none` | `default` |
| `--multipass` | Multiple optimization passes | `false` |
| `--multipass-iterations <N>` | Pass count (1+) | `10` |

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
| 0 | Success |
| 1 | Error (file not found, invalid input, etc.) |