---
title: Batch Processing
---

# Batch Processing

Covecto handles multiple files efficiently — with parallel processing via Rayon.

## Directory Mode

```bash
# All images in a directory
covecto ./icons/ -o ./output/
```

Files maintain their original names with the output extension.

## Glob Patterns

```bash
# PNG files only
covecto "screenshots/*.png" -o ./vectorized/

# Multiple patterns
covecto "assets/**/*.png" -o ./svg/
```

## Recursive Scan

```bash
# All images in all subdirectories
covecto ./assets/ --recursive -o ./svg-output/
```

## Output Templates

Customize output filenames with placeholders:

| Placeholder | Description | Example |
|-------------|-------------|--------|
| `{stem}` | Filename without extension | `icon-32x32` |
| `{name}` | Same as stem | `icon-32x32` |
| `{ext}` | Output format extension | `svg` |

```bash
# Prefix
covecto ./icons/ --output-template "vec-{stem}.{ext}" -o ./out/

# Custom directory structure
covecto ./photos/ --output-template "{stem}.{ext}" --output-dir ./vectorized/
```

## Collision Handling

When output filenames collide, Covecto appends an incremental suffix:

``
input/a.png → output/a.svg
subdir/a.png → output/a_1.svg
```

## Parallel Processing

Batch mode uses Rayon for automatic parallelism. The number of threads defaults to your CPU core count.

## JSON Output

Get machine-readable results for pipelines:

```bash
# Compact JSON
covecto ./icons/ --json

# Pretty-printed
covecto ./icons/ --json-pretty

# Example output
[
  {
    "input": "icon-16x16.png",
    "output": "output/icon-16x16.svg",
    "engine": "pixel-exact",
    "metadata": { "path_count": 12, "processing_time_ms": 1.2 }
  }
]
```

## Dry Run

Preview what would be processed:

```bash
covecto ./assets/ --recursive --dry-run
```