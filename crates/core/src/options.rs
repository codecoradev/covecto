//! Building a [`VectorizeConfig`] from the Options a user supplied.
//!
//! Every frontend (the CLI, the HTTP API, downstream apps) collects the
//! Options the user actually supplied as `(name, value)` pairs and calls
//! [`resolve`]. All rules live here, in one place:
//!
//! - **Precedence**: defaults < Preset < Profile < Explicit options. An
//!   Explicit option always wins, whatever order the pairs come in.
//! - **Preset** is a starting point for Spline's parameters; it never
//!   chooses the Engine and never discards other parameters.
//! - **Profile** may choose the Engine as well as Spline's parameters.
//! - **Strictness**: an unknown Option, Profile or Preset name, a value that
//!   does not parse, or the same Option supplied twice is an error naming
//!   the Option. Nothing is silently ignored.
//!
//! Names are canonical snake_case (`color_precision`); kebab-case and a
//! leading `--` are accepted too (`--color-precision`), case-insensitively.
//!
//! See `GLOSSARY.md` for the terms and `docs/adr/0001-option-precedence-and-strictness.md`
//! for why.

use std::collections::BTreeMap;

use crate::config::{
    ColorMode, Engine, HierarchicalMode, OptimizePreset, PathSimplifyMode, SplinePreset,
    VectorizeConfig,
};
use crate::{Error, Result};

/// The type of value an Option takes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OptionKind {
    /// A signed integer.
    Int,
    /// A non-negative integer.
    UInt,
    /// A finite number.
    Float,
    /// `true`/`false` (also `1`/`0`, `yes`/`no`, `on`/`off`).
    Bool,
    /// One of a fixed set of names.
    Choice(&'static [&'static str]),
}

/// One Option [`resolve`] accepts.
#[derive(Debug, Clone, Copy)]
pub struct OptionSpec {
    /// Canonical snake_case name.
    pub name: &'static str,
    /// The type of value it takes.
    pub kind: OptionKind,
    /// One-line description, for help text and docs.
    pub help: &'static str,
}

const ENGINES: &[&str] = &["auto", "spline", "pixel-exact"];
const PROFILES: &[&str] = &["icon", "logo", "photo", "lineart"];
const PRESETS: &[&str] = &["bw", "poster", "photo"];
const OPTIMIZATION_LEVELS: &[&str] = &["default", "safe", "none"];
const COLOR_MODES: &[&str] = &["color", "binary"];
const HIERARCHICAL_MODES: &[&str] = &["stacked", "cutout"];
const PATH_SIMPLIFY_MODES: &[&str] = &["spline", "polygon", "none"];

