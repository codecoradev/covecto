use axum::body::Bytes;
use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use covecto_core::{
    Engine, OptimizeConfig, OptimizePreset, OutputFormat, VectorizeConfig, VectorizeRequest,
    convert_output, load_image_from_bytes_with_limit, optimize_svg, vectorize as core_vectorize,
};
use serde::{Deserialize, Serialize};
use tracing::info;

use super::state::AppState;

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

#[derive(Deserialize)]
struct VectorizeParams {
    format: Option<String>,
    engine: Option<String>,
    profile: Option<String>,
    optimize: Option<bool>,
    optimize_preset: Option<String>,
    multipass: Option<bool>,
    multipass_iterations: Option<usize>,
    preset: Option<String>,
    color_precision: Option<i32>,
    filter_speckle: Option<usize>,
    corner_threshold: Option<i32>,
    splice_threshold: Option<i32>,
    hierarchical: Option<String>,
    path_simplify: Option<String>,
    layer_difference: Option<i32>,
    length_threshold: Option<f64>,
    max_iterations: Option<usize>,
    path_precision: Option<u32>,
    color_mode: Option<String>,
    spline_max_mp: Option<f64>,
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
    let mut params = VectorizeParams {
        format: None,
        engine: None,
        profile: None,
        optimize: None,
        optimize_preset: None,
        multipass: None,
        multipass_iterations: None,
        preset: None,
        color_precision: None,
        filter_speckle: None,
        corner_threshold: None,
        splice_threshold: None,
        hierarchical: None,
        path_simplify: None,
        layer_difference: None,
        length_threshold: None,
        max_iterations: None,
        path_precision: None,
        color_mode: None,
        spline_max_mp: None,
    };
    let mut image_data: Option<Bytes> = None;

