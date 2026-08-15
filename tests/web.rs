//! Tests for the web REST API, exercised in-process via `tower::oneshot`.

mod common;

use std::collections::HashMap;
use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use tempfile::TempDir;
use tower::ServiceExt;

use common::{ImagesMode, MockServer};
use imagai::config::{EngineConfig, Settings};
use imagai::web;

fn test_settings(mock: &MockServer, out_dir: &Path) -> Settings {
    Settings {
        output_dir: out_dir.to_path_buf(),
        default_engine: Some("mock".to_string()),
        engines: HashMap::from([(
            "mock".to_string(),
            EngineConfig {
                api_key: "test-key".to_string(),
                base_url: Some(mock.base_url()),
                model: Some("dall-e-3".to_string()),
            },
        )]),
    }
}

async fn get(app: Router, uri: &str) -> (StatusCode, Value) {
    let resp = app
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("send request");
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 2 * 1024 * 1024)
        .await
        .expect("read body");
    let text = String::from_utf8_lossy(&bytes).to_string();
    let json = serde_json::from_str(&text).unwrap_or(Value::String(text));
    (status, json)
}

async fn post_json(app: Router, uri: &str, payload: &Value) -> (StatusCode, Value) {
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .expect("build request"),
        )
        .await
        .expect("send request");
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 4 * 1024 * 1024)
        .await
        .expect("read body");
    let text = String::from_utf8_lossy(&bytes).to_string();
    let json = serde_json::from_str(&text).unwrap_or(Value::String(text));
    (status, json)
}

#[tokio::test(flavor = "multi_thread")]
async fn root_serves_html() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let app = web::router(test_settings(&mock, out.path()));

    let (status, value) = get(app, "/").await;
    assert_eq!(status, StatusCode::OK);
    let html = value.as_str().expect("HTML body");
    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("Imagai"));
}

#[tokio::test(flavor = "multi_thread")]
async fn engines_endpoint() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let app = web::router(test_settings(&mock, out.path()));

    let (status, value) = get(app, "/api/engines").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["success"], true);
    assert_eq!(value["default_engine"], "mock");
    assert_eq!(value["engines"][0]["name"], "mock");
    assert_eq!(value["engines"][0]["model"], "dall-e-3");
}

#[tokio::test(flavor = "multi_thread")]
async fn generate_endpoint_returns_image_preview() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let out_path = out.path().to_path_buf();
    let app = web::router(test_settings(&mock, out.path()));

    let payload = json!({"prompt": "a web cat", "n": 1});
    let (status, value) = post_json(app, "/api/generate", &payload).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["success"], true);
    let result = &value["results"][0];
    assert_eq!(result["success"], true);
    assert!(result["saved_path"].as_str().is_some());
    let image_data = result["image_data"].as_str().expect("base64 preview");
    assert!(image_data.starts_with("data:image/png;base64,"));

    // The file really exists on disk.
    let files: Vec<_> = std::fs::read_dir(&out_path)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(files.len(), 1);

    // And the request reached the mock with correct body.
    let bodies = mock.requests_for("/v1/images/generations");
    assert_eq!(bodies.len(), 1);
    assert_eq!(bodies[0]["prompt"], "a web cat");
}

#[tokio::test(flavor = "multi_thread")]
async fn generate_endpoint_unknown_engine() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let app = web::router(test_settings(&mock, out.path()));

    let payload = json!({"prompt": "x", "engine": "missing"});
    let (status, value) = post_json(app, "/api/generate", &payload).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["success"], false);
    assert!(value["error"].as_str().unwrap().contains("not configured"));
}

#[tokio::test(flavor = "multi_thread")]
async fn generate_endpoint_missing_prompt_is_rejected() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let app = web::router(test_settings(&mock, out.path()));

    let payload = json!({"engine": "mock"});
    let (status, _) = post_json(app, "/api/generate", &payload).await;
    assert!(status == StatusCode::UNPROCESSABLE_ENTITY || status == StatusCode::BAD_REQUEST);
}

#[tokio::test(flavor = "multi_thread")]
async fn generate_endpoint_reports_api_error() {
    let mock = MockServer::start().await;
    mock.set_images_mode(ImagesMode::Error {
        status: 400,
        message: "nope".to_string(),
    });
    let out = TempDir::new().unwrap();
    let app = web::router(test_settings(&mock, out.path()));

    let payload = json!({"prompt": "x"});
    let (status, value) = post_json(app, "/api/generate", &payload).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["results"][0]["success"], false);
    assert!(value["results"][0]["error"]
        .as_str()
        .unwrap()
        .contains("nope"));
}

#[tokio::test(flavor = "multi_thread")]
async fn list_images_and_serve() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let out_path = out.path().to_path_buf();
    let app = web::router(test_settings(&mock, out.path()));

    // Generate one image first so there's something to list.
    let payload = json!({"prompt": "a listed cat"});
    post_json(app.clone(), "/api/generate", &payload).await;

    let (status, value) = get(app.clone(), "/api/images").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["success"], true);
    assert_eq!(value["images"].as_array().unwrap().len(), 1);
    let name = value["images"][0]["filename"].as_str().unwrap();
    let url = value["images"][0]["url"].as_str().unwrap();
    assert_eq!(url, format!("/api/images/{name}"));

    // Serve the image itself.
    let (status, served) = get(app.clone(), &format!("/api/images/{name}")).await;
    assert_eq!(status, StatusCode::OK);
    let body = served.as_str().unwrap();
    assert!(body.starts_with("data:image") || body.starts_with("PNG") || body.contains("PNG"));
    let _ = out_path;
}

#[tokio::test(flavor = "multi_thread")]
async fn serve_image_blocks_traversal() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let app = web::router(test_settings(&mock, out.path()));

    let (status, _) = get(app.clone(), "/api/images/..%2F..%2F.env").await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = get(app, "/api/images/../Cargo.toml").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn generate_cli_rejects_non_imagai_commands() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let app = web::router(test_settings(&mock, out.path()));

    let payload = json!({"command": "rm -rf /"});
    let (status, value) = post_json(app, "/api/generate-cli", &payload).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["success"], false);
    assert!(value["error"]
        .as_str()
        .unwrap()
        .contains("Only imagai commands"));
}

#[tokio::test(flavor = "multi_thread")]
async fn generate_cli_runs_valid_command() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let app = web::router(test_settings(&mock, out.path()));

    // `imagai` may or may not be on PATH; either way the API returns a JSON
    // report rather than an error.
    let payload = json!({"command": "imagai --version"});
    let (status, value) = post_json(app, "/api/generate-cli", &payload).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["command"], "imagai --version");
    assert!(value["stdout"].is_string());
    assert!(value["stderr"].is_string());
    assert!(value.get("returncode").is_some());
}
