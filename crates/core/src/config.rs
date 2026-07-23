use serde::{Deserialize, Serialize};

/// Which vectorization engine to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Engine {
    /// Pixel-exact, no smoothing. Best for icons, pixel art, screenshots.
    PixelExact,
    /// Spline approximation (vtracer-backed). Best for photos, illustrations.
    Spline,
    /// Auto-select based on image characteristics.
    #[default]
    Auto,
}

impl std::fmt::Display for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Engine::PixelExact => write!(f, "pixel-exact"),
            Engine::Spline => write!(f, "spline"),
            Engine::Auto => write!(f, "auto"),
        }
    }
}

impl std::str::FromStr for Engine {
    type Err = crate::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pixel-exact" | "pixel_exact" | "exact" => Ok(Engine::PixelExact),
            "spline" | "curve" => Ok(Engine::Spline),
            "auto" => Ok(Engine::Auto),
            _ => Err(crate::Error::InvalidEngine(s.to_string())),
        }
    }
}

/// Optimization configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OptimizeConfig {
    /// Optimization preset.
    pub preset: OptimizePreset,
    /// Run optimization until output stabilizes.
    pub multipass: bool,
    /// Maximum multipass iterations.
    pub multipass_iterations: usize,
}

/// SVG optimization preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum OptimizePreset {
    #[default]
    Default,
    Safe,
    None,
}

/// vtracer presets, exposed for convenience.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SplinePreset {
    Bw,
    Poster,
    #[default]
    Photo,
}

/// Hierarchical mode for vtracer spline engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum HierarchicalMode {
    #[default]
    Stacked,
    Cutout,
}

/// Path simplification mode for vtracer spline engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PathSimplifyMode {
    #[default]
    Spline,
    Polygon,
    None,
}

/// Full vectorization configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorizeConfig {
    /// Which engine to use.
    pub engine: Engine,
    /// Whether to optimize the output SVG.
    pub optimize: bool,
    /// Optimization settings.
    pub optimize_config: OptimizeConfig,

    // --- Spline engine params (vtracer-backed) ---
    /// vtracer preset (overrides individual params).
    pub spline_preset: Option<SplinePreset>,

    /// Color quantization precision. Higher = more colors preserved. Default: 6.
    pub color_precision: Option<i32>,

    /// Filter speckle noise smaller than this. Default: 4.
    pub filter_speckle: Option<usize>,

    /// Corner detection threshold. Default: 60.
    pub corner_threshold: Option<i32>,

    /// Path splice threshold. Default: 45.
    pub splice_threshold: Option<i32>,

    /// Hierarchical mode (stacked/cutout). Default: stacked.
    pub hierarchical: Option<HierarchicalMode>,

    /// Path simplification mode. Default: spline.
    pub path_simplify_mode: Option<PathSimplifyMode>,

    /// Layer difference threshold. Default: 5.
    pub layer_difference: Option<i32>,

    /// Minimum path length. Default: 5.0.
    pub length_threshold: Option<f64>,

    /// Maximum color quantization iterations. Default: 2.
    pub max_iterations: Option<usize>,

    /// Path coordinate precision (decimal places). Default: None (vtracer default).
    pub path_precision: Option<u32>,

    /// Color mode for vtracer.
    pub color_mode: Option<ColorMode>,
}

/// Color mode for the spline engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ColorMode {
    #[default]
    Color,
    Binary,
}

impl std::str::FromStr for ColorMode {
    type Err = crate::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "color" => Ok(ColorMode::Color),
            "binary" | "bw" => Ok(ColorMode::Binary),
            _ => Err(crate::Error::InvalidConfig(format!(
                "Unknown color mode: {s}"
            ))),
        }
    }
}

impl std::str::FromStr for HierarchicalMode {
    type Err = crate::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "stacked" => Ok(HierarchicalMode::Stacked),
            "cutout" => Ok(HierarchicalMode::Cutout),
            _ => Err(crate::Error::InvalidConfig(format!(
                "Unknown hierarchical mode: {s}"
            ))),
        }
    }
}

impl std::str::FromStr for PathSimplifyMode {
    type Err = crate::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "spline" => Ok(PathSimplifyMode::Spline),
            "polygon" => Ok(PathSimplifyMode::Polygon),
            "none" => Ok(PathSimplifyMode::None),
            _ => Err(crate::Error::InvalidConfig(format!(
                "Unknown path simplify mode: {s}"
            ))),
        }
    }
}

