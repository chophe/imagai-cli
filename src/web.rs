use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path as AxumPath, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use tower_http::cors::{Any, CorsLayer};

use crate::config::Settings;
use crate::core::generate_image_core;
use crate::models::ImageGenerationRequest;
use crate::utils::IMAGE_EXTENSIONS;

const WEB_INTERFACE_HTML: &str = include_str!("../web_interface.html");

#[derive(Clone)]
struct AppState {
    settings: Arc<Settings>,
}

#[derive(Deserialize)]
struct GeneratePayload {
    prompt: String,
    #[serde(default)]
    engine: Option<String>,
    #[serde(default)]
    output: Option<String>,
    #[serde(default = "default_one")]
    n: u32,
    #[serde(default = "default_size")]
    size: String,
    #[serde(default = "default_quality")]
    quality: String,
    #[serde(default = "default_style")]
    style: String,
    #[serde(default = "default_response_format")]
    response_format: String,
    #[serde(default)]
    negative_prompt: Option<String>,
    #[serde(default)]
    seed: Option<u32>,
    #[serde(default)]
    strength: Option<f32>,
    #[serde(default)]
    output_format: Option<String>,
    #[serde(default)]
    aspect_ratio: Option<String>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    auto_filename: bool,
    #[serde(default)]
    random_filename: bool,
    #[serde(default)]
    verbose: bool,
    /// Accepted for image-to-image flows (mirrors the original API surface).
    #[serde(default)]
    input_image: Option<String>,
}

#[derive(Deserialize)]
struct CliPayload {
    command: String,
}

fn default_one() -> u32 {
    1
}
fn default_size() -> String {
    "1024x1024".to_string()
}
fn default_quality() -> String {
    "standard".to_string()
}
fn default_style() -> String {
    "vivid".to_string()
}
fn default_response_format() -> String {
    "b64_json".to_string()
}

/// Build the router for the web UI.
pub fn router(settings: Settings) -> Router {
    let state = AppState {
        settings: Arc::new(settings),
    };
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);
    Router::new()
        .route("/", get(index))
        .route("/api/engines", get(list_engines))
        .route("/api/generate", post(generate))
        .route("/api/generate-cli", post(generate_cli))
        .route("/api/images", get(list_images))
        .route("/api/images/{filename}", get(serve_image))
        .with_state(state)
        .layer(cors)
}

async fn index() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        WEB_INTERFACE_HTML,
    )
}

async fn list_engines(State(state): State<AppState>) -> Json<Value> {
    let engines: Vec<Value> = state
        .settings
        .engine_names()
        .iter()
        .map(|name| {
            let cfg = &state.settings.engines[name];
            json!({
                "name": name,
                "model": cfg.model,
                "base_url": cfg.base_url,
            })
        })
        .collect();
    Json(json!({
        "success": true,
        "engines": engines,
        "default_engine": state.settings.default_engine,
    }))
}

async fn generate(State(state): State<AppState>, Json(payload): Json<GeneratePayload>) -> Response {
    let engine = payload
        .engine
        .clone()
        .or_else(|| state.settings.default_engine.clone());
    let Some(engine) = engine else {
        return Json(json!({
            "success": false,
            "error": "No engine specified and no default engine configured",
        }))
        .into_response();
    };

    if state.settings.get_engine(&engine).is_none() {
        return Json(json!({
            "success": false,
            "error": format!("Engine \"{engine}\" is not configured"),
        }))
        .into_response();
    }

    let mut extra_params: std::collections::HashMap<String, Value> = Default::default();
    if let Some(v) = payload.negative_prompt {
        extra_params.insert("negative_prompt".into(), json!(v));
    }
    if let Some(v) = payload.seed {
        extra_params.insert("seed".into(), json!(v));
    }
    if let Some(v) = payload.strength {
        extra_params.insert("strength".into(), json!(v));
    }
    if let Some(v) = payload.output_format {
        extra_params.insert("output_format".into(), json!(v));
    }
    if let Some(v) = payload.aspect_ratio {
        extra_params.insert("aspect_ratio".into(), json!(v));
    }
    if let Some(v) = payload.mode {
        extra_params.insert("mode".into(), json!(v));
    }
    if let Some(v) = payload.input_image {
        // Mirrors the original: accepted but not consumed by current providers.
        extra_params.insert("input_image".into(), json!(v));
    }

    let request = ImageGenerationRequest {
        prompt: payload.prompt,
        engine,
        output_filename: payload.output,
        n: payload.n.max(1),
        size: payload.size,
        quality: payload.quality,
        style: payload.style,
        response_format: payload.response_format,
        extra_params,
        verbose: payload.verbose,
        auto_filename: payload.auto_filename,
        random_filename: payload.random_filename,
        // Edit behaviour arrives with plan 01-03; the fields exist so the
        // crate compiles against the shared DTO.
        source_image: None,
        ref_images: Vec::new(),
    };

    let results = generate_image_core(&request, &state.settings).await;

    let mut result_items = Vec::new();
    for (i, result) in results.iter().enumerate() {
        let mut item = json!({
            "index": i + 1,
            "success": result.error.is_none(),
            "error": result.error,
        });
        if let Some(err) = &result.error {
            item["error"] = json!(err);
        }
        if let Some(path) = &result.saved_path {
            item["saved_path"] = json!(path);
            if let Ok(bytes) = std::fs::read(path) {
                use base64::Engine;
                let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                let mime = content_type_for(Path::new(path));
                item["image_data"] = json!(format!("data:{mime};base64,{b64}"));
            }
        }
        if let Some(url) = &result.image_url {
            item["image_url"] = json!(url);
        }
        if let Some(b64) = &result.image_b64_json {
            item["image_b64_json"] = json!(b64);
        }
        if let Some(text) = &result.text_content {
            item["text_content"] = json!(text);
        }
        result_items.push(item);
    }

    Json(json!({
        "success": true,
        "results": result_items,
        "command": "Generated using core engine",
    }))
    .into_response()
}

