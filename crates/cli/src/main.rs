use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

use clap::{Parser, Subcommand};
use covecto_core::{
    Engine, ImageLimits, OutputFormat, RenderResult, VectorizeConfig, load_image, options,
    render_to, select_engine,
};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use serde::Serialize;
use std::io::Write;
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
    /// Input file, directory, or glob pattern (e.g. "icons/*.png")
    #[arg(value_name = "INPUT")]
    input: PathBuf,
    /// Output file (single) or directory (batch)
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Output directory for batch mode (alternative to --output)
    #[arg(long)]
    output_dir: Option<PathBuf>,
    /// Output filename template with {stem} and {ext} placeholders (default: "{stem}.{ext}")
    #[arg(long)]
    output_template: Option<String>,
    /// Output format: svg, pdf, eps (default: svg)
    #[arg(short = 'F', long, default_value = "svg")]
    format: String,
    /// Engine: auto, spline, pixel-exact (default: auto)
    #[arg(short, long)]
    engine: Option<String>,
    /// Preset (vtracer starting point for Spline parameters): bw, poster, photo
    #[arg(long)]
    preset: Option<String>,
    /// Profile (may also choose the Engine): icon, logo, photo, lineart.
    /// Explicit flags always win over the Profile and the Preset.
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
    /// Spline: downscale inputs above this many megapixels before tracing
    /// (default 2; 0 = trace at full size). Output keeps the original size.
    #[arg(long, value_name = "MP")]
    spline_max_mp: Option<f64>,
    /// Run SVG optimization after vectorization (default: on).
    /// Disable with `--optimize=false` or `--no-optimize`.
    #[arg(
        long,
        value_name = "BOOL",
        action = clap::ArgAction::Set,
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "true"
    )]
    optimize: Option<bool>,
    /// Disable SVG optimization (same as `--optimize=false`)
    #[arg(long, conflicts_with = "optimize")]
    no_optimize: bool,
    /// Optimization level: default, safe, none (default: default)
    #[arg(long)]
    optimize_preset: Option<String>,
    /// Run multiple optimization passes
    #[arg(long)]
    multipass: bool,
    /// Number of multipass iterations (default: 10, capped at 16)
    #[arg(long)]
    multipass_iterations: Option<usize>,
    /// Recursively scan input directories
    #[arg(short = 'R', long)]
    recursive: bool,
    /// Output results as JSON to stdout (files still written to disk)
    #[arg(long)]
    json: bool,
    /// Pretty-print JSON output
    #[arg(long)]
    json_pretty: bool,
    /// Disable progress bar / spinner
    #[arg(long)]
    no_progress: bool,
    /// Preview what would be processed without actually vectorizing
    #[arg(long)]
    dry_run: bool,
    /// Refuse images wider or taller than this many pixels, checked before
    /// decoding (default: 8192; 0 = no limit)
    #[arg(long, value_name = "PX", default_value_t = covecto_core::DEFAULT_MAX_DIMENSION)]
    max_dimension: u32,
}