/// Every Option [`resolve`] accepts. Adapters can use it for help text,
/// docs and forms.
pub const OPTIONS: &[OptionSpec] = &[
    OptionSpec {
        name: "engine",
        kind: OptionKind::Choice(ENGINES),
        help: "Engine (default: auto)",
    },
    OptionSpec {
        name: "profile",
        kind: OptionKind::Choice(PROFILES),
        help: "Profile: a covecto bundle for a kind of image; may choose the Engine",
    },
    OptionSpec {
        name: "preset",
        kind: OptionKind::Choice(PRESETS),
        help: "Preset: vtracer starting point for Spline parameters",
    },
    OptionSpec {
        name: "optimize",
        kind: OptionKind::Bool,
        help: "Optimize the SVG after vectorization (default: true)",
    },
    OptionSpec {
        name: "optimize_preset",
        kind: OptionKind::Choice(OPTIMIZATION_LEVELS),
        help: "Optimization level (default: default)",
    },
    OptionSpec {
        name: "multipass",
        kind: OptionKind::Bool,
        help: "Repeat optimization until the SVG stops shrinking (default: false)",
    },
    OptionSpec {
        name: "multipass_iterations",
        kind: OptionKind::UInt,
        help: "Maximum optimization passes (default: 10, capped at 16)",
    },
    OptionSpec {
        name: "color_precision",
        kind: OptionKind::Int,
        help: "Spline: color quantization precision (higher = more colors)",
    },
    OptionSpec {
        name: "filter_speckle",
        kind: OptionKind::UInt,
        help: "Spline: discard patches smaller than this many pixels",
    },
    OptionSpec {
        name: "corner_threshold",
        kind: OptionKind::Int,
        help: "Spline: corner detection threshold in degrees",
    },
    OptionSpec {
        name: "splice_threshold",
        kind: OptionKind::Int,
        help: "Spline: path splice threshold in degrees",
    },
    OptionSpec {
        name: "color_mode",
        kind: OptionKind::Choice(COLOR_MODES),
        help: "Spline: color or binary",
    },
    OptionSpec {
        name: "hierarchical",
        kind: OptionKind::Choice(HIERARCHICAL_MODES),
        help: "Spline: stacked or cutout layers",
    },
    OptionSpec {
        name: "path_simplify",
        kind: OptionKind::Choice(PATH_SIMPLIFY_MODES),
        help: "Spline: path simplification",
    },
    OptionSpec {
        name: "layer_difference",
        kind: OptionKind::Int,
        help: "Spline: color difference between layers",
    },
    OptionSpec {
        name: "length_threshold",
        kind: OptionKind::Float,
        help: "Spline: minimum path length",
    },
    OptionSpec {
        name: "max_iterations",
        kind: OptionKind::UInt,
        help: "Spline: maximum color quantization iterations",
    },
    OptionSpec {
        name: "path_precision",
        kind: OptionKind::UInt,
        help: "Spline: decimal places in path coordinates",
    },
    OptionSpec {
        name: "spline_max_mp",
        kind: OptionKind::Float,
        help: "Spline: downscale inputs above this many megapixels (default: 2, 0 = off)",
    },
];

/// Older spellings still accepted, mapped to their canonical name.
const ALIASES: &[(&str, &str)] = &[
    ("path_simplify_mode", "path_simplify"),
    ("spline_max_megapixels", "spline_max_mp"),
];

/// Turn the Options a user supplied into a complete [`VectorizeConfig`].
///
/// `supplied` must contain only Options the user actually gave (never an
/// adapter's own defaults), as `(name, raw value)` pairs. Their order does
/// not matter. Precedence is defaults < Preset < Profile < Explicit options.
///
/// # Errors
/// - [`Error::UnknownName`] for an unknown Option, Profile or Preset name,
///   listing the valid names.
/// - [`Error::InvalidOption`] for a value that does not parse.
/// - [`Error::DuplicateOption`] when the same Option is supplied twice.
pub fn resolve<'a, I>(supplied: I) -> Result<VectorizeConfig>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    let mut explicit: BTreeMap<&'static str, &'a str> = BTreeMap::new();
    for (name, value) in supplied {
        let spec = lookup(name)?;
        if explicit.insert(spec.name, value).is_some() {
            return Err(Error::DuplicateOption(spec.name));
        }
    }

    let mut config = VectorizeConfig::default();

    if let Some(raw) = explicit.get("preset") {
        apply_preset(&mut config, parse_preset(raw)?);
    }
    if let Some(raw) = explicit.get("profile") {
        apply_profile(&mut config, raw)?;
    }
    for (&name, &raw) in &explicit {
        if name != "preset" && name != "profile" {
            apply_explicit(&mut config, name, raw)?;
        }
    }
    Ok(config)
}

/// Canonical form of an Option name: no leading `--`, lowercase, `_` for `-`.
fn canonical(name: &str) -> String {
    name.trim()
        .trim_start_matches("--")
        .to_ascii_lowercase()
        .replace('-', "_")
}

fn lookup(name: &str) -> Result<&'static OptionSpec> {
    let canon = canonical(name);
    let canon = ALIASES
        .iter()
        .find(|(alias, _)| *alias == canon)
        .map_or(canon.as_str(), |(_, to)| *to);
    OPTIONS
        .iter()
        .find(|spec| spec.name == canon)
        .ok_or_else(|| Error::UnknownName {
            what: "option",
            name: name.to_string(),
            valid: OPTIONS.iter().map(|spec| spec.name).collect(),
        })
}