async fn generate_cli(State(state): State<AppState>, Json(payload): Json<CliPayload>) -> Response {
    let command = payload.command;
    let allowed = [
        "imagai",
        "cargo run -- imagai",
        "cargo run --quiet -- imagai",
    ]
    .iter()
    .any(|p| command.trim_start().starts_with(p));
    if !allowed {
        return Json(json!({
            "success": false,
            "error": "Only imagai commands are allowed",
        }))
        .into_response();
    }

    let output = tokio::time::timeout(
        Duration::from_secs(300),
        tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&command)
            .output(),
    )
    .await;

    let output = match output {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => {
            return Json(json!({
                "success": false,
                "error": format!("Command execution failed: {e}"),
            }))
            .into_response();
        }
        Err(_) => {
            return (
                StatusCode::GATEWAY_TIMEOUT,
                Json(json!({
                    "success": false,
                    "error": "Command timed out after 5 minutes",
                })),
            )
                .into_response();
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let success = output.status.success();

    let mut generated_images = Vec::new();
    if success {
        let dir = &state.settings.output_dir;
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    let ext = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.to_lowercase())
                        .unwrap_or_default();
                    if ext == "png" && is_recent(&path) {
                        if let Ok(bytes) = std::fs::read(&path) {
                            use base64::Engine;
                            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                            generated_images.push(json!({
                                "filename": name,
                                "path": path.to_string_lossy(),
                                "data": format!("data:image/png;base64,{b64}"),
                            }));
                        }
                    }
                }
            }
        }
    }

    Json(json!({
        "success": success,
        "command": command,
        "stdout": stdout,
        "stderr": stderr,
        "returncode": output.status.code(),
        "generated_images": generated_images,
    }))
    .into_response()
}

async fn list_images(State(state): State<AppState>) -> Json<Value> {
    let mut images: Vec<Value> = Vec::new();
    let dir = &state.settings.output_dir;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
                .unwrap_or_default();
            if !IMAGE_EXTENSIONS.contains(&ext.as_str()) {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let Ok(meta) = path.metadata() else {
                continue;
            };
            let modified = std::time::UNIX_EPOCH
                + Duration::from_secs(
                    meta.modified()
                        .map(|t| {
                            t.duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_secs())
                                .unwrap_or(0)
                        })
                        .unwrap_or(0),
                );
            let created_iso = datetime_iso(modified);
            let modified_iso = datetime_iso(modified);
            images.push(json!({
                "filename": name,
                "size": meta.len(),
                "created": created_iso,
                "modified": modified_iso,
                "url": format!("/api/images/{name}"),
            }));
        }
    }
    images.sort_by(|a, b| {
        b.get("created")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .cmp(a.get("created").and_then(|v| v.as_str()).unwrap_or(""))
    });
    Json(json!({ "success": true, "images": images }))
}

async fn serve_image(
    State(state): State<AppState>,
    AxumPath(filename): AxumPath<String>,
) -> Response {
    // Path traversal protection: only take the final path component.
    let Some(name) = Path::new(&filename).file_name().and_then(|n| n.to_str()) else {
        return json_not_found();
    };
    let path = state.settings.output_dir.join(name);
    match std::fs::read(&path) {
        Ok(bytes) => {
            let mime = content_type_for(&path);
            (StatusCode::OK, [(header::CONTENT_TYPE, mime)], bytes).into_response()
        }
        Err(_) => json_not_found(),
    }
}

fn json_not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({ "success": false, "error": "Endpoint not found" })),
    )
        .into_response()
}

fn content_type_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    }
}

fn is_recent(path: &Path) -> bool {
    let Ok(meta) = path.metadata() else {
        return false;
    };
    let Ok(modified) = meta.modified() else {
        return false;
    };
    let Ok(elapsed) = std::time::SystemTime::now().duration_since(modified) else {
        return false;
    };
    elapsed.as_secs() < 300
}

fn datetime_iso(t: std::time::SystemTime) -> String {
    match t.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => {
            let secs = d.as_secs() as i64;
            let days = secs.div_euclid(86_400);
            let secs_of_day = secs.rem_euclid(86_400);
            let (y, m, d) = civil_from_days(days);
            format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
                y,
                m,
                d,
                secs_of_day / 3600,
                (secs_of_day % 3600) / 60,
                secs_of_day % 60
            )
        }
        Err(_) => String::new(),
    }
}

/// Convert days since 1970-01-01 to (year, month, day) in the Gregorian calendar.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Start the web server and block until shutdown.
pub async fn serve(settings: Settings, host: &str, port: u16) -> anyhow::Result<()> {
    let app = router(settings);
    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("🎨 Starting Imagai Web Server on http://{addr} (Ctrl+C to stop)");
    axum::serve(listener, app).await?;
    Ok(())
}
