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
        // The lineage sidecar also lives in output_dir (D-08).
        .filter(|e| e.file_name() != ".imagai-history.json")
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
async fn list_images_returns_every_output_image_newest_first() {
    let mock = MockServer::start().await;
    mock.set_images_mode(ImagesMode::B64 { count: 1 });
    let out = TempDir::new().unwrap();
    let app = web::router(test_settings(&mock, out.path()));

    // Drive through the real pipeline so both files carry the injected
    // metadata chunks, rather than writing files directly.
    for prompt in ["a first gallery cat", "a second gallery cat"] {
        let payload = json!({"prompt": prompt});
        let (status, value) = post_json(app.clone(), "/api/generate", &payload).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(value["success"], true);
    }

    let (status, value) = get(app.clone(), "/api/images").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["success"], true);
    let images = value["images"].as_array().unwrap();
    assert_eq!(images.len(), 2);

    for entry in images {
        for key in ["filename", "size", "created", "modified", "url"] {
            assert!(entry.get(key).is_some(), "entry carries {key}");
        }
        let filename = entry["filename"].as_str().unwrap();
        assert!(!entry["modified"].as_str().unwrap().is_empty());
        assert_eq!(
            entry["url"].as_str().unwrap(),
            format!("/api/images/{filename}"),
            "url matches its own filename"
        );
    }

    // Newest first, compared on the derived timestamp strings. Both files are
    // written within the same second in CI, so only the non-decreasing
    // property is asserted — a strict ordering would flake.
    let first = images[0]["modified"].as_str().unwrap();
    let second = images[1]["modified"].as_str().unwrap();
    assert!(first >= second, "newest first: {first} >= {second}");
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
async fn edit_endpoint_rejects_traversal_source() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let app = web::router(edit_capable_settings(&mock, out.path()));

    let payload = json!({"prompt": "x", "source": "../../.env"});
    let (status, value) = post_json(app, "/api/edit", &payload).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["success"], false);
    assert!(
        value["error"].as_str().unwrap().contains("plain filename"),
        "explicit refusal, not silent reduction: {}",
        value["error"]
    );
    assert!(mock.requests().is_empty(), "nothing was read or sent");
}

#[tokio::test(flavor = "multi_thread")]
async fn edit_endpoint_rejects_traversal_reference() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    seed_source_png(out.path(), "source.png");
    let app = web::router(edit_capable_settings(&mock, out.path()));

    let payload = json!({"prompt": "x", "source": "source.png", "refs": ["../../Cargo.toml"]});
    let (status, value) = post_json(app, "/api/edit", &payload).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["success"], false);
    assert!(
        value["error"].as_str().unwrap().contains("plain filename"),
        "explicit refusal, not silent reduction: {}",
        value["error"]
    );
    assert!(mock.requests().is_empty(), "nothing was read or sent");
}

#[tokio::test(flavor = "multi_thread")]
async fn edit_endpoint_rejects_traversal_output() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    seed_source_png(out.path(), "source.png");
    let app = web::router(edit_capable_settings(&mock, out.path()));

    let payload = json!({"prompt": "x", "source": "source.png", "output": "../../escaped.png"});
    let (status, value) = post_json(app, "/api/edit", &payload).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["success"], false);
    assert!(mock.requests().is_empty(), "nothing was read or sent");
    // The write side: nothing escaped the output directory.
    let escaped = out.path().join("..").join("escaped.png");
    assert!(!escaped.exists(), "no file written outside output_dir");
    assert!(
        !out.path().parent().unwrap().join("escaped.png").exists(),
        "no file written outside output_dir"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn edit_endpoint_reports_missing_source() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let app = web::router(edit_capable_settings(&mock, out.path()));

    let payload = json!({"prompt": "x", "source": "nope.png"});
    let (status, value) = post_json(app, "/api/edit", &payload).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["success"], false);
    assert!(
        value["error"].as_str().unwrap().contains("nope.png"),
        "error names the file: {}",
        value["error"]
    );
    assert!(mock.requests().is_empty(), "nothing was sent");
}

#[tokio::test(flavor = "multi_thread")]
async fn edit_endpoint_reports_incapable_engine() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    seed_source_png(out.path(), "source.png");
    // Default fixture model `dall-e-3` cannot take image input (D-05).
    let app = web::router(test_settings(&mock, out.path()));

    let payload = json!({"prompt": "x", "source": "source.png"});
    let (status, value) = post_json(app, "/api/edit", &payload).await;
    assert_eq!(status, StatusCode::OK);
    let body = value.to_string();
    let failed = value["success"] == false
        || value["results"]
            .as_array()
            .map(|r| r.iter().any(|e| e["success"] == false))
            .unwrap_or(false);
    assert!(failed, "edit reports failure: {body}");
    assert!(
        body.contains("cannot accept image input"),
        "gate message: {body}"
    );
    assert!(body.contains("dall-e-3"), "names the model: {body}");
    assert!(mock.requests().is_empty(), "no credits spent");
}