#[derive(Clone)]
struct VectorizeOpts {
    output: Option<PathBuf>,
    output_dir: Option<PathBuf>,
    output_template: Option<String>,
    format: String,
    /// Explicit options only (what the user actually typed), as canonical
    /// `(name, value)` pairs for `covecto_core::options::resolve`.
    options: Vec<(&'static str, String)>,
    recursive: bool,
    json: bool,
    json_pretty: bool,
    no_progress: bool,
    dry_run: bool,
    /// Applied to every image before decoding.
    limits: ImageLimits,
}

impl VectorizeOpts {
    fn to_config(&self) -> anyhow::Result<VectorizeConfig> {
        options::resolve(self.options.iter().map(|(k, v)| (*k, v.as_str()))).map_err(cli_error)
    }
}

/// Report an options error with the flag spelling the user typed.
fn cli_error(e: covecto_core::Error) -> anyhow::Error {
    let flag = |name: &str| format!("--{}", name.replace('_', "-"));
    match e {
        covecto_core::Error::InvalidOption {
            option,
            value,
            expected,
        } => anyhow::anyhow!(
            "invalid value '{value}' for {}: expected {expected}",
            flag(option)
        ),
        covecto_core::Error::UnknownName { what, name, valid } if what != "option" => {
            anyhow::anyhow!(
                "unknown {what} '{name}' for {}. Valid: {}",
                flag(what),
                valid.join(", ")
            )
        }
        covecto_core::Error::DuplicateOption(option) => {
            anyhow::anyhow!("{} was given more than once", flag(option))
        }
        other => anyhow::anyhow!(other),
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
    fn from_result(input: &Path, output: &Path, result: &RenderResult) -> Self {
        Self {
            path: input.display().to_string(),
            output: output.display().to_string(),
            engine: result.engine_used.to_string(),
            format: result.format.extension().to_string(),
            bytes: result.bytes_written,
            paths: result.metadata.path_count,
            time_ms: result.metadata.processing_time_ms,
            input_size: result.metadata.input_size,
            compression_ratio: result.metadata.compression_ratio,
        }
    }
}

/// Build output filename from template, stem, and extension.
fn apply_output_template(template: &str, stem: &str, ext: &str) -> String {
    template.replace("{stem}", stem).replace("{ext}", ext)
}

/// A unique temporary path in the same directory as `path` (same filesystem,
/// so the final rename is atomic).
fn temp_sibling(path: &Path) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let name = path
        .file_name()
        .map_or_else(|| "output".into(), |n| n.to_string_lossy().into_owned());
    path.with_file_name(format!(
        ".{name}.covecto-tmp-{}-{nanos}",
        std::process::id()
    ))
}

/// Vectorize one file into `output` (or next to the input) in `format`.
/// Returns the render result and the output path.
///
/// The output is written to a temporary file in the same directory and
/// renamed over `output` only on success, so a failure never leaves a
/// partial file and never touches a file that already exists there (which
/// may even be the input image).
fn vectorize_single_raw(
    input: &Path,
    output: Option<&Path>,
    config: &VectorizeConfig,
    format: &OutputFormat,
    limits: ImageLimits,
) -> anyhow::Result<(RenderResult, PathBuf)> {
    let img = load_image(input, limits)?;
    let req = covecto_core::VectorizeRequest::new(img).with_config(config.clone());

    let out_path = match output {
        Some(p) => p.to_path_buf(),
        None => {
            let stem = input.file_stem().unwrap().to_string_lossy().to_string();
            input
                .parent()
                .unwrap_or(Path::new("."))
                .join(format!("{stem}.{}", format.extension()))
        }
    };

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // core picks the strategy per format (SVG streams; PDF/EPS convert in
    // memory first). Write next to the target, then rename into place.
    let tmp_path = temp_sibling(&out_path);
    let mut writer = std::io::BufWriter::new(std::fs::File::create(&tmp_path)?);
    let rendered = (|| -> anyhow::Result<RenderResult> {
        let result = render_to(&mut writer, &req, *format)?;
        writer.flush()?;
        Ok(result)
    })();
    drop(writer);
    let result = match rendered.and_then(|r| {
        std::fs::rename(&tmp_path, &out_path)?;
        Ok(r)
    }) {
        Ok(result) => result,
        Err(e) => {
            // Only our own temporary file is removed; `out_path` is untouched.
            let _ = std::fs::remove_file(&tmp_path);
            return Err(e);
        }
    };

    info!(
        "✓ {} → {} ({}ms, {} bytes, {} paths, engine={}, format={})",
        input.display(),
        out_path.display(),
        result.metadata.processing_time_ms,
        result.bytes_written,
        result.metadata.path_count,
        result.engine_used,
        format.extension(),
    );

    Ok((result, out_path))
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

/// Detect if input string looks like a glob pattern (contains * ? or [).
fn is_glob_pattern(s: &str) -> bool {
    s.contains('*') || s.contains('?') || s.contains('[')
}

/// Collect image files from input path.
/// Handles: single file, directory (flat/recursive), glob patterns.
fn collect_files(input: &Path, recursive: bool) -> anyhow::Result<Vec<PathBuf>> {
    let input_str = input.to_string_lossy();

    // Glob pattern mode
    if is_glob_pattern(&input_str) {
        let pattern = if recursive {
            // If input has no path separator, prefix with ./
            if !input_str.contains('/') && !input_str.contains('\\') {
                format!("./**/{input_str}")
            } else {
                input_str.to_string()
            }
        } else {
            input_str.to_string()
        };

        let entries: Vec<PathBuf> = glob::glob(&pattern)?
            .filter_map(|e| e.ok())
            .filter(|p| p.is_file() && is_image_file(p))
            .collect();

        if entries.is_empty() {
            anyhow::bail!("No files matched pattern: {pattern}");
        }
        return Ok(entries);
    }

    // Single file
    if input.is_file() {
        return Ok(vec![input.to_path_buf()]);
    }

    // Directory mode
    if input.is_dir() {
        if recursive {
            let entries: Vec<PathBuf> = walkdir::WalkDir::new(input)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .map(|e| e.into_path())
                .filter(|p| is_image_file(p))
                .collect();
            if entries.is_empty() {
                anyhow::bail!("No image files found in: {}", input.display());
            }
            Ok(entries)
        } else {
            let entries: Vec<PathBuf> = std::fs::read_dir(input)?
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_file() && is_image_file(p))
                .collect();
            if entries.is_empty() {
                anyhow::bail!("No image files found in: {}", input.display());
            }
            Ok(entries)
        }
    } else {
        anyhow::bail!("Input not found: {}", input.display());
    }
}

/// Compute output path for a batch item, optionally preserving directory structure.
fn compute_output_path(
    input: &Path,
    root: &Path,
    out_dir: &Path,
    template: &str,
    ext: &str,
    preserve_structure: bool,
) -> PathBuf {
    let stem = input.file_stem().unwrap().to_string_lossy().to_string();
    let filename = apply_output_template(template, &stem, ext);

    if preserve_structure {
        // Preserve relative path from root
        if let Ok(rel) = input.strip_prefix(root)
            && let Some(parent) = rel.parent()
            && parent != Path::new("")
            && parent != Path::new(".")
        {
            return out_dir.join(parent).join(filename);
        }
    }
    out_dir.join(filename)
}

/// The Engine a real run would use for `file`, as shown by `--dry-run`.
///
/// Uses core's `select_engine`, the same decision `vectorize` makes, so the
/// preview cannot drift from a real run. Only an `Auto` Engine needs the
/// image's pixels; a configured Engine (flag or Profile) is shown without
/// reading the file.
fn preview_engine(
    file: &Path,
    config: &VectorizeConfig,
    limits: ImageLimits,
) -> anyhow::Result<String> {
    if config.engine != Engine::Auto {
        return Ok(config.engine.to_string());
    }
    let img = load_image(file, limits)?;
    Ok(format!("{} (auto)", select_engine(config, &img)))
}

fn cmd_vectorize(input: &Path, opts: &VectorizeOpts) -> anyhow::Result<()> {
    let config = opts.to_config()?;
    let format = parse_output_format(&opts.format)?;
    let ext = output_extension(&format);
    let template = opts.output_template.as_deref().unwrap_or("{stem}.{ext}");

    // Collect files
    let files = collect_files(input, opts.recursive)?;

    // Single file (non-batch) mode
    if files.len() == 1
        && input.is_file()
        && !opts.recursive
        && !is_glob_pattern(&input.to_string_lossy())
    {
        let file = &files[0];

        if opts.dry_run {
            let out_path = opts.output.clone().unwrap_or_else(|| {
                let stem = file.file_stem().unwrap().to_string_lossy().to_string();
                PathBuf::from(apply_output_template(template, &stem, ext))
            });
            let engine = preview_engine(file, &config, opts.limits)
                .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", file.display()))?;
            println!(
                "Would process: {} → {} (engine: {}, format: {})",
                file.display(),
                out_path.display(),
                engine,
                ext,
            );
            return Ok(());
        }

        let out_path = opts.output.clone().unwrap_or_else(|| {
            let stem = file.file_stem().unwrap().to_string_lossy().to_string();
            PathBuf::from(apply_output_template(template, &stem, ext))
        });

        if opts.json {
            let show_progress = !opts.no_progress;
            let spinner = if show_progress {
                Some(make_spinner(&format!(
                    "{} → {} ",
                    file.display(),
                    out_path.display()
                )))
            } else {
                None
            };

            match vectorize_single_raw(file, Some(&out_path), &config, &format, opts.limits) {
                Ok((result, out)) => {
                    if let Some(s) = spinner {
                        s.finish_and_clear();
                    }
                    let json_result = JsonResult::from_result(file, &out, &result);
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
            let show_progress = !opts.no_progress;
            let spinner = if show_progress {
                Some(make_spinner(&format!(
                    "{} → {} ",
                    file.display(),
                    out_path.display()
                )))
            } else {
                None
            };

            let res = vectorize_single_raw(file, Some(&out_path), &config, &format, opts.limits);
            if let Some(s) = spinner {
                s.finish_and_clear();
            }
            res?;
        } else {
            // No --output and no --json: write to stdout (backward compat)
            let img = load_image(file, opts.limits)?;
            let req = covecto_core::VectorizeRequest::new(img).with_config(config.clone());
            let mut stdout = std::io::BufWriter::new(std::io::stdout().lock());
            render_to(&mut stdout, &req, format)?;
            stdout.flush()?;
        }

        return Ok(());
    }

    // Batch mode (multiple files: directory, recursive, glob)
    let root = input;
    let preserve_structure = opts.recursive && !is_glob_pattern(&input.to_string_lossy());
    let out_dir = opts
        .output_dir
        .clone()
        .or_else(|| opts.output.clone())
        .unwrap_or_else(|| root.join("vectorized"));

    // Dry-run: just list files (writes nothing, not even the output directory)
    if opts.dry_run {
        // Decoding (only needed for an Auto Engine) runs in parallel; the
        // listing keeps input order.
        let previews: Vec<anyhow::Result<String>> = files
            .par_iter()
            .map(|file| preview_engine(file, &config, opts.limits))
            .collect();
        println!("Would process {} files:", files.len());
        let mut unreadable = 0usize;
        for (file, preview) in files.iter().zip(previews) {
            let out_path =
                compute_output_path(file, root, &out_dir, template, ext, preserve_structure);
            match preview {
                Ok(engine) => println!(
                    "  {} → {} (engine: {})",
                    file.display(),
                    out_path.display(),
                    engine
                ),
                Err(e) => {
                    unreadable += 1;
                    eprintln!("  {} → error: {e}", file.display());
                }
            }
        }
        if unreadable > 0 {
            anyhow::bail!("{unreadable} of {} files could not be read", files.len());
        }
        return Ok(());
    }

    std::fs::create_dir_all(&out_dir)?;

    let show_progress = !opts.no_progress && !files.is_empty();
    let (multi, pb) = if show_progress {
        let (m, p) = make_progress(files.len());
        (Some(m), Some(p))
    } else {
        (None, None)
    };

    // Pre-compute output paths (sequential — needs HashSet for dedup)
    let out_paths: Vec<PathBuf> = {
        let mut used_names = HashSet::with_capacity(files.len());
        files
            .iter()
            .map(|file| {
                let mut out_path =
                    compute_output_path(file, root, &out_dir, template, ext, preserve_structure);

                // Deduplicate output filenames to prevent silent overwrites
                if !preserve_structure {
                    let name = out_path.file_name().unwrap().to_string_lossy().to_string();
                    if !used_names.insert(name) {
                        let stem = file.file_stem().unwrap().to_string_lossy().to_string();
                        let parent_dir = file
                            .parent()
                            .and_then(|p| p.file_name())
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown");
                        let mut candidate = format!("{parent_dir}_{stem}.{ext}");
                        let mut counter: usize = 1;
                        while !used_names.insert(candidate.clone()) {
                            counter += 1;
                            candidate = format!("{parent_dir}_{stem}_{counter}.{ext}");
                        }
                        out_path = out_dir.join(candidate);
                    }
                }
                out_path
            })
            .collect()
    };

    // Atomic counters for thread-safe progress and error tracking
    let count = AtomicU32::new(0);
    let error_count = AtomicU32::new(0);
    let error_list = Mutex::new(Vec::new());

    // Store results as (index, JsonResult) to preserve input ordering
    let indexed_results: Vec<(usize, JsonResult)> = files
        .par_iter()
        .zip(out_paths.par_iter())
        .enumerate()
        .filter_map(|(idx, (file, out_path))| {
            let res = vectorize_single_raw(file, Some(out_path), &config, &format, opts.limits);
            if let Some(ref p) = pb {
                p.inc(1);
            }
            match res {
                Ok((result, out)) => {
                    count.fetch_add(1, Ordering::Relaxed);
                    if opts.json {
                        Some((idx, JsonResult::from_result(file, &out, &result)))
                    } else {
                        None
                    }
                }
                Err(e) => {
                    error_count.fetch_add(1, Ordering::Relaxed);
                    error_list
                        .lock()
                        .unwrap()
                        .push((file.display().to_string(), e.to_string()));
                    None
                }
            }
        })
        .collect();

    if let Some(p) = pb {
        p.finish_and_clear();
    }
    drop(multi);

    // Report errors collected from threads
    let errors = error_list.into_inner().unwrap();
    for (path, err) in &errors {
        eprintln!("\u{2717} {path}: {err}");
    }

    let count = count.load(Ordering::Relaxed);
    let error_count = errors.len() as u32;

    // Sort by original index to preserve input ordering in JSON output
    let mut sorted_results = indexed_results;
    sorted_results.sort_by_key(|(idx, _)| *idx);
    let results: Vec<JsonResult> = sorted_results.into_iter().map(|(_, r)| r).collect();

    if !opts.json && !opts.no_progress {
        println!("Done: {count} converted, {error_count} errors");
    }

    if opts.json {
        let json_output = if opts.json_pretty {
            serde_json::to_string_pretty(&results)?
        } else {
            serde_json::to_string(&results)?
        };
        println!("{json_output}");
    }

    // Failures are reported above, per file; the exit code must say so too.
    if error_count > 0 {
        anyhow::bail!("{error_count} of {} files failed", files.len());
    }
    Ok(())
}

fn from_args(args: VectorizeArgs) -> VectorizeOpts {
    // Only flags the user typed become Explicit options; defaults belong to
    // covecto-core, so a Profile can still choose the Engine.
    let mut options: Vec<(&'static str, String)> = Vec::new();
    let mut push = |name: &'static str, value: Option<String>| {
        if let Some(v) = value {
            options.push((name, v));
        }
    };
    let s = |v: Option<i32>| v.map(|v| v.to_string());
    let u = |v: Option<usize>| v.map(|v| v.to_string());
    push("engine", args.engine);
    push("preset", args.preset);
    push("profile", args.profile);
    push("color_precision", s(args.color_precision));
    push("filter_speckle", u(args.filter_speckle));
    push("corner_threshold", s(args.corner_threshold));
    push("splice_threshold", s(args.splice_threshold));
    push("color_mode", args.color_mode);
    push("hierarchical", args.hierarchical);
    push("path_simplify", args.path_simplify);
    push("layer_difference", s(args.layer_difference));
    push(
        "length_threshold",
        args.length_threshold.map(|v| v.to_string()),
    );
    push("max_iterations", u(args.max_iterations));
    push("path_precision", args.path_precision.map(|v| v.to_string()));
    push("spline_max_mp", args.spline_max_mp.map(|v| v.to_string()));
    push(
        "optimize",
        if args.no_optimize {
            Some("false".into())
        } else {
            args.optimize.map(|v| v.to_string())
        },
    );
    push("optimize_preset", args.optimize_preset);
    push("multipass", args.multipass.then(|| "true".into()));
    push("multipass_iterations", u(args.multipass_iterations));

    VectorizeOpts {
        output: args.output,
        output_dir: args.output_dir,
        output_template: args.output_template,
        format: args.format,
        options,
        recursive: args.recursive,
        json: args.json || args.json_pretty,
        json_pretty: args.json_pretty,
        no_progress: args.no_progress,
        dry_run: args.dry_run,
        limits: ImageLimits {
            max_dimension: args.max_dimension,
        },
    }
}

/// Make `vectorize` the default subcommand, so the documented short form
/// `covecto input.png -o out.svg` works the same as `covecto vectorize ...`.
fn with_default_subcommand(mut args: Vec<std::ffi::OsString>) -> Vec<std::ffi::OsString> {
    const PASSTHROUGH: &[&str] = &[
        "vectorize",
        "serve",
        "help",
        "-h",
        "--help",
        "-V",
        "--version",
    ];
    if let Some(first) = args.get(1) {
        let first_str = first.to_string_lossy();
        if !PASSTHROUGH.contains(&first_str.as_ref()) && looks_like_input(first) {
            args.insert(1, "vectorize".into());
        }
    }
    args
}

/// Whether the first argument is an input (or a vectorize flag) rather than a
/// mistyped subcommand. A bare word like `serv` that is not an existing path
/// is left to clap, which reports "unrecognized subcommand" and suggests
/// `serve`, instead of failing later with "file not found".
fn looks_like_input(arg: &std::ffi::OsStr) -> bool {
    let s = arg.to_string_lossy();
    s.starts_with('-')
        || std::path::Path::new(arg).exists()
        || s.contains(['.', '/', '\\', '*', '?', '['])
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse_from(with_default_subcommand(std::env::args_os().collect()));

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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        let args = args.iter().map(std::ffi::OsString::from).collect();
        Cli::try_parse_from(with_default_subcommand(args))
    }

    fn optimize_of(args: &[&str]) -> bool {
        match parse(args).unwrap().command {
            Commands::Vectorize(a) => from_args(*a).to_config().unwrap().optimize,
            Commands::Serve { .. } => unreachable!(),
        }
    }

    #[test]
    fn optimize_flag_defaults_on_and_can_be_disabled() {
        assert!(optimize_of(&["covecto", "vectorize", "in.png"]));
        assert!(optimize_of(&[
            "covecto",
            "vectorize",
            "in.png",
            "--optimize"
        ]));
        assert!(optimize_of(&[
            "covecto",
            "vectorize",
            "--optimize",
            "in.png"
        ]));
        assert!(!optimize_of(&[
            "covecto",
            "vectorize",
            "in.png",
            "--optimize=false"
        ]));
        assert!(!optimize_of(&[
            "covecto",
            "vectorize",
            "in.png",
            "--no-optimize"
        ]));
        assert!(
            parse(&[
                "covecto",
                "vectorize",
                "x.png",
                "--optimize",
                "--no-optimize"
            ])
            .is_err()
        );
    }

    fn config_of(args: &[&str]) -> anyhow::Result<VectorizeConfig> {
        match parse(args).unwrap().command {
            Commands::Vectorize(a) => from_args(*a).to_config(),
            Commands::Serve { .. } => unreachable!(),
        }
    }

    #[test]
    fn only_typed_flags_are_explicit_options() {
        // No flag typed: nothing is explicit, core defaults apply.
        let c = config_of(&["covecto", "in.png"]).unwrap();
        assert_eq!(c.engine, covecto_core::Engine::Auto);
        // An untyped --engine must not count as explicit "auto", so the
        // Profile can choose the Engine; a typed flag beats the Profile.
        let c = config_of(&[
            "covecto",
            "in.png",
            "--profile",
            "icon",
            "--corner-threshold",
            "120",
        ])
        .unwrap();
        assert_eq!(c.engine, covecto_core::Engine::Spline);
        assert_eq!(c.corner_threshold, Some(120));
        let c = config_of(&[
            "covecto",
            "in.png",
            "--engine",
            "pixel-exact",
            "--profile",
            "icon",
        ])
        .unwrap();
        assert_eq!(c.engine, covecto_core::Engine::PixelExact);
    }

    #[test]
    fn option_errors_use_flag_spelling() {
        let e = config_of(&["covecto", "in.png", "--color-mode", "purple"])
            .unwrap_err()
            .to_string();
        assert!(
            e.contains("--color-mode") && e.contains("color, binary"),
            "{e}"
        );
        let e = config_of(&["covecto", "in.png", "--profile", "icn"])
            .unwrap_err()
            .to_string();
        assert!(e.contains("--profile") && e.contains("icon, logo"), "{e}");
    }

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures")
            .join(name)
    }

    fn opts_of(args: &[&str]) -> VectorizeOpts {
        match parse(args).unwrap().command {
            Commands::Vectorize(a) => from_args(*a),
            Commands::Serve { .. } => unreachable!(),
        }
    }

    #[test]
    fn preview_matches_core_selection() {
        // A configured Engine is shown without reading the file at all.
        let spline = config_of(&["covecto", "x.png", "--engine", "spline"]).unwrap();
        assert_eq!(
            preview_engine(
                Path::new("/does/not/exist.png"),
                &spline,
                ImageLimits::default()
            )
            .unwrap(),
            "spline"
        );
        let profile = config_of(&["covecto", "x.png", "--profile", "icon"]).unwrap();
        assert_eq!(
            preview_engine(
                Path::new("/does/not/exist.png"),
                &profile,
                ImageLimits::default()
            )
            .unwrap(),
            "spline"
        );
        // Auto: the preview is exactly core's decision for this image.
        let auto = config_of(&["covecto", "x.png"]).unwrap();
        for name in [
            "icon-16x16.png",
            "photo-gradient-64.png",
            "shapes-512x512.png",
        ] {
            let file = fixture(name);
            let expected =
                select_engine(&auto, &load_image(&file, ImageLimits::default()).unwrap());
            assert_eq!(
                preview_engine(&file, &auto, ImageLimits::default()).unwrap(),
                format!("{expected} (auto)"),
                "{name}"
            );
        }
        assert!(
            preview_engine(
                Path::new("/does/not/exist.png"),
                &auto,
                ImageLimits::default()
            )
            .is_err()
        );
    }

    /// A fresh temp directory, removed on drop (also when a test panics).
    struct TempDir(PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let dir = std::env::temp_dir()
                .join(format!("covecto-cli-{tag}-{}-{nanos}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn unreadable_files_fail_dry_run_and_batch() {
        let tmp = TempDir::new("unreadable");
        let dir = tmp.0.clone();
        let input = dir.join("in");
        let out = dir.join("out");
        std::fs::create_dir_all(&input).unwrap();
        std::fs::copy(fixture("icon-16x16.png"), input.join("good.png")).unwrap();
        std::fs::write(input.join("broken.png"), b"not a png").unwrap();
        let (i, o) = (input.to_str().unwrap(), out.to_str().unwrap());

        let dry = opts_of(&["covecto", i, "-o", o, "--dry-run", "--no-progress"]);
        let err = cmd_vectorize(&input, &dry).unwrap_err().to_string();
        assert!(err.contains("1 of 2 files could not be read"), "{err}");
        assert!(
            !out.exists(),
            "a dry run writes nothing, not even the output dir"
        );

        let real = opts_of(&["covecto", i, "-o", o, "--no-progress"]);
        let err = cmd_vectorize(&input, &real).unwrap_err().to_string();
        assert!(err.contains("1 of 2 files failed"), "{err}");
        assert!(
            out.join("good.svg").exists(),
            "good files are still converted"
        );
    }

    #[test]
    fn max_dimension_limits_cli_loads() {
        // The CLI applies the same pre-decode limit as the API (default 8192).
        assert_eq!(
            opts_of(&["covecto", "x.png"]).limits,
            ImageLimits::default()
        );
        let tmp = TempDir::new("maxdim");
        let input = fixture("icon-32x32.png");
        let out = tmp.0.join("o.svg");
        let (i, o) = (input.to_str().unwrap(), out.to_str().unwrap());

        let small = opts_of(&["covecto", i, "-o", o, "--max-dimension", "16"]);
        let err = cmd_vectorize(&input, &small).unwrap_err().to_string();
        assert!(err.contains("too large") && err.contains("32x32"), "{err}");
        assert!(!out.exists());

        let unlimited = opts_of(&["covecto", i, "-o", o, "--max-dimension", "0"]);
        cmd_vectorize(&input, &unlimited).unwrap();
        assert!(out.exists());
    }

    #[test]
    fn every_format_goes_through_core_render() {
        let tmp = TempDir::new("formats");
        let input = fixture("icon-32x32.png");
        let config = config_of(&["covecto", "x.png"]).unwrap();
        for (format, magic) in [
            (OutputFormat::Svg, b"<svg".as_slice()),
            (OutputFormat::Pdf, b"%PDF".as_slice()),
            (OutputFormat::Eps, b"%!PS".as_slice()),
        ] {
            let out = tmp.0.join(format!("o.{}", format.extension()));
            let (result, path) =
                vectorize_single_raw(&input, Some(&out), &config, &format, ImageLimits::default())
                    .unwrap();
            let written = std::fs::read(&path).unwrap();
            assert!(written.starts_with(magic), "{format:?}");
            assert_eq!(result.bytes_written, written.len(), "{format:?}");
            assert_eq!(result.format, format);
        }
    }

    fn leftovers(dir: &Path) -> Vec<String> {
        std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains("covecto-tmp"))
            .collect()
    }

    #[test]
    fn existing_output_survives_a_failed_write() {
        // The render succeeds but moving it into place fails (the target is
        // a directory): the target must be untouched and no temp file left.
        let tmp = TempDir::new("survive");
        let input = fixture("icon-32x32.png");
        let config = config_of(&["covecto", "x.png"]).unwrap();
        let target = tmp.0.join("taken.pdf");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("keep.txt"), b"keep").unwrap();
        let res = vectorize_single_raw(
            &input,
            Some(&target),
            &config,
            &OutputFormat::Pdf,
            ImageLimits::default(),
        );
        assert!(res.is_err());
        assert_eq!(std::fs::read(target.join("keep.txt")).unwrap(), b"keep");
        assert!(leftovers(&tmp.0).is_empty(), "{:?}", leftovers(&tmp.0));

        // An unreadable input never touches an existing output file.
        let existing = tmp.0.join("o.svg");
        std::fs::write(&existing, b"previous").unwrap();
        let broken = tmp.0.join("broken.png");
        std::fs::write(&broken, b"not a png").unwrap();
        let res = vectorize_single_raw(
            &broken,
            Some(&existing),
            &config,
            &OutputFormat::Svg,
            ImageLimits::default(),
        );
        assert!(res.is_err());
        assert_eq!(std::fs::read(&existing).unwrap(), b"previous");
    }

    #[test]
    fn successful_write_replaces_existing_output() {
        let tmp = TempDir::new("replace");
        let input = fixture("icon-32x32.png");
        let config = config_of(&["covecto", "x.png"]).unwrap();
        let out = tmp.0.join("o.svg");
        std::fs::write(&out, b"previous").unwrap();
        vectorize_single_raw(
            &input,
            Some(&out),
            &config,
            &OutputFormat::Svg,
            ImageLimits::default(),
        )
        .unwrap();
        assert!(std::fs::read(&out).unwrap().starts_with(b"<svg"));
        assert!(leftovers(&tmp.0).is_empty());
    }

    #[test]
    fn vectorize_is_the_default_subcommand() {
        let cli = parse(&["covecto", "in.png", "-o", "out.svg"]).unwrap();
        assert!(matches!(cli.command, Commands::Vectorize(_)));
        let cli = parse(&["covecto", "--engine", "spline", "in.png"]).unwrap();
        assert!(matches!(cli.command, Commands::Vectorize(_)));
        let cli = parse(&["covecto", "serve", "--port", "8080"]).unwrap();
        assert!(matches!(cli.command, Commands::Serve { port: 8080 }));
        // Paths and globs without an extension still route to vectorize.
        let cli = parse(&["covecto", "icons/", "-o", "out/"]).unwrap();
        assert!(matches!(cli.command, Commands::Vectorize(_)));
        let cli = parse(&["covecto", "*.png"]).unwrap();
        assert!(matches!(cli.command, Commands::Vectorize(_)));
        // #60: a mistyped subcommand is clap's error (with a suggestion),
        // not a vectorize run on a file named "serv".
        let err = parse(&["covecto", "serv"]).err().expect("must not parse");
        assert_eq!(err.kind(), clap::error::ErrorKind::InvalidSubcommand);
        assert!(err.to_string().contains("serve"), "{err}");
        assert!(parse(&["covecto", "--version"]).is_err()); // clap's DisplayVersion
    }
}
