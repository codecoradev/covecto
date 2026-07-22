use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use covecto_core::{
    ColorMode, Engine, HierarchicalMode, OptimizeConfig, OptimizePreset, OutputFormat,
    PathSimplifyMode, SplinePreset, VectorizeConfig, convert_output, load_image, vectorize,
};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use serde::Serialize;
use tracing::info;

/// Covecto — Cora Vectorizer. Dual-engine image-to-SVG vectorization.
#[derive(Parser)]
#[command(name = "covecto", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Vectorize one or more images to SVG
    Vectorize(Box<VectorizeArgs>),
    /// Start HTTP API server
    Serve {
        /// Port to listen on (default: 3000)
        #[arg(short, long, default_value = "3000")]
        port: u16,
    },
}

#[derive(clap::Args)]
struct VectorizeArgs {
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
    /// Output results as JSON to stdout (files still written to disk)
    #[arg(long)]
    json: bool,
    /// Pretty-print JSON output
    #[arg(long)]
    json_pretty: bool,
    /// Disable progress bar / spinner
    #[arg(long)]
    no_progress: bool,
}

fn parse_engine(s: &str) -> anyhow::Result<Engine> {
    s.parse::<Engine>()
        .map_err(|e| anyhow::anyhow!(e.to_string()))
}

fn parse_spline_preset(s: &str) -> anyhow::Result<SplinePreset> {
    s.parse::<SplinePreset>()
        .map_err(|e| anyhow::anyhow!(e.to_string()))
}

fn parse_opt_preset(s: &str) -> OptimizePreset {
    match s.to_lowercase().as_str() {
        "safe" => OptimizePreset::Safe,
        "none" => OptimizePreset::None,
        _ => OptimizePreset::Default,
    }
}

fn parse_color_mode(s: &str) -> anyhow::Result<ColorMode> {
    s.parse::<ColorMode>()
        .map_err(|e| anyhow::anyhow!(e.to_string()))
}

fn parse_hierarchical(s: &str) -> anyhow::Result<HierarchicalMode> {
    s.parse::<HierarchicalMode>()
        .map_err(|e| anyhow::anyhow!(e.to_string()))
}

fn parse_path_simplify(s: &str) -> anyhow::Result<PathSimplifyMode> {
    s.parse::<PathSimplifyMode>()
        .map_err(|e| anyhow::anyhow!(e.to_string()))
}

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
    json: bool,
    json_pretty: bool,
    no_progress: bool,
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
            spline_preset: self
                .preset
                .as_deref()
                .map(parse_spline_preset)
                .transpose()?,
            color_precision: self.color_precision,
            filter_speckle: self.filter_speckle,
            corner_threshold: self.corner_threshold,
            splice_threshold: self.splice_threshold,
            color_mode: self
                .color_mode
                .as_deref()
                .map(parse_color_mode)
                .transpose()?,
            hierarchical: self
                .hierarchical
                .as_deref()
                .map(parse_hierarchical)
                .transpose()?,
            path_simplify_mode: self
                .path_simplify
                .as_deref()
                .map(parse_path_simplify)
                .transpose()?,
            layer_difference: self.layer_difference,
            length_threshold: self.length_threshold,
            max_iterations: self.max_iterations,
            path_precision: self.path_precision,
        };

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

/// Machine-readable JSON output for scripting.
#[derive(Serialize)]
struct JsonResult {
    path: String,
    output: String,
    engine: String,
    format: String,
    bytes: usize,
    paths: usize,
    time_ms: u64,
    input_size: (u32, u32),
    compression_ratio: f64,
}

impl JsonResult {
    fn from_result(
        input: &Path,
        output: &Path,
        result: &covecto_core::VectorizeResult,
        format: &OutputFormat,
        output_bytes: usize,
    ) -> Self {
        Self {
            path: input.display().to_string(),
            output: output.display().to_string(),
            engine: result.engine_used.to_string(),
            format: format.extension().to_string(),
            bytes: output_bytes,
            paths: result.metadata.path_count,
            time_ms: result.metadata.processing_time_ms,
            input_size: result.metadata.input_size,
            compression_ratio: result.metadata.compression_ratio,
        }
    }
}

/// Vectorize a single file and optionally write output.
/// Returns the result and output bytes for JSON reporting.
fn vectorize_single_raw(
    input: &Path,
    output: Option<&Path>,
    config: &VectorizeConfig,
    format: &OutputFormat,
) -> anyhow::Result<(covecto_core::VectorizeResult, usize, PathBuf)> {
    let img = load_image(input)?;
    let req = covecto_core::VectorizeRequest::new(img).with_config(config.clone());
    let result = vectorize(&req)?;

    let out_path = match output {
        Some(p) => p.to_path_buf(),
        None => {
            let stem = input.file_stem().unwrap().to_string_lossy().to_string();
            let ext = format.extension();
            input
                .parent()
                .unwrap_or(Path::new("."))
                .join(format!("{stem}.{ext}"))
        }
    };

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let (bytes, _content_type) = convert_output(&result.svg, *format)?;
    std::fs::write(&out_path, &bytes)?;

    info!(
        "✓ {} → {} ({}ms, {} bytes, {} paths, engine={}, format={})",
        input.display(),
        out_path.display(),
        result.metadata.processing_time_ms,
        bytes.len(),
        result.metadata.path_count,
        result.engine_used,
        format.extension(),
    );

    Ok((result, bytes.len(), out_path))
}

fn make_spinner(msg: &str) -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    spinner.set_message(msg.to_string());
    spinner
}

