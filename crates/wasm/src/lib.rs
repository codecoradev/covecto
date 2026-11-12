//! Browser build of `covecto-core` for `wasm32-unknown-unknown`.
//!
//! The module has no imports, so it loads with
//! `WebAssembly.instantiate(bytes, {})` in a page or a Web Worker, with no JS
//! glue. The caller decodes the image itself (e.g. `createImageBitmap` +
//! `OffscreenCanvas.getImageData`) and passes raw RGBA pixels.
//!
//! # ABI
//!
//! - `covecto_alloc(len) -> ptr`: a buffer of `len` bytes in WASM memory
//!   (0 when `len` is 0).
//! - `covecto_free(ptr, len)`: frees a buffer from `covecto_alloc` or
//!   `covecto_vectorize`; `len` must be the length it was created with.
//! - `covecto_vectorize(pixels, pixels_len, width, height, opts, opts_len,
//!   out) -> svg_ptr`: vectorizes `width * height` RGBA pixels. `opts` is a
//!   UTF-8 string of Options as `name=value` pairs joined by `&` (for example
//!   `engine=auto&preset=photo`; empty for the defaults), resolved by
//!   `covecto_core::options::resolve` with the same names and rules as the
//!   CLI and HTTP API. `out` points to three little-endian `u32`s the call
//!   fills in: `[svg_len, status, engine]`. On success the return value is the
//!   UTF-8 SVG (free it with `covecto_free(svg_ptr, svg_len)`); on failure it
//!   is 0 and `status` says why.
//!
//! `status`: 0 ok, 1 bad input (dimensions or pixel length), 2 bad Options,
//! 3 engine error. `engine`: 0 pixel-exact, 1 spline (the Engine that ran,
//! after `auto` was resolved). A panic traps (`RuntimeError` in JS).

use covecto_core::{Engine, VectorizeRequest, options, vectorize};
use image::RgbaImage;

/// Why a call failed; the value written to `out[1]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Status {
    Ok = 0,
    BadInput = 1,
    BadOptions = 2,
    Engine = 3,
}

/// A successful run: the SVG and the Engine that produced it.
#[derive(Debug)]
pub struct Output {
    pub svg: String,
    pub engine: Engine,
}

/// Splits `name=value&name=value` into pairs. Empty segments are skipped; a
/// segment with no `=` is passed with an empty value so `resolve` names it in
/// its error.
fn parse_options(opts: &str) -> impl Iterator<Item = (&str, &str)> {
    opts.split('&')
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.split_once('=').unwrap_or((s, "")))
        .map(|(k, v)| (k.trim(), v.trim()))
}