#[tokio::test(flavor = "multi_thread")]
async fn edit_endpoint_rejects_more_than_sixteen_images() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let app = web::router(edit_capable_settings(&mock, out.path()));

    let refs: Vec<Value> = (0..16)
        .map(|i| Value::String(format!("ref-{i}.png")))
        .collect();
    let payload = json!({"prompt": "x", "source": "source.png", "refs": refs});
    let (status, value) = post_json(app, "/api/edit", &payload).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["success"], false);
    let error = value["error"].as_str().unwrap();
    assert!(error.contains("17"), "states the requested count: {error}");
    assert!(error.contains("16"), "states the allowed count: {error}");
    assert!(mock.requests().is_empty(), "nothing was read or sent");
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

// ---------------------------------------------------------------- history

/// Seed a `.imagai-history.json` sidecar directly into `dir`, so the endpoint
/// responses are deterministic. No PNGs are written: the nodes read as
/// `missing` (D-20), which is what two of these tests assert.
fn seed_history_store(dir: &Path, records: &[Value]) {
    let json = serde_json::to_string(records).expect("serialize store");
    std::fs::write(dir.join(".imagai-history.json"), json).expect("seed store");
}

#[tokio::test(flavor = "multi_thread")]
async fn history_endpoint_returns_nested_tree() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    seed_history_store(
        out.path(),
        &[
            json!({
                "id": "1a2b3c4d", "parent": null, "filename": "cat.png",
                "prompt": "a cat", "model": "dall-e-3", "created": "2026-10-09T11:40:00"
            }),
            json!({
                "id": "2b3c4d5e", "parent": "1a2b3c4d", "filename": "cat-edit.png",
                "prompt": "make it sunset", "model": "gpt-image-1",
                "created": "2026-10-09T11:41:00"
            }),
        ],
    );
    let app = web::router(test_settings(&mock, out.path()));

    let (status, value) = get(app, "/api/history").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["success"], true);
    assert_eq!(value["nodes"], 2);
    assert_eq!(value["missing"], 2, "neither file exists on disk");

    let roots = value["roots"].as_array().expect("roots array");
    assert_eq!(roots.len(), 1, "one session");
    let root = &roots[0];
    let id = root["id"].as_str().expect("root id");
    assert_eq!(id.len(), 8, "8-hex node id: {id}");
    assert!(id
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
    let children = root["children"].as_array().expect("children array");
    assert_eq!(children.len(), 1);
    assert_eq!(children[0]["parent"], root["id"], "child names its parent");
}

#[tokio::test(flavor = "multi_thread")]
async fn history_endpoint_orders_roots_newest_first() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    seed_history_store(
        out.path(),
        &[
            json!({
                "id": "aaa11111", "parent": null, "filename": "old.png",
                "prompt": "old", "model": "dall-e-3", "created": "2026-10-09T10:00:00"
            }),
            json!({
                "id": "bbb22222", "parent": null, "filename": "new.png",
                "prompt": "new", "model": "dall-e-3", "created": "2026-10-09T12:00:00"
            }),
        ],
    );
    let app = web::router(test_settings(&mock, out.path()));

    let (status, value) = get(app, "/api/history").await;
    assert_eq!(status, StatusCode::OK);
    let roots = value["roots"].as_array().expect("roots array");
    assert_eq!(roots.len(), 2);
    // The fixed-width ISO format makes string ordering chronological.
    let first = roots[0]["created"].as_str().expect("created");
    let second = roots[1]["created"].as_str().expect("created");
    assert!(first > second, "roots newest first: {first} > {second}");

    // Children oldest-first under one root.
    seed_history_store(
        out.path(),
        &[
            json!({
                "id": "aaa11111", "parent": null, "filename": "root.png",
                "prompt": "root", "model": "dall-e-3", "created": "2026-10-09T10:00:00"
            }),
            json!({
                "id": "bbb22222", "parent": "aaa11111", "filename": "kid2.png",
                "prompt": "kid2", "model": "gpt-image-1", "created": "2026-10-09T13:00:00"
            }),
            json!({
                "id": "ccc33333", "parent": "aaa11111", "filename": "kid1.png",
                "prompt": "kid1", "model": "gpt-image-1", "created": "2026-10-09T11:30:00"
            }),
        ],
    );
    let app = web::router(test_settings(&mock, out.path()));
    let (status, value) = get(app, "/api/history").await;
    assert_eq!(status, StatusCode::OK);
    let children = value["roots"][0]["children"].as_array().expect("children");
    assert_eq!(children.len(), 2);
    let first = children[0]["created"].as_str().expect("created");
    let second = children[1]["created"].as_str().expect("created");
    assert!(first < second, "children oldest first: {first} < {second}");
}

