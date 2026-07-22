use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use covecto_core::{
    Engine, OptimizeConfig, OptimizePreset, SplinePreset, VectorizeConfig, load_image,
    optimize_svg, vectorize,
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
enum Commands {
    /// Vectorize one or more images to SVG
    Vectorize {
        /// Input file or directory
        #[arg(value_name = "INPUT")]
        input: PathBuf,
        /// Output file or directory (default: stdout / INPUT/vectorized/)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Engine: auto, spline, pixel-exact (default: auto)
        #[arg(short, long, default_value = "auto")]
        engine: String,
        /// Spline preset: bw, poster, photo (overrides individual params)
        #[arg(long)]
        preset: Option<String>,
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
    match s.to_lowercase().as_str() {
        "auto" => Ok(Engine::Auto),
        "spline" => Ok(Engine::Spline),
        "pixel-exact" | "pixel_exact" => Ok(Engine::PixelExact),
        _ => anyhow::bail!("Unknown engine: {s}. Use: auto, spline, pixel-exact"),
    }
}

fn parse_spline_preset(s: &str) -> anyhow::Result<SplinePreset> {
    match s.to_lowercase().as_str() {
        "bw" => Ok(SplinePreset::Bw),
        "poster" => Ok(SplinePreset::Poster),
        "photo" => Ok(SplinePreset::Photo),
        _ => anyhow::bail!("Unknown preset: {s}. Use: bw, poster, photo"),
    }
}

fn parse_opt_preset(s: &str) -> OptimizePreset {
    match s.to_lowercase().as_str() {
        "safe" => OptimizePreset::Safe,
        "none" => OptimizePreset::None,
        _ => OptimizePreset::Default,
    }
}

#[derive(Clone)]
struct VectorizeOpts {
    output: Option<PathBuf>,
    engine: String,
    preset: Option<String>,
    optimize: bool,
    optimize_preset: String,
    multipass: bool,
    multipass_iterations: usize,
}

impl VectorizeOpts {
    fn to_config(&self) -> anyhow::Result<VectorizeConfig> {
        let engine = parse_engine(&self.engine)?;
        let spline_preset = self
            .preset
            .as_deref()
            .map(parse_spline_preset)
            .transpose()?;
        let opt_preset = parse_opt_preset(&self.optimize_preset);
        Ok(VectorizeConfig {
            engine,
            optimize: self.optimize,
            optimize_config: OptimizeConfig {
                preset: opt_preset,
                multipass: self.multipass,
                multipass_iterations: self.multipass_iterations,
            },
            spline_preset,
            ..Default::default()
        })
    }
}

fn vectorize_single(input: &Path, output: &Path, config: &VectorizeConfig) -> anyhow::Result<()> {
    let img = load_image(input)?;
    let req = covecto_core::VectorizeRequest::new(img).with_config(config.clone());
    let result = vectorize(&req)?;

    let svg = if config.optimize {
        optimize_svg(&result.svg, &config.optimize_config)?
    } else {
        result.svg
    };

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output, &svg)?;
    info!(
        "✓ {} → {} ({}ms, {} bytes, {} paths, engine={})",
        input.display(),
        output.display(),
        result.metadata.processing_time_ms,
        svg.len(),
        result.metadata.path_count,
        result.engine_used,
    );
    Ok(())
}

fn cmd_vectorize(input: &Path, opts: &VectorizeOpts) -> anyhow::Result<()> {
    let config = opts.to_config()?;

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
                let out_path = out_dir.join(format!("{stem}.svg"));
                match vectorize_single(&path, &out_path, &config) {
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
            Some(out) => vectorize_single(input, out, &config)?,
            None => {
                let img = load_image(input)?;
                let req = covecto_core::VectorizeRequest::new(img).with_config(config.clone());
                let result = vectorize(&req)?;
                let svg = if config.optimize {
                    optimize_svg(&result.svg, &config.optimize_config)?
                } else {
                    result.svg
                };
                println!("{svg}");
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

fn _print_result_summary(result: &covecto_core::VectorizeResult) {
    println!(
        "Engine: {} | Time: {}ms | Size: {} bytes | Paths: {}",
        result.engine_used,
        result.metadata.processing_time_ms,
        result.metadata.svg_byte_size,
        result.metadata.path_count,
    );
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Vectorize {
            input,
            output,
            engine,
            preset,
            optimize,
            optimize_preset,
            multipass,
            multipass_iterations,
        } => {
            let opts = VectorizeOpts {
                output,
                engine,
                preset,
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