/// The safe core of [`covecto_vectorize`], usable from Rust and tests.
pub fn vectorize_rgba(
    pixels: &[u8],
    width: u32,
    height: u32,
    opts: &str,
) -> Result<Output, Status> {
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or(Status::BadInput)?;
    if width == 0 || height == 0 || pixels.len() != expected {
        return Err(Status::BadInput);
    }
    let config = options::resolve(parse_options(opts)).map_err(|_| Status::BadOptions)?;
    let image = RgbaImage::from_raw(width, height, pixels.to_vec()).ok_or(Status::BadInput)?;
    let result =
        vectorize(&VectorizeRequest::new(image).with_config(config)).map_err(|_| Status::Engine)?;
    Ok(Output {
        svg: result.svg,
        engine: result.engine_used,
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn covecto_alloc(len: usize) -> *mut u8 {
    if len == 0 {
        return std::ptr::null_mut();
    }
    let mut buf = Vec::<u8>::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// # Safety
/// `ptr` must come from `covecto_alloc(len)` or be an SVG returned by
/// `covecto_vectorize` with its `svg_len`, and must not be freed twice.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn covecto_free(ptr: *mut u8, len: usize) {
    if ptr.is_null() || len == 0 {
        return;
    }
    // SAFETY: per the contract, `ptr` owns a Vec<u8> allocation of capacity `len`.
    unsafe { drop(Vec::from_raw_parts(ptr, 0, len)) };
}

/// # Safety
/// `pixels` must be valid for `pixels_len` bytes, `opts` for `opts_len` bytes
/// (or null with `opts_len` 0), and `out` for three `u32`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn covecto_vectorize(
    pixels: *const u8,
    pixels_len: usize,
    width: u32,
    height: u32,
    opts: *const u8,
    opts_len: usize,
    out: *mut u32,
) -> *mut u8 {
    if out.is_null() {
        return std::ptr::null_mut();
    }
    let write = |len: u32, status: Status, engine: u32| {
        // SAFETY: `out` is valid for three u32s per the contract.
        unsafe {
            out.write_unaligned(len);
            out.add(1).write_unaligned(status as u32);
            out.add(2).write_unaligned(engine);
        }
    };

    if pixels.is_null() || (opts.is_null() && opts_len != 0) {
        write(0, Status::BadInput, 0);
        return std::ptr::null_mut();
    }
    // SAFETY: both ranges are valid per the contract.
    let pixels = unsafe { std::slice::from_raw_parts(pixels, pixels_len) };
    let opts = if opts_len == 0 {
        ""
    } else {
        match std::str::from_utf8(unsafe { std::slice::from_raw_parts(opts, opts_len) }) {
            Ok(s) => s,
            Err(_) => {
                write(0, Status::BadOptions, 0);
                return std::ptr::null_mut();
            }
        }
    };

    match vectorize_rgba(pixels, width, height, opts) {
        Ok(output) => {
            let engine = match output.engine {
                Engine::PixelExact => 0,
                _ => 1,
            };
            let bytes = output.svg.into_bytes().into_boxed_slice();
            let len = bytes.len();
            let Ok(len32) = u32::try_from(len) else {
                write(0, Status::Engine, engine);
                return std::ptr::null_mut();
            };
            write(len32, Status::Ok, engine);
            Box::into_raw(bytes) as *mut u8
        }
        Err(status) => {
            write(0, status, 0);
            std::ptr::null_mut()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checker(size: u32) -> Vec<u8> {
        let mut px = Vec::with_capacity((size * size * 4) as usize);
        for y in 0..size {
            for x in 0..size {
                let on = (x / 4 + y / 4) % 2 == 0;
                px.extend_from_slice(if on {
                    &[0, 0, 0, 255]
                } else {
                    &[255, 255, 255, 255]
                });
            }
        }
        px
    }

    #[test]
    fn auto_picks_pixel_exact_for_a_small_icon() {
        let out = vectorize_rgba(&checker(16), 16, 16, "").unwrap();
        assert_eq!(out.engine, Engine::PixelExact);
        assert!(out.svg.contains("<path"));
    }

    #[test]
    fn engine_option_forces_spline() {
        let out = vectorize_rgba(&checker(32), 32, 32, "engine=spline&preset=poster").unwrap();
        assert_eq!(out.engine, Engine::Spline);
        assert!(out.svg.starts_with("<?xml") || out.svg.contains("<svg"));
    }

    #[test]
    fn pixel_length_must_match_dimensions() {
        assert_eq!(
            vectorize_rgba(&[0; 15], 2, 2, "").unwrap_err(),
            Status::BadInput
        );
        assert_eq!(vectorize_rgba(&[], 0, 0, "").unwrap_err(), Status::BadInput);
    }

    #[test]
    fn unknown_option_is_rejected() {
        assert_eq!(
            vectorize_rgba(&checker(8), 8, 8, "colour=1").unwrap_err(),
            Status::BadOptions
        );
    }

    #[test]
    fn abi_round_trip() {
        let px = checker(16);
        let opts = b"engine=pixel-exact";
        let mut out = [0u32; 3];
        let svg_ptr = unsafe {
            covecto_vectorize(
                px.as_ptr(),
                px.len(),
                16,
                16,
                opts.as_ptr(),
                opts.len(),
                out.as_mut_ptr(),
            )
        };
        assert!(!svg_ptr.is_null());
        assert_eq!(out[1], Status::Ok as u32);
        assert_eq!(out[2], 0);
        let svg = unsafe { std::slice::from_raw_parts(svg_ptr, out[0] as usize) };
        assert!(std::str::from_utf8(svg).unwrap().contains("<path"));
        unsafe { covecto_free(svg_ptr, out[0] as usize) };

        let p = covecto_alloc(8);
        assert!(!p.is_null());
        unsafe { covecto_free(p, 8) };
    }
}