#[tokio::test(flavor = "multi_thread")]
async fn history_endpoint_empty_dir() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let app = web::router(test_settings(&mock, out.path()));

    let (status, value) = get(app, "/api/history").await;
    assert_eq!(status, StatusCode::OK);
    // An empty history is a successful response, not an error.
    assert_eq!(value["success"], true);
    assert_eq!(value["nodes"], 0);
    assert_eq!(value["missing"], 0);
    assert!(value["roots"].as_array().expect("roots").is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn history_endpoint_reports_unreadable_store() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    std::fs::write(out.path().join(".imagai-history.json"), b"{not json")
        .expect("write corrupt sidecar");
    let app = web::router(test_settings(&mock, out.path()));

    let (status, value) = get(app, "/api/history").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["success"], false);
    let error = value["error"].as_str().expect("error sentence");
    assert!(
        error.contains("Could not read history at"),
        "the store sentence arrives verbatim: {error}"
    );
    assert!(
        error.contains(".imagai-history.json"),
        "names the path: {error}"
    );
}

// ---------------------------------------------------------------- fork

#[tokio::test(flavor = "multi_thread")]
async fn fork_endpoint_returns_result_items() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    seed_history_store(
        out.path(),
        &[json!({
            "id": "1a2b3c4d", "parent": null, "filename": "cat.png",
            "prompt": "a cat", "model": "dall-e-3", "created": "2026-10-09T11:40:00"
        })],
    );
    seed_source_png(out.path(), "cat.png");
    let app = web::router(edit_capable_settings(&mock, out.path()));

    let payload = json!({"from": "1a2b3c4d", "prompt": "make it sunset"});
    let (status, value) = post_json(app, "/api/fork", &payload).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["success"], true);
    let result = &value["results"][0];
    assert_eq!(result["success"], true);
    let saved_path = result["saved_path"].as_str().expect("saved_path");
    assert!(
        saved_path.ends_with("cat-edit.png"),
        "the existing -edit naming scheme: {saved_path}"
    );
    let image_data = result["image_data"].as_str().expect("base64 preview");
    assert!(image_data.starts_with("data:image/png;base64,"));
    assert!(Path::new(saved_path).is_file(), "file exists on disk");

    // The response carries exactly the keys `result_items_json` produces, so
    // drift from /api/edit is a test failure.
    for key in ["index", "success", "error", "saved_path", "image_data"] {
        assert!(result.get(key).is_some(), "result carries {key}");
    }

    assert_eq!(mock.requests_for("/v1/images/edits").len(), 1);
    assert_eq!(mock.requests_for("/v1/images/generations").len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn fork_endpoint_rejects_traversal_from() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    seed_source_png(out.path(), "cat.png");
    let app = web::router(edit_capable_settings(&mock, out.path()));

    let payload = json!({"prompt": "x", "from": "../../.env"});
    let (status, value) = post_json(app, "/api/fork", &payload).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["success"], false);
    assert!(
        value["error"].as_str().unwrap().contains("plain filename"),
        "explicit refusal, not silent reduction: {}",
        value["error"]
    );
    assert!(mock.requests().is_empty(), "nothing was read or sent");
}

#[tokio::test(flavor = "multi_thread")]
async fn fork_endpoint_rejects_traversal_output() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    seed_source_png(out.path(), "cat.png");
    let app = web::router(edit_capable_settings(&mock, out.path()));

    let payload = json!({"prompt": "x", "from": "cat.png", "output": "../../escaped.png"});
    let (status, value) = post_json(app, "/api/fork", &payload).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["success"], false);
    assert!(mock.requests().is_empty(), "nothing was read or sent");
    // The write side: nothing escaped the output directory.
    assert!(
        !out.path().parent().unwrap().join("escaped.png").exists(),
        "no file written outside output_dir"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn fork_endpoint_rejects_empty_prompt() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    seed_history_store(
        out.path(),
        &[json!({
            "id": "1a2b3c4d", "parent": null, "filename": "cat.png",
            "prompt": "a cat", "model": "dall-e-3", "created": "2026-10-09T11:40:00"
        })],
    );
    seed_source_png(out.path(), "cat.png");
    let app = web::router(edit_capable_settings(&mock, out.path()));

    let payload = json!({"from": "1a2b3c4d", "prompt": ""});
    let (status, value) = post_json(app, "/api/fork", &payload).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["success"], false);
    assert!(mock.requests().is_empty(), "nothing was sent");
}

