---
title: Output Formats
---

# Output Formats

Covecto supports three output formats.

## SVG (Default)

```bash
covecto input.png -o output.svg
```

The native vector format. Best for web use, editing, and further processing.

## PDF

```bash
covecto input.png --format pdf -o output.pdf
```

Embeds the SVG inside a PDF. Useful for print or document inclusion.

## EPS

```bash
covecto input.png --format eps -o output.eps
```

Encapsulated PostScript. Useful for legacy workflows, LaTeX, and print.

## API Format Selection

```bash
curl -F "file=@photo.jpg" -F "format=pdf" http://localhost:3000/v1/vectorize --output photo.pdf
```

For SVG format, the API returns JSON containing the SVG string. For PDF/EPS, it returns raw binary with the appropriate `Content-Type` header.

## Streaming

When writing SVG to a file, Covecto uses streaming I/O — writing directly to disk instead of building the entire SVG in memory. This significantly reduces peak memory usage for large images.