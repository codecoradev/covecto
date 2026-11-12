# covecto-wasm

`covecto-core` for the browser (`wasm32-unknown-unknown`). The module has **no imports**: load it with `WebAssembly.instantiate(bytes, {})` in a page or a Web Worker, no JS glue or bundler plugin needed. Images never leave the user's machine.

## Build

```bash
rustup target add wasm32-unknown-unknown
cargo build -p covecto-wasm --target wasm32-unknown-unknown --release
# -> target/wasm32-unknown-unknown/release/covecto_wasm.wasm
node crates/wasm/tests/smoke.mjs target/wasm32-unknown-unknown/release/covecto_wasm.wasm
```

Each release also ships the prebuilt `covecto-wasm-vX.Y.Z.wasm`.

## ABI

Decode the image yourself (`createImageBitmap` + `OffscreenCanvas.getImageData`) and pass raw RGBA.

| Export | Signature | Notes |
|---|---|---|
| `covecto_alloc` | `(len) -> ptr` | `len` bytes in WASM memory; 0 when `len` is 0 |
| `covecto_free` | `(ptr, len)` | frees a buffer from `covecto_alloc` or an SVG from `covecto_vectorize`; `len` must be the length it was created with |
| `covecto_vectorize` | `(pixels, pixels_len, width, height, opts, opts_len, out) -> svg_ptr` | see below |

`opts` is a UTF-8 string of Options as `name=value` pairs joined by `&`, for example `engine=auto&preset=photo` (empty for the defaults). The names and rules are the CLI's and API's (`docs/configuration.md`): unknown names, bad values and repeats are errors.

`out` points to three little-endian `u32`s that the call fills in: `[svg_len, status, engine]`.

- `status`: `0` ok, `1` bad input (zero size or `pixels_len != width * height * 4`), `2` bad Options, `3` engine error.
- `engine`: `0` pixel-exact, `1` spline: the engine that actually ran, after `auto` was resolved.

On success the return value points to the UTF-8 SVG; free it with `covecto_free(svg_ptr, svg_len)`. On failure it is 0. A panic traps (a `RuntimeError` in JS); catch it and treat it as an engine error.

`ResultMetadata::processing_time_ms` is always 0 here (`wasm32-unknown-unknown` has no clock); time the call with `performance.now()`.

## Example (Web Worker)

```js
const { instance } = await WebAssembly.instantiate(await (await fetch(wasmUrl)).arrayBuffer(), {});
const e = instance.exports;

const { data, width, height } = ctx.getImageData(0, 0, w, h);
const opts = new TextEncoder().encode('engine=auto');

const px = e.covecto_alloc(data.length);
new Uint8Array(e.memory.buffer).set(data, px);
const op = e.covecto_alloc(opts.length);
if (opts.length) new Uint8Array(e.memory.buffer).set(opts, op);
const out = e.covecto_alloc(12);
try {
  const svgPtr = e.covecto_vectorize(px, data.length, width, height, op, opts.length, out);
  const [len, status, engine] = new Uint32Array(e.memory.buffer.slice(out, out + 12));
  if (status !== 0) throw new Error(`covecto status ${status}`);
  const svg = new TextDecoder().decode(new Uint8Array(e.memory.buffer, svgPtr, len));
  e.covecto_free(svgPtr, len);
  // use svg, engine
} finally {
  e.covecto_free(px, data.length);
  e.covecto_free(op, opts.length);
  e.covecto_free(out, 12);
}
```

Large or noisy photos can produce very large SVGs; downscale before calling (Spline already traces above 2 MP at a reduced size, see `spline_max_megapixels`) and cap the output size you accept.