impl std::str::FromStr for SplinePreset {
    type Err = crate::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "bw" => Ok(SplinePreset::Bw),
            "poster" => Ok(SplinePreset::Poster),
            "photo" => Ok(SplinePreset::Photo),
            _ => Err(crate::Error::InvalidConfig(format!(
                "Unknown spline preset: {s}"
            ))),
        }
    }
}

impl Default for VectorizeConfig {
    fn default() -> Self {
        Self {
            engine: Engine::Auto,
            optimize: true,
            optimize_config: OptimizeConfig {
                preset: OptimizePreset::Default,
                multipass: false,
                multipass_iterations: 10,
            },
            spline_preset: None,
            color_precision: None,
            filter_speckle: None,
            corner_threshold: None,
            splice_threshold: None,
            hierarchical: None,
            path_simplify_mode: None,
            layer_difference: None,
            length_threshold: None,
            max_iterations: None,
            path_precision: None,
            color_mode: None,
        }
    }
}

/// A vectorization request.
pub struct VectorizeRequest {
    /// The input image (RGBA).
    pub image: image::RgbaImage,
    /// Configuration.
    pub config: VectorizeConfig,
}

impl VectorizeRequest {
    /// Create a new request with default config.
    pub fn new(image: image::RgbaImage) -> Self {
        Self {
            image,
            config: VectorizeConfig::default(),
        }
    }

    /// Set the engine.
    pub fn with_engine(mut self, engine: Engine) -> Self {
        self.config.engine = engine;
        self
    }

    /// Set optimization on/off.
    pub fn with_optimize(mut self, optimize: bool) -> Self {
        self.config.optimize = optimize;
        self
    }

    /// Set optimization config.
    pub fn with_optimize_config(mut self, config: OptimizeConfig) -> Self {
        self.config.optimize_config = config;
        self
    }

    /// Set full config.
    pub fn with_config(mut self, config: VectorizeConfig) -> Self {
        self.config = config;
        self
    }
}

/// Apply a named profile preset to a vectorization config.
///
/// Profiles override individual vtracer parameters. Useful presets:
/// - `icon` — optimized for small icons (32×32), high detail preservation
/// - `logo` — optimized for logos, high corner threshold, cutout hierarchical
/// - `photo` — optimized for photos, lower precision, stacked hierarchical
/// - `lineart` — optimized for line drawings, binary color mode
pub fn apply_profile(name: &str, config: &mut VectorizeConfig) {
    match name.to_lowercase().as_str() {
        "icon" => {
            config.engine = Engine::Spline;
            config.color_precision = Some(4);
            config.filter_speckle = Some(2);
            config.corner_threshold = Some(80);
            config.splice_threshold = Some(30);
            config.color_mode = Some(ColorMode::Color);
            config.path_simplify_mode = Some(PathSimplifyMode::Spline);
            config.layer_difference = Some(10);
            config.max_iterations = Some(4);
        }
        "logo" => {
            config.engine = Engine::Spline;
            config.color_precision = Some(8);
            config.filter_speckle = Some(4);
            config.corner_threshold = Some(90);
            config.splice_threshold = Some(45);
            config.color_mode = Some(ColorMode::Color);
            config.path_simplify_mode = Some(PathSimplifyMode::Spline);
            config.hierarchical = Some(HierarchicalMode::Cutout);
            config.layer_difference = Some(5);
            config.max_iterations = Some(4);
        }
        "photo" => {
            config.engine = Engine::Spline;
            config.color_precision = Some(10);
            config.filter_speckle = Some(4);
            config.corner_threshold = Some(40);
            config.splice_threshold = Some(45);
            config.color_mode = Some(ColorMode::Color);
            config.path_simplify_mode = Some(PathSimplifyMode::Spline);
            config.hierarchical = Some(HierarchicalMode::Stacked);
            config.layer_difference = Some(5);
            config.max_iterations = Some(2);
        }
        "lineart" => {
            config.engine = Engine::Spline;
            config.color_precision = Some(2);
            config.filter_speckle = Some(8);
            config.corner_threshold = Some(30);
            config.splice_threshold = Some(60);
            config.color_mode = Some(ColorMode::Binary);
            config.path_simplify_mode = Some(PathSimplifyMode::Spline);
            config.hierarchical = Some(HierarchicalMode::Cutout);
            config.layer_difference = Some(10);
            config.max_iterations = Some(2);
        }
        _ => {} // Unknown profile silently ignored
    }
}