fn spec(name: &'static str) -> &'static OptionSpec {
    OPTIONS
        .iter()
        .find(|spec| spec.name == name)
        .expect("every Option applied here is in OPTIONS")
}

fn invalid(option: &'static str, value: &str) -> Error {
    let expected = match spec(option).kind {
        OptionKind::Int => "an integer".to_string(),
        OptionKind::UInt => "a non-negative integer".to_string(),
        OptionKind::Float => "a number".to_string(),
        OptionKind::Bool => "true or false".to_string(),
        OptionKind::Choice(names) => format!("one of {}", names.join(", ")),
    };
    Error::InvalidOption {
        option,
        value: value.to_string(),
        expected,
    }
}

fn parse_num<T: std::str::FromStr>(option: &'static str, raw: &str) -> Result<T> {
    raw.trim().parse().map_err(|_| invalid(option, raw))
}

fn parse_float(option: &'static str, raw: &str) -> Result<f64> {
    let v: f64 = parse_num(option, raw)?;
    if v.is_finite() {
        Ok(v)
    } else {
        Err(invalid(option, raw))
    }
}

fn parse_bool(option: &'static str, raw: &str) -> Result<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(invalid(option, raw)),
    }
}

fn parse_choice<T: std::str::FromStr>(option: &'static str, raw: &str) -> Result<T> {
    raw.trim().parse().map_err(|_| invalid(option, raw))
}

fn parse_preset(raw: &str) -> Result<SplinePreset> {
    raw.trim().parse().map_err(|_| Error::UnknownName {
        what: "preset",
        name: raw.to_string(),
        valid: PRESETS.to_vec(),
    })
}

/// Fill Spline's parameters from a vtracer Preset (a starting point only:
/// later layers may change any of them). A Preset never chooses the Engine.
pub(crate) fn apply_preset(config: &mut VectorizeConfig, preset: SplinePreset) {
    let vc = vtracer::Config::from_preset(match preset {
        SplinePreset::Bw => vtracer::Preset::Bw,
        SplinePreset::Poster => vtracer::Preset::Poster,
        SplinePreset::Photo => vtracer::Preset::Photo,
    });
    config.color_mode = Some(match vc.color_mode {
        vtracer::ColorMode::Color => ColorMode::Color,
        vtracer::ColorMode::Binary => ColorMode::Binary,
    });
    config.hierarchical = Some(match vc.hierarchical {
        vtracer::Hierarchical::Stacked => HierarchicalMode::Stacked,
        vtracer::Hierarchical::Cutout => HierarchicalMode::Cutout,
    });
    config.path_simplify_mode = Some(match vc.mode {
        visioncortex::PathSimplifyMode::Spline => PathSimplifyMode::Spline,
        visioncortex::PathSimplifyMode::Polygon => PathSimplifyMode::Polygon,
        visioncortex::PathSimplifyMode::None => PathSimplifyMode::None,
    });
    config.filter_speckle = Some(vc.filter_speckle);
    config.color_precision = Some(vc.color_precision);
    config.layer_difference = Some(vc.layer_difference);
    config.corner_threshold = Some(vc.corner_threshold);
    config.length_threshold = Some(vc.length_threshold);
    config.max_iterations = Some(vc.max_iterations);
    config.splice_threshold = Some(vc.splice_threshold);
    config.path_precision = vc.path_precision;
}

