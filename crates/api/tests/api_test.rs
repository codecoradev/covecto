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
            format!("Content-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\n")
                .as_bytes(),
        );
        if filename.ends_with(".png") || filename.ends_with(".jpg") || filename.ends_with(".jpeg") {
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

    let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
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

    let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
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
async fn test_vectorize_invalid_file_returns_415() {
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
    assert_eq!(resp.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
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
    let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["request_count"], 1);
}

#[tokio::test]
async fn test_vectorize_oversized_image_returns_413() {
    let state = covecto_api::create_app_state_with_limits(covecto_api::Limits {
        max_concurrency: 1,
        max_dimension: 16,
        spline_max_megapixels: 2.0,
        ..covecto_api::Limits::default()
    });
    let app = covecto_api::app_with_state(state.clone());
    // 32x32 > 16 per side: rejected from the header, before decoding.
    let bytes = fixture_bytes("icon-32x32.png");
    let req = multipart_request("icon-32x32.png", &bytes);

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["error"].as_str().unwrap().contains("too large"));
    assert_eq!(state.request_count(), 0);
}

#[tokio::test]
async fn test_vectorize_translucent_output_is_well_formed() {
    // Regression (#59): the default optimizer used to delete `<g>` groups of
    // translucent PixelExact paths, producing malformed SVG.
    let img = image::RgbaImage::from_fn(4, 2, |x, _| {
        if x < 2 {
            image::Rgba([255, 0, 0, 128])
        } else {
            image::Rgba([0, 0, 255, 128])
        }
    });
    let mut png = std::io::Cursor::new(Vec::new());
    img.write_to(&mut png, image::ImageFormat::Png).unwrap();
    let req = multipart_request("t.png", png.get_ref());

    let resp = app().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let svg = json["svg"].as_str().unwrap();
    assert!(svg.contains("<g fill-opacity="), "{svg}");
    assert_eq!(svg.matches("<path").count(), 2, "{svg}");
    assert_eq!(
        svg.matches("<g").count(),
        svg.matches("</g>").count(),
        "{svg}"
    );
}

#[tokio::test]
async fn test_spline_downscale_server_cap_applies() {
    // Server budget 0.0005 MP (500 px): a 32x32 (1024 px) spline job is
    // traced smaller but keeps its 32x32 display size via viewBox, even when
    // the client asks for a larger budget.
    let state = covecto_api::create_app_state_with_limits(covecto_api::Limits {
        max_concurrency: 1,
        max_dimension: 64,
        spline_max_megapixels: 0.0005,
        ..covecto_api::Limits::default()
    });
    let bytes = fixture_bytes("icon-32x32.png");
    let boundary = "b-spline";
    let body = build_multipart(
        boundary,
        &[
            ("file", "icon-32x32.png", &bytes),
            ("engine", "", b"spline"),
            ("spline_max_mp", "", b"1000"),
        ],
    );
    let req = Request::builder()
        .method("POST")
        .uri("/v1/vectorize")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();
    let resp = covecto_api::app_with_state(state)
        .oneshot(req)
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let svg = json["svg"].as_str().unwrap();
    assert!(
        svg.contains(r#"width="32" height="32" viewBox="0 0 22 22" preserveAspectRatio="none""#),
        "{svg}"
    );
}

#[tokio::test]
async fn test_vectorize_corrupt_image_returns_400() {
    // #60: a recognized but broken image is the client's fault, not a 500.
    let mut bytes = fixture_bytes("icon-32x32.png");
    bytes.truncate(bytes.len() / 2);
    let resp = app()
        .oneshot(multipart_request("broken.png", &bytes))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_saturated_queue_returns_503_before_reading_body() {
    // #60: 1 running slot, no waiting room. While the place is taken, a new
    // request is rejected with 503 + Retry-After; once freed, it succeeds.
    let state = covecto_api::create_app_state_with_limits(covecto_api::Limits {
        max_concurrency: 1,
        max_queue: 0,
        ..covecto_api::Limits::default()
    });
    let held = state.admit().unwrap();
    let bytes = fixture_bytes("icon-32x32.png");

    let resp = covecto_api::app_with_state(state.clone())
        .oneshot(multipart_request("icon-32x32.png", &bytes))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(resp.headers()["retry-after"], "1");
    assert_eq!(
        state.admitted(),
        1,
        "rejected request must not keep a place"
    );
    assert_eq!(state.request_count(), 0);

    drop(held);
    let resp = covecto_api::app_with_state(state.clone())
        .oneshot(multipart_request("icon-32x32.png", &bytes))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(state.admitted(), 0);
}

#[tokio::test]
async fn test_stalled_upload_times_out_and_frees_its_place() {
    // #60 review: the body is read while holding a queue place, so a client
    // that stops sending mid-upload must not keep that place forever.
    use futures_util::stream::{self, StreamExt};
    let state = covecto_api::create_app_state_with_limits(covecto_api::Limits {
        max_concurrency: 1,
        max_queue: 0,
        upload_timeout: std::time::Duration::from_millis(150),
        ..covecto_api::Limits::default()
    });
    let boundary = "stall";
    let head = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a.png\"\r\n\r\npartial"
    );
    let body = stream::once(async move { Ok::<_, std::io::Error>(axum::body::Bytes::from(head)) })
        .chain(stream::pending());
    let req = Request::builder()
        .method("POST")
        .uri("/v1/vectorize")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from_stream(body))
        .unwrap();

    let t = std::time::Instant::now();
    let resp = covecto_api::app_with_state(state.clone())
        .oneshot(req)
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::REQUEST_TIMEOUT);
    assert!(
        t.elapsed() < std::time::Duration::from_secs(2),
        "{:?}",
        t.elapsed()
    );
    assert_eq!(state.admitted(), 0, "the queue place must be released");
}

