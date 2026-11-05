# Covecto

Covecto turns raster images into vector graphics (SVG, PDF, EPS). This glossary fixes the words used for how a vectorization is configured.

## Engines

**Engine**:
The algorithm that turns an image into paths: **PixelExact**, **Spline**, or **Auto**, which picks one of the two for each image.
_Avoid_: mode, backend, tracer

**PixelExact**:
The Engine that reproduces every pixel exactly with rectilinear paths; for icons, pixel art and screenshots.
_Avoid_: exact mode, lossless engine

**Spline**:
The Engine that fits smooth curves to color regions; for photos and illustrations.
_Avoid_: curve mode, vtracer (vtracer is the library behind it, not the concept)

## Configuring a vectorization

**Option**:
One named setting a user supplies for a vectorization, such as a CLI flag or an API form field.
_Avoid_: param, argument, field (when the concept, not the transport, is meant)

**Explicit option**:
An Option the user actually supplied, as opposed to a value filled in by a Profile, a Preset or a default. An Explicit option always wins.
_Avoid_: override, manual setting

**Preset**:
A named starting point for Spline's parameters that comes from vtracer (`bw`, `poster`, `photo`). It never chooses the Engine.
_Avoid_: profile, template

**Profile**:
A named bundle defined by covecto for a kind of image (`icon`, `logo`, `photo`, `lineart`); it may choose the Engine as well as Spline's parameters. The `photo` Profile and the `photo` Preset are different things.
_Avoid_: preset, template, mode