/// Apply a Profile. Unlike a Preset, a Profile may choose the Engine.
fn apply_profile(config: &mut VectorizeConfig, raw: &str) -> Result<()> {
    struct ProfileValues {
        color_precision: i32,
        filter_speckle: usize,
        corner_threshold: i32,
        splice_threshold: i32,
        color_mode: ColorMode,
        hierarchical: Option<HierarchicalMode>,
        layer_difference: i32,
        max_iterations: usize,
    }
    let p = match raw.trim().to_ascii_lowercase().as_str() {
        "icon" => ProfileValues {
            color_precision: 4,
            filter_speckle: 2,
            corner_threshold: 80,
            splice_threshold: 30,
            color_mode: ColorMode::Color,
            hierarchical: None,
            layer_difference: 10,
            max_iterations: 4,
        },
        "logo" => ProfileValues {
            color_precision: 8,
            filter_speckle: 4,
            corner_threshold: 90,
            splice_threshold: 45,
            color_mode: ColorMode::Color,
            hierarchical: Some(HierarchicalMode::Cutout),
            layer_difference: 5,
            max_iterations: 4,
        },
        "photo" => ProfileValues {
            color_precision: 10,
            filter_speckle: 4,
            corner_threshold: 40,
            splice_threshold: 45,
            color_mode: ColorMode::Color,
            hierarchical: Some(HierarchicalMode::Stacked),
            layer_difference: 5,
            max_iterations: 2,
        },
        "lineart" => ProfileValues {
            color_precision: 2,
            filter_speckle: 8,
            corner_threshold: 30,
            splice_threshold: 60,
            color_mode: ColorMode::Binary,
            hierarchical: Some(HierarchicalMode::Cutout),
            layer_difference: 10,
            max_iterations: 2,
        },
        _ => {
            return Err(Error::UnknownName {
                what: "profile",
                name: raw.to_string(),
                valid: PROFILES.to_vec(),
            });
        }
    };
    config.engine = Engine::Spline;
    config.color_precision = Some(p.color_precision);
    config.filter_speckle = Some(p.filter_speckle);
    config.corner_threshold = Some(p.corner_threshold);
    config.splice_threshold = Some(p.splice_threshold);
    config.color_mode = Some(p.color_mode);
    config.path_simplify_mode = Some(PathSimplifyMode::Spline);
    if let Some(h) = p.hierarchical {
        config.hierarchical = Some(h);
    }
    config.layer_difference = Some(p.layer_difference);
    config.max_iterations = Some(p.max_iterations);
    Ok(())
}