fn multipart_with_fields(fields: &[(&str, &str)]) -> Request<Body> {
    let boundary = "opts";
    let bytes = fixture_bytes("icon-32x32.png");
    let mut parts: Vec<(&str, &str, &[u8])> = vec![("file", "icon-32x32.png", &bytes)];
    for (name, value) in fields {
        parts.push((name, "", value.as_bytes()));
    }
    let body = build_multipart(boundary, &parts);
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

async fn error_of(resp: axum::response::Response) -> (StatusCode, String) {
    let status = resp.status();
    let body = axum::body::to_bytes(resp.into_body(), 64 * 1024)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    (
        status,
        json["error"].as_str().unwrap_or_default().to_string(),
    )
}

#[tokio::test]
async fn test_invalid_option_values_are_400_not_ignored() {
    // Options resolved by covecto-core: bad values used to be silently dropped.
    for (fields, needle) in [
        (vec![("color_mode", "purple")], "color_mode"),
        (vec![("filter_speckle", "abc")], "filter_speckle"),
        (vec![("profile", "icn")], "icon, logo"),
        (vec![("colour_precision", "4")], "colour_precision"),
        (
            vec![("color_precision", "4"), ("color_precision", "5")],
            "more than once",
        ),
    ] {
        let (status, error) =
            error_of(app().oneshot(multipart_with_fields(&fields)).await.unwrap()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{fields:?}");
        assert!(error.contains(needle), "{fields:?}: {error}");
    }
}

#[tokio::test]
async fn test_profile_with_explicit_option_is_accepted() {
    let resp = app()
        .oneshot(multipart_with_fields(&[
            ("profile", "icon"),
            ("corner_threshold", "120"),
        ]))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_optimize_endpoint_rejects_bad_optimization_level() {
    let boundary = "optlvl";
    let body = build_multipart(
        boundary,
        &[
            (
                "file",
                "a.svg",
                b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
            ),
            ("preset", "", b"aggressive"),
        ],
    );
    let req = Request::builder()
        .method("POST")
        .uri("/v1/optimize")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();
    let (status, error) = error_of(app().oneshot(req).await.unwrap()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error.contains("optimization level"), "{error}");
}
