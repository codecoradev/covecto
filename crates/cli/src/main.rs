use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use covecto_core::{
    ColorMode, Engine, HierarchicalMode, OptimizeConfig, OptimizePreset, OutputFormat,
    PathSimplifyMode, SplinePreset, VectorizeConfig, convert_output, load_image, vectorize,
};
use tracing::info;

/// Covecto — Cora Vectorizer. Dual-engine image-to-SVG vectorization.
#[derive(Parser)]
#[command(name = "covecto", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
enum Commands {
    /// Vectorize one or more images to SVG
    Vectorize {
        /// Input file or directory
        #[arg(value_name = "INPUT")]
        input: PathBuf,
        /// Output file or directory (default: stdout / INPUT/vectorized/)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Output format: svg, pdf, eps (default: svg)
        #[arg(short = 'F', long, default_value = "svg")]
        format: String,
        /// Engine: auto, spline, pixel-exact (default: auto)
        #[arg(short, long, default_value = "auto")]
        engine: String,
        /// vtracer preset: bw, poster, photo (overrides individual params)
        #[arg(long)]
        preset: Option<String>,
        /// Custom preset: icon, logo, photo, lineart (overrides individual params)
        #[arg(long)]
        profile: Option<String>,
        /// Color quantization precision (1-32, higher = more colors)
        #[arg(long)]
        color_precision: Option<i32>,
        /// Filter speckle noise smaller than this size
        #[arg(long)]
        filter_speckle: Option<usize>,
        /// Corner detection threshold (0-180, higher = fewer corners)
        #[arg(long)]
        corner_threshold: Option<i32>,
        /// Path splice threshold (0-100)
        #[arg(long)]
        splice_threshold: Option<i32>,
        /// Color mode: color, binary
        #[arg(long)]
        color_mode: Option<String>,
        /// Hierarchical mode: stacked, cutout
        #[arg(long)]
        hierarchical: Option<String>,
        /// Path simplification: spline, polygon, none
        #[arg(long)]
        path_simplify: Option<String>,
        /// Layer difference threshold
        #[arg(long)]
        layer_difference: Option<i32>,
        /// Minimum path length
        #[arg(long)]
        length_threshold: Option<f64>,
        /// Max color quantization iterations
        #[arg(long)]
        max_iterations: Option<usize>,
        /// Path coordinate precision (decimal places)
        #[arg(long)]
        path_precision: Option<u32>,
        /// Run SVG optimization after vectorization
        #[arg(long, default_value = "true")]
        optimize: bool,
        /// Optimization preset: default, safe, none
        #[arg(long, default_value = "default")]
        optimize_preset: String,
        /// Run multiple optimization passes
        #[arg(long)]
        multipass: bool,
        /// Number of multipass iterations (default: 10)
        #[arg(long, default_value = "10")]
        multipass_iterations: usize,
    },
    /// Start HTTP API server
    Serve {
        /// Port to listen on (default: 3000)
        #[arg(short, long, default_value = "3000")]
        port: u16,
    },
}

fn parse_engine(s: &str) -> anyhow::Result<Engine> {
    s.parse::<Engine>().map_err(|e| anyhow::anyhow!(e.to_string()))
}

fn parse_spline_preset(s: &str) -> anyhow::Result<SplinePreset> {
    s.parse::<SplinePreset>().map_err(|e| anyhow::anyhow!(e.to_string()))
}

fn parse_opt_preset(s: &str) -> OptimizePreset {
    match s.to_lowercase().as_str() {
        "safe" => OptimizePreset::Safe,
        "none" => OptimizePreset::None,
        _ => OptimizePreset::Default,
    }
}

fn parse_color_mode(s: &str) -> anyhow::Result<ColorMode> {
    s.parse::<ColorMode>().map_err(|e| anyhow::anyhow!(e.to_string()))
}

fn parse_hierarchical(s: &str) -> anyhow::Result<HierarchicalMode> {
    s.parse::<HierarchicalMode>().map_err(|e| anyhow::anyhow!(e.to_string()))
}

fn parse_path_simplify(s: &str) -> anyhow::Result<PathSimplifyMode> {
    s.parse::<PathSimplifyMode>().map_err(|e| anyhow::anyhow!(e.to_string()))
}

/// Apply a custom profile preset (delegates to covecto_core).
fn apply_profile(name: &str, config: &mut VectorizeConfig) {
    covecto_core::apply_profile(name, config);
}

#[derive(Clone)]
struct VectorizeOpts {
    output: Option<PathBuf>,
    format: String,
    engine: String,
    preset: Option<String>,
    profile: Option<String>,
    color_precision: Option<i32>,
    filter_speckle: Option<usize>,
    corner_threshold: Option<i32>,
    splice_threshold: Option<i32>,
    color_mode: Option<String>,
    hierarchical: Option<String>,
    path_simplify: Option<String>,
    layer_difference: Option<i32>,
    length_threshold: Option<f64>,
    max_iterations: Option<usize>,
    path_precision: Option<u32>,
    optimize: bool,
    optimize_preset: String,
    multipass: bool,
    multipass_iterations: usize,
}

