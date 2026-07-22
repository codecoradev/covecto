//! Covecto HTTP API.

pub(crate) mod routes;
pub(crate) mod state;

pub use state::AppState;

use axum::Router;
use axum::routing::{get, post};
use tower_http::{cors::CorsLayer, limit::RequestBodyLimitLayer, trace::TraceLayer};

/// Build and return the application router.
pub fn app() -> Router {
    let state = state::create_app_state();
    app_with_state(state)
}

/// Build router with provided state (for testing).
pub fn app_with_state(state: AppState) -> Router {
    Router::new()
        .route("/v1/health", get(routes::health))
        .route("/v1/metrics", get(routes::metrics))
        .route("/v1/vectorize", post(routes::vectorize_handler))
        .route("/v1/optimize", post(routes::optimize_handler))
        .layer(TraceLayer::new_for_http())
        .layer(RequestBodyLimitLayer::new(50 * 1024 * 1024))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// Run the server (used by CLI's `serve` command).
pub async fn run_server(port: u16) -> anyhow::Result<()> {
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Listening on {addr}");
    axum::serve(listener, app()).await?;
    Ok(())
}
