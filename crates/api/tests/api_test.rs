//! Integration tests for covecto-api HTTP endpoints.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use covecto_api::{app_with_state, create_app_state};
use std::path::Path;
use tower::ServiceExt;

fn fixtures_dir() -> String {
    // Tests run from crates/api/, fixtures are at workspace root tests/fixtures/
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let workspace_root = Path::new(manifest_dir).parent().unwrap().parent().unwrap();
    format!("{}/tests/fixtures", workspace_root.display())
}

fn fixture_bytes(name: &str) -> Vec<u8> {
    let path = format!("{}/{}", fixtures_dir(), name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read fixture {name}: {e}"))
}

fn app() -> axum::Router {
    let state = create_app_state();
    app_with_state(state)
}

/// Build a properly-encoded multipart request body for file upload.
fn build_multipart(boundary: &str, parts: &[(&str, &str, &[u8])]) -> Vec<u8> {
    let mut body = Vec::new();
    for (name, filename, data) in parts {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            format!(
                "Content-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\n"
            )
            .as_bytes(),
        );
        if filename.ends_with(".png") || filename.ends_with(".jpg") || filename.ends_with(".jpeg")
        {
            body.extend_from_slice(b"Content-Type: image/png\r\n");
        } else if filename.ends_with(".svg") {
            body.extend_from_slice(b"Content-Type: image/svg+xml\r\n");
        }
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(data);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    body
}

fn multipart_request(file_name: &str, file_bytes: &[u8]) -> Request<Body> {
    let boundary = "test-boundary-12345";
    let body = build_multipart(boundary, &[("file", file_name, file_bytes)]);

    Request::builder()
        .method("POST")
        .uri("/v1/vectorize")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap()
}

#[tokio::test]
async fn test_health_endpoint() {
    let app = app();
    let req = Request::builder()
        .uri("/v1/health")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), 1024)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
    assert!(json["version"].is_string());
}

#[tokio::test]
async fn test_metrics_endpoint() {
    let app = app();
    let req = Request::builder()
        .uri("/v1/metrics")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), 1024)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["request_count"], 0);
    assert_eq!(json["avg_processing_ms"], 0.0);
}

#[tokio::test]
async fn test_vectorize_multipart_success() {
    let app = app();
    let bytes = fixture_bytes("icon-32x32.png");
    let req = multipart_request("icon-32x32.png", &bytes);

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), 10 * 1024 * 1024)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(json["svg"].is_string());
    let svg = json["svg"].as_str().unwrap();
    assert!(svg.contains("<svg"), "response should contain SVG");
    assert!(json["engine_used"].is_string());
    assert!(json["metadata"]["processing_time_ms"].is_number());
}

#[tokio::test]
async fn test_vectorize_multipart_engine_param() {
    let app = app();
    let bytes = fixture_bytes("icon-32x32.png");
    let boundary = "test-boundary-12345";

    // File part
    let mut file_part = Vec::new();
    file_part.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    file_part.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"icon.png\"\r\n",
    );
    file_part.extend_from_slice(b"Content-Type: image/png\r\n");
    file_part.extend_from_slice(b"\r\n");
    file_part.extend_from_slice(&bytes);
    file_part.extend_from_slice(b"\r\n");

    // Engine part (text field, no filename)
    let mut engine_part = Vec::new();
    engine_part.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    engine_part.extend_from_slice(b"Content-Disposition: form-data; name=\"engine\"\r\n");
    engine_part.extend_from_slice(b"\r\n");
    engine_part.extend_from_slice(b"spline");
    engine_part.extend_from_slice(b"\r\n");

    let mut body = file_part;
    body.extend_from_slice(&engine_part);
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

    let req = Request::builder()
        .method("POST")
        .uri("/v1/vectorize")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp_body = axum::body::to_bytes(resp.into_body(), 10 * 1024 * 1024)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&resp_body).unwrap();
    assert_eq!(json["engine_used"], "spline");
}

#[tokio::test]
async fn test_vectorize_no_file_returns_400() {
    let app = app();
    let boundary = "test-boundary-12345";
    let body = format!("--{boundary}--\r\n");

    let req = Request::builder()
        .method("POST")
        .uri("/v1/vectorize")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_vectorize_invalid_engine_returns_400() {
    let app = app();
    let bytes = fixture_bytes("icon-16x16.png");
    let boundary = "test-boundary-12345";

    // File part
    let mut file_part = Vec::new();
    file_part.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    file_part.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"icon.png\"\r\n",
    );
    file_part.extend_from_slice(b"Content-Type: image/png\r\n");
    file_part.extend_from_slice(b"\r\n");
    file_part.extend_from_slice(&bytes);
    file_part.extend_from_slice(b"\r\n");

    // Invalid engine part
    let mut engine_part = Vec::new();
    engine_part.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    engine_part.extend_from_slice(b"Content-Disposition: form-data; name=\"engine\"\r\n");
    engine_part.extend_from_slice(b"\r\n");
    engine_part.extend_from_slice(b"invalid_engine");
    engine_part.extend_from_slice(b"\r\n");

    let mut body = file_part;
    body.extend_from_slice(&engine_part);
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

    let req = Request::builder()
        .method("POST")
        .uri("/v1/vectorize")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_vectorize_invalid_file_returns_500() {
    let app = app();
    let boundary = "test-boundary-12345";

    let mut file_part = Vec::new();
    file_part.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    file_part.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"bad.txt\"\r\n",
    );
    file_part.extend_from_slice(b"Content-Type: text/plain\r\n");
    file_part.extend_from_slice(b"\r\n");
    file_part.extend_from_slice(b"this is not an image");
    file_part.extend_from_slice(b"\r\n");

    let mut body = file_part;
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

    let req = Request::builder()
        .method("POST")
        .uri("/v1/vectorize")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_optimize_endpoint() {
    let app = app();
    let svg_content = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
<!-- comment -->
<title>Test</title>
<rect width="100" height="100" fill="red"/>
</svg>"#;

    let mut file_part = Vec::new();
    let boundary = "test-boundary-12345";
    file_part.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    file_part.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"test.svg\"\r\n",
    );
    file_part.extend_from_slice(b"Content-Type: image/svg+xml\r\n");
    file_part.extend_from_slice(b"\r\n");
    file_part.extend_from_slice(svg_content.as_bytes());
    file_part.extend_from_slice(b"\r\n");

    let mut body = file_part;
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

    let req = Request::builder()
        .method("POST")
        .uri("/v1/optimize")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp_body = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&resp_body).unwrap();

    assert!(json["svg"].is_string());
    assert!(json["original_size"].is_number());
    assert!(json["optimized_size"].is_number());
    assert!(json["reduction_pct"].is_number());
    // Should have removed comment and title
    let optimized = json["svg"].as_str().unwrap();
    assert!(!optimized.contains("<!--"), "comments should be removed");
    assert!(!optimized.contains("<title>"), "title should be removed");
}

#[tokio::test]
async fn test_metrics_increment_after_vectorize() {
    let app = app();

    // First request
    let bytes = fixture_bytes("pixel-1x1.png");
    let req = multipart_request("pixel.png", &bytes);
    let _ = app.clone().oneshot(req).await.unwrap();

    // Check metrics
    let req = Request::builder()
        .uri("/v1/metrics")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 1024)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["request_count"], 1);
}