fn make_progress(total: usize) -> (MultiProgress, ProgressBar) {
    let multi = MultiProgress::new();
    let pb = multi.add(ProgressBar::new(total as u64));
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.cyan} [{bar:30.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("█▓░"),
    );
    pb.set_message("vectorizing".to_string());
    (multi, pb)
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

fn cmd_vectorize(input: &Path, opts: &VectorizeOpts) -> anyhow::Result<()> {
    let config = opts.to_config()?;
    let format = parse_output_format(&opts.format)?;
    let ext = output_extension(&format);

    if input.is_dir() {
        // Batch mode
        let out_dir = opts
            .output
            .clone()
            .unwrap_or_else(|| input.join("vectorized"));
        std::fs::create_dir_all(&out_dir)?;

        // Collect image files first
        let entries: Vec<PathBuf> = std::fs::read_dir(input)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_file() && is_image_file(p))
            .collect();

        let show_progress = !opts.no_progress && !entries.is_empty();
        let (_multi, pb) = if show_progress {
            let (m, p) = make_progress(entries.len());
            (Some(m), Some(p))
        } else {
            (None, None)
        };

        let mut results = Vec::with_capacity(entries.len());
        let mut count = 0u32;
        let mut errors = 0u32;

        for path in &entries {
            let stem = path.file_stem().unwrap().to_string_lossy().to_string();
            let out_path = out_dir.join(format!("{stem}.{ext}"));

            match vectorize_single_raw(path, Some(&out_path), &config, &format) {
                Ok((result, bytes, out)) => {
                    if opts.json {
                        results.push(JsonResult::from_result(path, &out, &result, &format, bytes));
                    }
                    count += 1;
                }
                Err(e) => {
                    eprintln!("✗ {}: {e}", path.display());
                    errors += 1;
                }
            }

            if let Some(ref p) = pb {
                p.inc(1);
            }
        }

        if let Some(p) = pb {
            p.finish_and_clear();
        }
        drop(_multi);

        if !opts.json && !opts.no_progress {
            println!("Done: {count} converted, {errors} errors");
        }

        // Output JSON for batch
        if opts.json {
            let json_output = if opts.json_pretty {
                serde_json::to_string_pretty(&results)?
            } else {
                serde_json::to_string(&results)?
            };
            println!("{json_output}");
        }
    } else {
        // Single file mode
        let out_path = opts.output.clone().unwrap_or_else(|| {
            let stem = input.file_stem().unwrap().to_string_lossy().to_string();
            PathBuf::from(format!("{stem}.{ext}"))
        });

        if opts.json {
            // JSON mode: always write to file, output JSON to stdout
            let show_progress = !opts.no_progress;
            let spinner = if show_progress {
                Some(make_spinner(&format!(
                    "{} → {} ",
                    input.display(),
                    out_path.display()
                )))
            } else {
                None
            };

            match vectorize_single_raw(input, Some(&out_path), &config, &format) {
                Ok((result, bytes, out)) => {
                    if let Some(s) = spinner {
                        s.finish_and_clear();
                    }
                    let json_result = JsonResult::from_result(input, &out, &result, &format, bytes);
                    let json_output = if opts.json_pretty {
                        serde_json::to_string_pretty(&json_result)?
                    } else {
                        serde_json::to_string(&json_result)?
                    };
                    println!("{json_output}");
                }
                Err(e) => {
                    if let Some(s) = spinner {
                        s.finish_and_clear();
                    }
                    return Err(e);
                }
            }
        } else if opts.output.is_some() {
            // Output to specified file
            let show_progress = !opts.no_progress;
            let spinner = if show_progress {
                Some(make_spinner(&format!(
                    "{} → {} ",
                    input.display(),
                    out_path.display()
                )))
            } else {
                None
            };

            let res = vectorize_single_raw(input, Some(&out_path), &config, &format);

            if let Some(s) = spinner {
                s.finish_and_clear();
            }

            res?;
        } else {
            // No --output and no --json: write SVG to stdout (backward compat)
            let img = load_image(input)?;
            let req = covecto_core::VectorizeRequest::new(img).with_config(config.clone());
            let result = vectorize(&req)?;
            let (bytes, _ct) = convert_output(&result.svg, format)?;
            std::io::Write::write_all(&mut std::io::stdout(), &bytes)?;
        }
    }

    Ok(())
}

fn from_args(args: VectorizeArgs) -> VectorizeOpts {
    VectorizeOpts {
        output: args.output,
        format: args.format,
        engine: args.engine,
        preset: args.preset,
        profile: args.profile,
        color_precision: args.color_precision,
        filter_speckle: args.filter_speckle,
        corner_threshold: args.corner_threshold,
        splice_threshold: args.splice_threshold,
        color_mode: args.color_mode,
        hierarchical: args.hierarchical,
        path_simplify: args.path_simplify,
        layer_difference: args.layer_difference,
        length_threshold: args.length_threshold,
        max_iterations: args.max_iterations,
        path_precision: args.path_precision,
        optimize: args.optimize,
        optimize_preset: args.optimize_preset,
        multipass: args.multipass,
        multipass_iterations: args.multipass_iterations,
        json: args.json || args.json_pretty,
        json_pretty: args.json_pretty,
        no_progress: args.no_progress,
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Vectorize(args) => {
            let input = args.input.clone();
            let opts = from_args(*args);
            cmd_vectorize(&input, &opts)?;
        }
        Commands::Serve { port } => {
            covecto_api::run_server(port).await?;
        }
    }
    Ok(())
}
