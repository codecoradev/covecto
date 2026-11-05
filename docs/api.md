---
title: API Reference
---

# API Reference

Start the server:

```bash
covecto serve --port 3000
```

All endpoints are under `/v1/`.

---

## `GET /v1/health`

Health check.

**Response** `200`

```json
{
  "status": "ok",
  "version": "0.1.0"
}
```

---

## `GET /v1/metrics`

Server metrics since startup.

**Response** `200`

```json
{
  "request_count": 42,
  "avg_processing_ms": 23.5
}
```

---

## `POST /v1/vectorize`

Vectorize an image.

**Request** `multipart/form-data`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `file` | binary | ✅ | Image file (PNG, JPEG, WebP, BMP, GIF, TIFF) |
| `engine` | string | — | `auto`, `spline`, `pixel-exact` (default: `auto`) |
| `format` | string | — | `svg`, `pdf`, `eps` (default: `svg`) |
| `preset` | string | — | vtracer preset: `bw`, `poster`, `photo` |
| `profile` | string | — | Profile: `icon`, `logo`, `photo`, `lineart` |
| `optimize` | boolean | — | Run SVG optimization (default: `true`) |
| `optimize_preset` | string | — | `default`, `safe`, `none` |
| `multipass` | boolean | — | Multiple optimization passes |
| `color_precision` | int | — | Color quantization precision (1–32) |
| `filter_speckle` | int | — | Speckle noise threshold |
| `corner_threshold` | int | — | Corner detection (0–180) |
| `splice_threshold` | int | — | Path splice (0–100) |
| `color_mode` | string | — | `color` or `binary` |
| `hierarchical` | string | — | `stacked` or `cutout` |
| `path_simplify` | string | — | `spline`, `polygon`, `none` |
| `layer_difference` | int | — | Layer difference threshold |
| `length_threshold` | float | — | Minimum path length |
| `max_iterations` | int | — | Max quantization iterations |
| `path_precision` | int | — | Coordinate decimal places |
| `spline_max_mp` | float | — | Spline input budget in megapixels; can only lower the server's `COVECTO_SPLINE_MAX_MP` |

**Response** `200` (format=svg)

```json
{
  "svg": "<svg>...</svg>",
  "engine_used": "spline",
  "metadata": {
    "input_size": [128, 128],
    "svg_byte_size": 4520,
    "path_count": 24,
    "processing_time_ms": 12.3,
    "engine_used": "spline"
  }
}
```

**Response** `200` (format=pdf or eps)

Returns raw binary with `Content-Type: application/pdf` or `application/postscript`.

**Error** `400`

```json
{
  "error": "No file provided"
}
```

**Error** `415`: the upload is not in a supported image format.
**Error** `400`: the image format is recognized but the data is corrupt or truncated.

**Error** `503`: the server is saturated (job queue full, or no job slot freed
within `COVECTO_QUEUE_TIMEOUT_SECS`). The response has `Retry-After: 1`; retry later.

**Error** `413`: image width or height exceeds `COVECTO_MAX_DIMENSION`
(checked from the image header, before decoding).

```json
{
  "error": "Image too large: 10000x200 exceeds maximum 8192x8192"
}
```

---

## `POST /v1/optimize`

Optimize an existing SVG.

**Request** `multipart/form-data`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `file` | binary | ✅ | SVG file to optimize |
| `preset` | string | — | `default`, `safe`, `none` (default: `default`) |
| `multipass` | boolean | — | Multiple passes (default: `false`) |
| `multipass_iterations` | int | — | Pass count (default: `10`, capped at `16`) |

**Response** `200`

```json
{
  "svg": "<svg>...</svg>",
  "original_size": 15240,
  "optimized_size": 8120,
  "reduction_pct": 46.7
}
```

---

## Resource Limits

Vectorization runs on a blocking thread pool, so health checks and uploads stay
responsive while heavy jobs run. Jobs beyond the concurrency limit wait in a
bounded queue. When `COVECTO_MAX_CONCURRENCY + COVECTO_MAX_QUEUE` requests are
already admitted, a new request gets `503` with `Retry-After` **before its upload
is read**. A queued request that waits longer than `COVECTO_QUEUE_TIMEOUT_SECS`
also gets `503`. This applies to `/v1/vectorize` and `/v1/optimize`.

| Env var | Default | Description |
|---------|---------|-------------|
| `COVECTO_MAX_CONCURRENCY` | CPU count | Max vectorize/optimize jobs running at once |
| `COVECTO_MAX_QUEUE` | 4 × concurrency | Max requests waiting for a slot; beyond that `503`. `0` = no waiting |
| `COVECTO_QUEUE_TIMEOUT_SECS` | `30` | Max wait for a slot before `503` |
| `COVECTO_MAX_DIMENSION` | `8192` | Max image width or height (px); larger uploads get `413` |
| `COVECTO_SPLINE_MAX_MP` | `2` | Spline input budget (megapixels); larger images are downscaled before tracing. Clients may only lower it. `0` = no server cap |

The request body limit is 50 MB.

## OpenAPI Spec

The full [OpenAPI 3.1 specification](https://github.com/codecoradev/covecto/blob/main/openapi.yaml) is available in the repository.
