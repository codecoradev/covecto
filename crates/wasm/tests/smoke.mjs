// Loads a covecto-wasm build the way a browser Web Worker does (no imports,
// no JS glue) and checks the ABI end to end.
//
//   node crates/wasm/tests/smoke.mjs target/wasm32-unknown-unknown/release/covecto_wasm.wasm
import { readFileSync } from 'node:fs';
import assert from 'node:assert/strict';

const [wasmPath] = process.argv.slice(2);
assert.ok(wasmPath, 'usage: smoke.mjs <path to covecto_wasm.wasm>');

const mod = await WebAssembly.compile(readFileSync(wasmPath));
assert.deepEqual(WebAssembly.Module.imports(mod), [], 'module must have no imports');
const { exports: e } = await WebAssembly.instantiate(mod, {});
for (const name of ['memory', 'covecto_alloc', 'covecto_free', 'covecto_vectorize']) {
	assert.ok(name in e, `missing export ${name}`);
}

function run(w, h, pixel, opts) {
	const px = new Uint8Array(w * h * 4);
	for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) px.set(pixel(x, y), (y * w + x) * 4);
	const o = new TextEncoder().encode(opts);
	const pp = e.covecto_alloc(px.length);
	new Uint8Array(e.memory.buffer).set(px, pp);
	const op = e.covecto_alloc(o.length);
	if (o.length) new Uint8Array(e.memory.buffer).set(o, op);
	const out = e.covecto_alloc(12);
	const svgPtr = e.covecto_vectorize(pp, px.length, w, h, op, o.length, out);
	const [len, status, engine] = new Uint32Array(e.memory.buffer.slice(out, out + 12));
	const svg = svgPtr ? new TextDecoder().decode(new Uint8Array(e.memory.buffer, svgPtr, len)) : '';
	e.covecto_free(pp, px.length);
	e.covecto_free(op, o.length);
	e.covecto_free(out, 12);
	if (svgPtr) e.covecto_free(svgPtr, len);
	return { status, engine, svg };
}

const checker = (x, y) => (((x >> 2) + (y >> 2)) % 2 ? [255, 255, 255, 255] : [0, 0, 0, 255]);
const gradient = (x, y) => [x & 255, (y * 3) & 255, 128, 255];

let r = run(16, 16, checker, '');
assert.equal(r.status, 0);
assert.equal(r.engine, 0, 'auto picks pixel-exact for a small icon');
assert.match(r.svg, /<path/);

r = run(64, 64, checker, 'engine=spline&preset=poster');
assert.equal(r.status, 0);
assert.equal(r.engine, 1);
assert.match(r.svg, /<svg/);

r = run(256, 256, gradient, 'preset=photo');
assert.equal(r.status, 0);
assert.equal(r.engine, 1, 'auto picks spline for a many-color image');

assert.equal(run(8, 8, checker, 'nope=1').status, 2, 'unknown Option is status 2');

console.log('covecto-wasm smoke: ok');
