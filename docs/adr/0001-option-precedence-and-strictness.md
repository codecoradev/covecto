# Option precedence and strictness live in one module in core

Every frontend (CLI, HTTP API, downstream FFI apps) turns the Options a user supplied into a `VectorizeConfig` through one public function, `covecto_core::options::resolve`, which applies **defaults < Preset < Profile < Explicit options** regardless of the order Options arrive in, and rejects anything it cannot apply: unknown Option/Profile/Preset names, values that do not parse, and an Option supplied twice. Before this, the CLI and the API each rebuilt the config, both applied the Profile *after* the user's flags (contradicting the documented precedence), a Preset silently discarded every individual Spline parameter, and the API silently ignored invalid values the CLI rejected.

## Considered Options

- **One function over `(name, value)` pairs plus an `OPTIONS` catalog** (chosen): the deepest interface; both production adapters already speak name/value pairs, so the seam is real.
- **A flexible layered builder with pluggable sources and provenance tracking**: more introspection, but most of its seams had only one adapter, so they were hypothetical.
- **A typed builder (`Options::profile(Icon).corner_threshold(120)`)**: best for Rust callers, but roughly doubles the interface; deferred until a downstream app needs it.

## Consequences

- A Preset is a starting point: it is expanded into concrete Spline parameters at resolve time, so `VectorizeConfig` no longer has a `spline_preset` field and later layers can change any of them. A Preset never chooses the Engine; a Profile may.
- Adapters must pass only Options the user actually supplied, never their own defaults (a clap `default_value = "auto"` would make the Engine look explicit and stop a Profile from choosing it).
- Stricter API input (unknown or duplicate fields, bad values → `400`) and different output for "Profile + explicit flag" combinations are breaking changes, released in 0.2.0.