impl VectorizeOpts {
    fn to_config(&self) -> anyhow::Result<VectorizeConfig> {
        let mut config = VectorizeConfig {
            engine: parse_engine(&self.engine)?,
            optimize: self.optimize,
            optimize_config: OptimizeConfig {
                preset: parse_opt_preset(&self.optimize_preset),
                multipass: self.multipass,
                multipass_iterations: self.multipass_iterations,
            },
            spline_preset: self.preset.as_deref().map(parse_spline_preset).transpose()?,
            color_precision: self.color_precision,
            filter_speckle: self.filter_speckle,
            corner_threshold: self.corner_threshold,
            splice_threshold: self.splice_threshold,
            color_mode: self.color_mode.as_deref().map(parse_color_mode).transpose()?,
            hierarchical: self.hierarchical.as_deref().map(parse_hierarchical).transpose()?,
            path_simplify_mode: self.path_simplify.as_deref().map(parse_path_simplify).transpose()?,
            layer_difference: self.layer_difference,
            length_threshold: self.length_threshold,
            max_iterations: self.max_iterations,
            path_precision: self.path_precision,
        };

        // Apply profile preset (overrides individual params)
        if let Some(ref profile) = self.profile {
            apply_profile(profile, &mut config);
        }

        Ok(config)
    }
}

fn parse_output_format(s: &str) -> anyhow::Result<OutputFormat> {
    s.parse::<OutputFormat>()
        .map_err(|e| anyhow::anyhow!(e.to_string()))
}

fn output_extension(format: &OutputFormat) -> &'static str {
    format.extension()
}

fn vectorize_single(input: &Path, output: &Path, config: &VectorizeConfig, format: &OutputFormat) -> anyhow::Result<()> {
    let img = load_image(input)?;
    let req = covecto_core::VectorizeRequest::new(img).with_config(config.clone());
    let result = vectorize(&req)?;

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let (bytes, _content_type) = convert_output(&result.svg, *format)?;

    std::fs::write(output, &bytes)?;
    info!(
        "✓ {} → {} ({}ms, {} bytes, {} paths, engine={}, format={})",
        input.display(),
        output.display(),
        result.metadata.processing_time_ms,
        bytes.len(),
        result.metadata.path_count,
        result.engine_used,
        format.extension(),
    );
    Ok(())
}

fn cmd_vectorize(input: &Path, opts: &VectorizeOpts) -> anyhow::Result<()> {
    let config = opts.to_config()?;
    let format = parse_output_format(&opts.format)?;
    let ext = output_extension(&format);

    if input.is_dir() {
        let out_dir = opts
            .output
            .clone()
            .unwrap_or_else(|| input.join("vectorized"));
        std::fs::create_dir_all(&out_dir)?;
        let mut count = 0u32;
        let mut errors = 0u32;
        for entry in std::fs::read_dir(input)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && is_image_file(&path) {
                let stem = path.file_stem().unwrap().to_string_lossy().to_string();
                let out_path = out_dir.join(format!("{stem}.{ext}"));
                match vectorize_single(&path, &out_path, &config, &format) {
                    Ok(()) => count += 1,
                    Err(e) => {
                        eprintln!("✗ {}: {e}", path.display());
                        errors += 1;
                    }
                }
            }
        }
        println!("Done: {count} converted, {errors} errors");
    } else {
        match opts.output.as_deref() {
            Some(out) => vectorize_single(input, out, &config, &format)?,
            None => {
                let img = load_image(input)?;
                let req =
                    covecto_core::VectorizeRequest::new(img).with_config(config.clone());
                let result = vectorize(&req)?;
                let (bytes, _ct) = convert_output(&result.svg, format)?;
                std::io::Write::write_all(&mut std::io::stdout(), &bytes)?;
            }
        }
    }
    Ok(())
}

fn is_image_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .as_deref(),
        Some("png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif" | "tiff" | "tif" | "avif")
    )
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Vectorize {
            input,
            output,
            format,
            engine,
            preset,
            profile,
            color_precision,
            filter_speckle,
            corner_threshold,
            splice_threshold,
            color_mode,
            hierarchical,
            path_simplify,
            layer_difference,
            length_threshold,
            max_iterations,
            path_precision,
            optimize,
            optimize_preset,
            multipass,
            multipass_iterations,
        } => {
            let opts = VectorizeOpts {
                output,
                format,
                engine,
                preset,
                profile,
                color_precision,
                filter_speckle,
                corner_threshold,
                splice_threshold,
                color_mode,
                hierarchical,
                path_simplify,
                layer_difference,
                length_threshold,
                max_iterations,
                path_precision,
                optimize,
                optimize_preset,
                multipass,
                multipass_iterations,
            };
            cmd_vectorize(&input, &opts)?;
        }
        Commands::Serve { port } => {
            covecto_api::run_server(port).await?;
        }
    }
    Ok(())
}