    while let Some(field) = multipart.next_field().await.map_err(AppError::Multipart)? {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" || name.is_empty() {
            image_data = Some(field.bytes().await.map_err(AppError::Multipart)?);
        } else {
            let value = field.text().await.map_err(AppError::Multipart)?;
            match name.as_str() {
                "format" => params.format = Some(value),
                "engine" => params.engine = Some(value),
                "optimize" => params.optimize = value.parse().ok(),
                "optimize_preset" => params.optimize_preset = Some(value),
                "multipass" => params.multipass = value.parse().ok(),
                "multipass_iterations" => params.multipass_iterations = value.parse().ok(),
                "preset" => params.preset = Some(value),
                "profile" => params.profile = Some(value),
                "color_precision" => params.color_precision = value.parse().ok(),
                "filter_speckle" => params.filter_speckle = value.parse().ok(),
                "corner_threshold" => params.corner_threshold = value.parse().ok(),
                "splice_threshold" => params.splice_threshold = value.parse().ok(),
                "hierarchical" => params.hierarchical = Some(value),
                "path_simplify" => params.path_simplify = Some(value),
                "layer_difference" => params.layer_difference = value.parse().ok(),
                "length_threshold" => params.length_threshold = value.parse().ok(),
                "max_iterations" => params.max_iterations = value.parse().ok(),
                "path_precision" => params.path_precision = value.parse().ok(),
                "spline_max_mp" => params.spline_max_mp = value.parse().ok(),
                "color_mode" => params.color_mode = Some(value),
                _ => {}
            }
        }
    }

    let data = image_data.ok_or(AppError::BadRequest("No file provided".into()))?;

    let engine = params
        .engine
        .as_deref()
        .unwrap_or("auto")
        .parse::<Engine>()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let opt_preset = match params.optimize_preset.as_deref().unwrap_or("default") {
        "safe" => OptimizePreset::Safe,
        "none" => OptimizePreset::None,
        _ => OptimizePreset::Default,
    };

    let mut config = VectorizeConfig {
        engine,
        optimize: params.optimize.unwrap_or(true),
        optimize_config: OptimizeConfig {
            preset: opt_preset,
            multipass: params.multipass.unwrap_or(false),
            multipass_iterations: params.multipass_iterations.unwrap_or(10),
        },
        spline_preset: params
            .preset
            .as_deref()
            .map(|s| s.parse())
            .transpose()
            .ok()
            .flatten(),
        color_precision: params.color_precision,
        filter_speckle: params.filter_speckle,
        corner_threshold: params.corner_threshold,
        splice_threshold: params.splice_threshold,
        color_mode: params
            .color_mode
            .as_deref()
            .map(|s| s.parse())
            .transpose()
            .ok()
            .flatten(),
        hierarchical: params
            .hierarchical
            .as_deref()
            .map(|s| s.parse())
            .transpose()
            .ok()
            .flatten(),
        path_simplify_mode: params
            .path_simplify
            .as_deref()
            .map(|s| s.parse())
            .transpose()
            .ok()
            .flatten(),
        layer_difference: params.layer_difference,
        length_threshold: params.length_threshold,
        max_iterations: params.max_iterations,
        path_precision: params.path_precision,
        spline_max_megapixels: Some(state.limits().spline_budget(params.spline_max_mp)),
    };

    // Apply profile preset (overrides individual params)
    if let Some(ref profile) = params.profile {
        covecto_core::apply_profile(profile, &mut config);
    }

    // Validate the output format before doing any heavy work.
    let output_format: OutputFormat = params
        .format
        .as_deref()
        .unwrap_or("svg")
        .parse::<OutputFormat>()
        .map_err(|e: covecto_core::Error| AppError::BadRequest(e.to_string()))?;

    // Decode, vectorize and convert off the async runtime (#55).
    let max_dimension = state.limits().max_dimension;
    let (result, converted) = state
        .run_job(move || -> covecto_core::Result<_> {
            let img = load_image_from_bytes_with_limit(&data, None, max_dimension)?;
            drop(data);
            let req = VectorizeRequest::new(img).with_config(config);
            let result = core_vectorize(&req)?;
            let converted = if output_format == OutputFormat::Svg {
                None
            } else {
                Some(convert_output(&result.svg, output_format)?)
            };
            Ok((result, converted))
        })
        .await
        .map_err(AppError::Job)?
        .map_err(AppError::Core)?;

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
    let mut svg_data: Option<Bytes> = None;
    let mut preset = "default".to_string();
    let mut multipass = false;
    let mut multipass_iterations = 10usize;

    while let Some(field) = multipart.next_field().await.map_err(AppError::Multipart)? {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" || name.is_empty() {
            svg_data = Some(field.bytes().await.map_err(AppError::Multipart)?);
        } else {
            let value = field.text().await.map_err(AppError::Multipart)?;
            match name.as_str() {
                "preset" => preset = value,
                "multipass" => multipass = value.parse().unwrap_or(false),
                "multipass_iterations" => multipass_iterations = value.parse().unwrap_or(10),
                _ => {}
            }
        }
    }

    let data = svg_data.ok_or(AppError::BadRequest("No file provided".into()))?;
    let svg_str = String::from_utf8(data.into())
        .map_err(|e| AppError::BadRequest(format!("Invalid UTF-8: {e}")))?;

    let original_size = svg_str.len();
    let opt_preset = match preset.as_str() {
        "safe" => OptimizePreset::Safe,
        "none" => OptimizePreset::None,
        _ => OptimizePreset::Default,
    };

    let config = OptimizeConfig {
        preset: opt_preset,
        multipass,
        multipass_iterations,
    };

    let optimized = state
        .run_job(move || optimize_svg(&svg_str, &config))
        .await
        .map_err(AppError::Job)?
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

pub enum AppError {
    BadRequest(String),
    Core(covecto_core::Error),
    Multipart(axum::extract::multipart::MultipartError),
    /// The blocking job panicked or was cancelled.
    Job(tokio::task::JoinError),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Core(e @ covecto_core::Error::ImageTooLarge { .. }) => {
                (StatusCode::PAYLOAD_TOO_LARGE, e.to_string())
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