#[tokio::test(flavor = "multi_thread")]
async fn fork_endpoint_rejects_more_than_sixteen_images() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    seed_history_store(
        out.path(),
        &[json!({
            "id": "1a2b3c4d", "parent": null, "filename": "cat.png",
            "prompt": "a cat", "model": "dall-e-3", "created": "2026-10-09T11:40:00"
        })],
    );
    seed_source_png(out.path(), "cat.png");
    let app = web::router(edit_capable_settings(&mock, out.path()));

    let refs: Vec<Value> = (0..16)
        .map(|i| Value::String(format!("ref-{i}.png")))
        .collect();
    let payload = json!({"from": "1a2b3c4d", "prompt": "x", "refs": refs});
    let (status, value) = post_json(app, "/api/fork", &payload).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["success"], false);
    let error = value["error"].as_str().unwrap();
    assert!(error.contains("17"), "states the requested count: {error}");
    assert!(error.contains("16"), "states the allowed count: {error}");
    assert!(mock.requests().is_empty(), "nothing was read or sent");
}

#[tokio::test(flavor = "multi_thread")]
async fn fork_endpoint_reports_unknown_from() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let app = web::router(edit_capable_settings(&mock, out.path()));

    let payload = json!({"from": "nope.png", "prompt": "x"});
    let (status, value) = post_json(app, "/api/fork", &payload).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["success"], false);
    let error = value["error"].as_str().unwrap();
    assert!(
        error.contains("No history node or image named 'nope.png'"),
        "the resolver is the single source of that wording: {error}"
    );
    assert!(mock.requests().is_empty(), "nothing was sent");
}

#[tokio::test(flavor = "multi_thread")]
async fn history_form_is_served_with_fork_controls() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let app = web::router(test_settings(&mock, out.path()));

    // `web_interface.html` is `include_str!`-ed at compile time, so a serve
    // test proves the edited file is embedded and reaches the browser.
    let (status, value) = get(app, "/").await;
    assert_eq!(status, StatusCode::OK);
    let html = value.as_str().expect("HTML body");
    for needle in [
        "historyForm",
        "history_node",
        "historyBtn",
        "Fork selected node",
        "/api/fork",
        "/api/history",
    ] {
        assert!(html.contains(needle), "served HTML contains {needle}");
    }
}

// ---------------------------------------------------------------- edits

/// Write a 1x1 PNG to `dir/name` and return its path.
fn seed_source_png(dir: &Path, name: &str) -> std::path::PathBuf {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(common::PIXEL_PNG_B64)
        .expect("decode PIXEL_PNG_B64");
    let path = dir.join(name);
    std::fs::write(&path, bytes).expect("seed source png");
    path
}

/// Same crate defaults `cmd_edit` uses, but with an edit-capable model:
/// `dall-e-3` cannot take image input, so the default fixture would exercise
/// the capability bail instead of the happy path.
fn edit_capable_settings(mock: &MockServer, out_dir: &Path) -> Settings {
    let mut settings = test_settings(mock, out_dir);
    settings.engines.get_mut("mock").expect("mock engine").model = Some("gpt-image-1".to_string());
    settings
}

#[tokio::test(flavor = "multi_thread")]
async fn edit_endpoint_saves_edited_image() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    seed_source_png(out.path(), "source.png");
    let app = web::router(edit_capable_settings(&mock, out.path()));

    let payload = json!({"prompt": "make it sunset", "source": "source.png"});
    let (status, value) = post_json(app, "/api/edit", &payload).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["success"], true);
    let result = &value["results"][0];
    assert_eq!(result["success"], true);
    let saved_path = result["saved_path"].as_str().expect("saved_path");
    assert!(
        saved_path.ends_with("source-edit.png"),
        "saved as source-derived name: {saved_path}"
    );
    let image_data = result["image_data"].as_str().expect("base64 preview");
    assert!(image_data.starts_with("data:image/png;base64,"));
    assert!(Path::new(saved_path).is_file(), "file exists on disk");

    // The saved PNG carries the Source lineage chunk naming the source file.
    let saved_bytes = std::fs::read(saved_path).expect("read saved png");
    let saved_text = String::from_utf8_lossy(&saved_bytes);
    assert!(saved_text.contains("source.png"));

    // One edits request, zero generations: the shared core took the edit path.
    assert_eq!(mock.requests_for("/v1/images/edits").len(), 1);
    assert_eq!(mock.requests_for("/v1/images/generations").len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn edit_form_is_served_with_source_and_ref_pickers() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let app = web::router(test_settings(&mock, out.path()));

    // `web_interface.html` is `include_str!`-ed at compile time, so a serve
    // test proves the edited file is embedded and reaches the browser — the
    // strongest automated check available with no browser tooling configured.
    let (status, value) = get(app, "/").await;
    assert_eq!(status, StatusCode::OK);
    let html = value.as_str().expect("HTML body");
    for needle in ["edit_source", "edit_refs", "edit_prompt", "/api/edit"] {
        assert!(html.contains(needle), "served HTML contains {needle}");
    }
}