fn apply_explicit(config: &mut VectorizeConfig, name: &'static str, raw: &str) -> Result<()> {
    match name {
        "engine" => config.engine = parse_choice::<Engine>(name, raw)?,
        "optimize" => config.optimize = parse_bool(name, raw)?,
        "optimize_preset" => {
            config.optimize_config.preset = parse_choice::<OptimizePreset>(name, raw)?
        }
        "multipass" => config.optimize_config.multipass = parse_bool(name, raw)?,
        "multipass_iterations" => {
            config.optimize_config.multipass_iterations = parse_num(name, raw)?
        }
        "color_precision" => config.color_precision = Some(parse_num(name, raw)?),
        "filter_speckle" => config.filter_speckle = Some(parse_num(name, raw)?),
        "corner_threshold" => config.corner_threshold = Some(parse_num(name, raw)?),
        "splice_threshold" => config.splice_threshold = Some(parse_num(name, raw)?),
        "color_mode" => config.color_mode = Some(parse_choice(name, raw)?),
        "hierarchical" => config.hierarchical = Some(parse_choice(name, raw)?),
        "path_simplify" => config.path_simplify_mode = Some(parse_choice(name, raw)?),
        "layer_difference" => config.layer_difference = Some(parse_num(name, raw)?),
        "length_threshold" => config.length_threshold = Some(parse_float(name, raw)?),
        "max_iterations" => config.max_iterations = Some(parse_num(name, raw)?),
        "path_precision" => config.path_precision = Some(parse_num(name, raw)?),
        "spline_max_mp" => config.spline_max_megapixels = Some(parse_float(name, raw)?),
        other => unreachable!("Option {other} is in OPTIONS but not applied"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(pairs: &[(&str, &str)]) -> VectorizeConfig {
        resolve(pairs.iter().copied()).unwrap()
    }

    #[test]
    fn no_options_is_the_default_config() {
        let c = ok(&[]);
        let d = VectorizeConfig::default();
        assert_eq!(c.engine, d.engine);
        assert_eq!(c.optimize, d.optimize);
        assert_eq!(c.color_precision, None);
    }

    #[test]
    fn explicit_option_beats_profile() {
        // docs/configuration.md: `--profile icon --corner-threshold 120` -> 120.
        let c = ok(&[("profile", "icon"), ("corner_threshold", "120")]);
        assert_eq!(c.corner_threshold, Some(120));
        assert_eq!(
            c.color_precision,
            Some(4),
            "rest of the Profile still applies"
        );
        // Order of the pairs does not matter.
        let c = ok(&[("corner_threshold", "120"), ("profile", "icon")]);
        assert_eq!(c.corner_threshold, Some(120));
    }

    #[test]
    fn explicit_engine_beats_profile_engine() {
        assert_eq!(ok(&[("profile", "icon")]).engine, Engine::Spline);
        let c = ok(&[("engine", "pixel-exact"), ("profile", "icon")]);
        assert_eq!(c.engine, Engine::PixelExact);
    }

    #[test]
    fn preset_is_a_starting_point() {
        let poster = ok(&[("preset", "poster")]);
        assert_eq!(poster.color_precision, Some(8));
        assert_eq!(
            poster.engine,
            Engine::Auto,
            "a Preset never chooses the Engine"
        );
        // Individual parameters are no longer discarded by a Preset.
        let c = ok(&[("preset", "poster"), ("filter_speckle", "9")]);
        assert_eq!(c.filter_speckle, Some(9));
        assert_eq!(c.color_precision, Some(8));
        // Profile sits above Preset.
        let c = ok(&[("preset", "poster"), ("profile", "icon")]);
        assert_eq!(c.color_precision, Some(4));
        assert_eq!(
            c.length_threshold,
            Some(4.0),
            "Preset values the Profile leaves alone stay"
        );
    }

    #[test]
    fn names_are_normalized_and_aliases_accepted() {
        let c = ok(&[
            ("--Color-Precision", "3"),
            ("path-simplify-mode", "polygon"),
            ("spline_max_megapixels", "0.5"),
        ]);
        assert_eq!(c.color_precision, Some(3));
        assert_eq!(c.path_simplify_mode, Some(PathSimplifyMode::Polygon));
        assert_eq!(c.spline_max_megapixels, Some(0.5));
    }

    #[test]
    fn every_option_is_applied() {
        let samples = |kind: OptionKind| match kind {
            OptionKind::Int | OptionKind::UInt => "3",
            OptionKind::Float => "1.5",
            OptionKind::Bool => "true",
            OptionKind::Choice(names) => names[0],
        };
        for spec in OPTIONS {
            resolve([(spec.name, samples(spec.kind))])
                .unwrap_or_else(|e| panic!("{}: {e}", spec.name));
        }
    }

    #[test]
    fn invalid_values_name_the_option() {
        for (name, value) in [
            ("color_mode", "purple"),
            ("filter_speckle", "-1"),
            ("corner_threshold", "abc"),
            ("optimize", "maybe"),
            ("optimize_preset", "aggressive"),
            ("length_threshold", "NaN"),
            ("engine", "magic"),
        ] {
            match resolve([(name, value)]) {
                Err(Error::InvalidOption {
                    option, value: v, ..
                }) => {
                    assert_eq!(option, canonical(name));
                    assert_eq!(v, value);
                }
                other => panic!("{name}={value}: {other:?}"),
            }
        }
    }

    #[test]
    fn unknown_names_list_the_valid_ones() {
        match resolve([("profile", "icn")]) {
            Err(Error::UnknownName { what, valid, .. }) => {
                assert_eq!(what, "profile");
                assert!(valid.contains(&"icon"));
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            resolve([("preset", "sketch")]),
            Err(Error::UnknownName { what: "preset", .. })
        ));
        match resolve([("colour_precision", "4")]) {
            Err(Error::UnknownName { what, valid, .. }) => {
                assert_eq!(what, "option");
                assert!(valid.contains(&"color_precision"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn duplicate_options_are_rejected_across_spellings() {
        assert!(matches!(
            resolve([("color_precision", "4"), ("--color-precision", "5")]),
            Err(Error::DuplicateOption("color_precision"))
        ));
    }

    #[test]
    fn error_messages_read_well() {
        let e = resolve([("color_mode", "purple")]).unwrap_err().to_string();
        assert!(
            e.contains("color_mode") && e.contains("color, binary"),
            "{e}"
        );
        let e = resolve([("profile", "icn")]).unwrap_err().to_string();
        assert!(
            e.contains("icn") && e.contains("icon, logo, photo, lineart"),
            "{e}"
        );
    }
}
