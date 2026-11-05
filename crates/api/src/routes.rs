use axum::body::Bytes;
use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use covecto_core::{
    ImageLimits, OptimizeConfig, OptimizePreset, OutputFormat, VectorizeRequest, convert_output,
    load_image, optimize_svg, vectorize as core_vectorize,
};
use serde::Serialize;
use tracing::info;

use super::state::{AppState, JobError};

#[derive(Serialize)]
pub struct HealthResponse {
    status: String,
    version: String,
}

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[derive(Serialize)]
pub struct MetricsResponse {
    request_count: u64,
    avg_processing_ms: f64,
}

pub async fn metrics(State(state): State<AppState>) -> Json<MetricsResponse> {
    Json(MetricsResponse {
        request_count: state.request_count(),
        avg_processing_ms: state.avg_processing_ms(),
    })
}

#[derive(Serialize)]
pub struct VectorizeResponse {
    svg: String,
    engine_used: String,
    metadata: serde_json::Value,
}

pub async fn vectorize_handler(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Response, AppError> {
    // Take a queue place before reading the upload (#60): when the server
    // is saturated the body is never buffered.
    let ticket = state.admit().map_err(AppError::from)?;
    // Explicit options as sent; covecto-core resolves and validates them.
    let mut options: Vec<(String, String)> = Vec::new();
    let mut format: Option<String> = None;
    let mut image_data: Option<Bytes> = None;

    // A slow upload holds a queue place, so bound how long reading it may
    // take (#60): otherwise slow clients could keep every place busy.
    tokio::time::timeout(state.limits().upload_timeout, async {
        while let Some(field) = multipart.next_field().await.map_err(AppError::Multipart)? {
            let name = field.name().unwrap_or("").to_string();
            let key = field_key(&name);
            if key == "file" || key.is_empty() {
                if image_data.is_some() {
                    return Err(duplicate_field("file"));
                }
                image_data = Some(field.bytes().await.map_err(AppError::Multipart)?);
            } else {
                let value = field.text().await.map_err(AppError::Multipart)?;
                if key == "format" {
                    if format.replace(value).is_some() {
                        return Err(duplicate_field("format"));
                    }
                } else {
                    // Options (including repeats) are validated by core.
                    options.push((name, value));
                }
            }
        }
        Ok::<(), AppError>(())
    })
    .await
    .map_err(|_| AppError::UploadTimeout)??;

    let data = image_data.ok_or(AppError::BadRequest("No file provided".into()))?;

    // One set of rules for every frontend: precedence (defaults < Preset <
    // Profile < Explicit options) and strict validation -> 400.
    let mut config =
        covecto_core::options::resolve(options.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
    // Server policy, not a domain rule: a client may only lower the budget.
    config.spline_max_megapixels = Some(state.limits().spline_budget(config.spline_max_megapixels));

    // Validate the output format before doing any heavy work.
    let output_format: OutputFormat = format
        .as_deref()
        .unwrap_or("svg")
        .parse::<OutputFormat>()
        .map_err(|e: covecto_core::Error| AppError::BadRequest(e.to_string()))?;

    // Decode, vectorize and convert off the async runtime (#55).
    let max_dimension = state.limits().max_dimension;
    let (result, converted) = state
        .run_job(ticket, move || -> Result<_, AppError> {
            // Decode errors are the client's input (4xx); errors after a
            // successful decode are ours (5xx).
            let img =
                load_image(&data[..], ImageLimits { max_dimension }).map_err(AppError::Decode)?;
            drop(data);
            let req = VectorizeRequest::new(img).with_config(config);
            let result = core_vectorize(&req).map_err(AppError::Core)?;
            let converted = if output_format == OutputFormat::Svg {
                None
            } else {
                Some(convert_output(&result.svg, output_format).map_err(AppError::Core)?)
            };
            Ok((result, converted))
        })
        .await
        .map_err(AppError::from)??;

    state.record_request(result.metadata.processing_time_ms);

    info!(
        "Vectorized: {} engine, {}ms, {} bytes, {} paths",
        result.engine_used,
        result.metadata.processing_time_ms,
        result.metadata.svg_byte_size,
        result.metadata.path_count,
    );

    // Non-SVG formats: return raw bytes with content-type header
    if let Some((bytes, content_type)) = converted {
        return Ok(([("content-type", content_type.as_str())], bytes).into_response());
    }

    // Default: JSON with SVG string
    Ok(Json(VectorizeResponse {
        svg: result.svg,
        engine_used: result.engine_used.to_string(),
        metadata: serde_json::to_value(result.metadata).unwrap_or_default(),
    })
    .into_response())
}
#[derive(Serialize)]
pub struct OptimizeResponse {
    svg: String,
    original_size: usize,
    optimized_size: usize,
    reduction_pct: f64,
}

pub async fn optimize_handler(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<OptimizeResponse>, AppError> {
    let ticket = state.admit().map_err(AppError::from)?;
    let mut svg_data: Option<Bytes> = None;
    let mut preset = OptimizePreset::Default;
    let mut multipass = false;
    let mut multipass_iterations = 10usize;

    // A slow upload holds a queue place, so bound how long reading it may
    // take (#60): otherwise slow clients could keep every place busy.
    tokio::time::timeout(state.limits().upload_timeout, async {
        let mut seen = std::collections::HashSet::new();
        while let Some(field) = multipart.next_field().await.map_err(AppError::Multipart)? {
            let name = field.name().unwrap_or("").to_string();
            let key = field_key(&name);
            let key = if key.is_empty() { "file".to_string() } else { key };
            if !seen.insert(key.clone()) {
                return Err(duplicate_field(&key));
            }
            if key == "file" {
                svg_data = Some(field.bytes().await.map_err(AppError::Multipart)?);
            } else {
                let value = field.text().await.map_err(AppError::Multipart)?;
                // Same strictness as /v1/vectorize: a bad value is a 400,
                // never a silent fallback. `preset` here is the
                // Optimization level, not a (vtracer) Preset.
                let bad = |what: &str| AppError::BadRequest(format!("Invalid {what}: '{value}'"));
                match key.as_str() {
                    "preset" => {
                        preset = value
                            .parse::<OptimizePreset>()
                            .map_err(|_| bad("optimization level (default, safe, none)"))?
                    }
                    "multipass" => {
                        multipass = match value.trim().to_ascii_lowercase().as_str() {
                            "true" | "1" | "yes" | "on" => true,
                            "false" | "0" | "no" | "off" => false,
                            _ => return Err(bad("multipass (true or false)")),
                        }
                    }
                    "multipass_iterations" => {
                        multipass_iterations = value
                            .trim()
                            .parse()
                            .map_err(|_| bad("multipass_iterations (non-negative integer)"))?
                    }
                    other => {
                        return Err(AppError::BadRequest(format!(
                            "Unknown option '{other}'. Valid: file, preset, multipass, multipass_iterations"
                        )));
                    }
                }
            }
        }
        Ok::<(), AppError>(())
    })
    .await
    .map_err(|_| AppError::UploadTimeout)??;

    let data = svg_data.ok_or(AppError::BadRequest("No file provided".into()))?;
    let svg_str = String::from_utf8(data.into())
        .map_err(|e| AppError::BadRequest(format!("Invalid UTF-8: {e}")))?;

    let original_size = svg_str.len();
    let config = OptimizeConfig {
        preset,
        multipass,
        multipass_iterations,
    };

    let optimized = state
        .run_job(ticket, move || optimize_svg(&svg_str, &config))
        .await
        .map_err(AppError::from)?
        .map_err(AppError::Core)?;
    let optimized_size = optimized.len();
    let reduction_pct = if original_size > 0 {
        (1.0 - optimized_size as f64 / original_size as f64) * 100.0
    } else {
        0.0
    };

    state.record_request(0);

    Ok(Json(OptimizeResponse {
        svg: optimized,
        original_size,
        optimized_size,
        reduction_pct,
    }))
}

/// Field names are matched like Option names in core: case-insensitive,
/// `-` and `_` interchangeable, leading `--` ignored.
fn field_key(name: &str) -> String {
    name.trim()
        .trim_start_matches("--")
        .to_ascii_lowercase()
        .replace('-', "_")
}

fn duplicate_field(name: &str) -> AppError {
    AppError::BadRequest(format!("Field '{name}' was sent more than once"))
}

pub enum AppError {
    BadRequest(String),
    Core(covecto_core::Error),
    Multipart(axum::extract::multipart::MultipartError),
    /// The uploaded image could not be decoded (client input).
    Decode(covecto_core::Error),
    /// The job queue is full or no slot freed up in time.
    Unavailable(&'static str),
    /// The request body was not received within the upload timeout.
    UploadTimeout,
    /// The blocking job panicked or was cancelled.
    Job(tokio::task::JoinError),
}

impl From<JobError> for AppError {
    fn from(e: JobError) -> Self {
        match e {
            JobError::Busy => AppError::Unavailable("Server busy: job queue is full"),
            JobError::Timeout => {
                AppError::Unavailable("Server busy: timed out waiting for a job slot")
            }
            JobError::Failed(e) => AppError::Job(e),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Core(e @ covecto_core::Error::ImageTooLarge { .. }) => {
                (StatusCode::PAYLOAD_TOO_LARGE, e.to_string())
            }
            AppError::Decode(e) if e.is_too_large() => {
                (StatusCode::PAYLOAD_TOO_LARGE, e.to_string())
            }
            AppError::Decode(e) if e.is_unsupported_format() => {
                (StatusCode::UNSUPPORTED_MEDIA_TYPE, e.to_string())
            }
            AppError::Decode(e) => (StatusCode::BAD_REQUEST, e.to_string()),
            AppError::UploadTimeout => (
                StatusCode::REQUEST_TIMEOUT,
                "Upload not received within the time limit".to_string(),
            ),
            AppError::Unavailable(msg) => {
                let body = serde_json::json!({ "error": msg });
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    [(axum::http::header::RETRY_AFTER, "1")],
                    Json(body),
                )
                    .into_response();
            }
            AppError::Core(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::Multipart(e) => (StatusCode::BAD_REQUEST, e.to_string()),
            AppError::Job(e) => {
                tracing::error!("background job failed: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal error while processing the request".to_string(),
                )
            }
        };

        let body = serde_json::json!({ "error": message });
        (status, Json(body)).into_response()
    }
}
