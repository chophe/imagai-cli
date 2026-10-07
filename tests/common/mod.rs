//! Shared test helpers: an in-process mock OpenAI-compatible server used by
//! integration tests.

// Not every helper is used by every test binary that includes this module.
#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::{header, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use axum::{Json, Router};
use serde_json::{json, Value};

pub const PIXEL_PNG_B64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

/// Behavior of `/…/images/generations`.
#[derive(Clone, Debug)]
pub enum ImagesMode {
    /// Return `count` base64 PNG payloads.
    B64 { count: usize },
    /// Return `count` URLs served by the mock.
    Url { count: usize },
    /// Return an HTTP error.
    Error { status: u16, message: String },
}

/// Behavior of `/…/chat/completions`.
#[derive(Clone, Debug)]
pub enum ChatMode {
    /// Return a plain-text message.
    Text { content: String },
    /// Return a `data:image/png;base64,...` message.
    Image,
}

/// Raw request bytes captured per path, for multipart wire-format assertions.
type RawCaptured = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

#[derive(Clone)]
pub struct MockState {
    pub addr: String,
    pub images_mode: Arc<Mutex<ImagesMode>>,
    pub chat_mode: Arc<Mutex<ChatMode>>,
    pub captured: Arc<Mutex<Vec<(String, Value)>>>,
    /// Raw request bytes, so multipart wire-format assertions are possible
    /// without adding a multipart parser.
    pub raw: RawCaptured,
}

/// An OpenAI-compatible mock server bound to an ephemeral port. Routes are
/// matched by path substring, so tests can craft URLs that both point at this
/// server and contain markers such as "openrouter.ai".
pub struct MockServer {
    pub addr: String,
    pub state: MockState,
}

impl MockServer {
    pub async fn start() -> MockServer {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock server");
        let port = listener.local_addr().expect("local addr").port();
        let addr = format!("127.0.0.1:{port}");
        let state = MockState {
            addr: addr.clone(),
            images_mode: Arc::new(Mutex::new(ImagesMode::B64 { count: 1 })),
            chat_mode: Arc::new(Mutex::new(ChatMode::Text {
                content: "mock_filename".to_string(),
            })),
            captured: Arc::new(Mutex::new(Vec::new())),
            raw: Arc::new(Mutex::new(Vec::new())),
        };
        let app = Router::new()
            .route("/{*path}", any(dispatch))
            .with_state(state.clone());
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("mock server run");
        });
        MockServer { addr, state }
    }

    /// Base URL of the form `http://host:port/v1`.
    pub fn base_url(&self) -> String {
        format!("http://{}/v1", self.addr)
    }

    pub fn set_images_mode(&self, mode: ImagesMode) {
        *self.state.images_mode.lock().unwrap() = mode;
    }

    pub fn set_chat_mode(&self, mode: ChatMode) {
        *self.state.chat_mode.lock().unwrap() = mode;
    }

    /// All requests captured so far as `(path, json_body)`.
    pub fn requests(&self) -> Vec<(String, Value)> {
        self.state.captured.lock().unwrap().clone()
    }

    /// Request bodies for a path that contains `path`.
    pub fn requests_for(&self, path: &str) -> Vec<Value> {
        self.requests()
            .into_iter()
            .filter(|(p, _)| p.contains(path))
            .map(|(_, b)| b)
            .collect()
    }

    /// Raw request bytes for a path that contains `path`.
    pub fn raw_requests_for(&self, path: &str) -> Vec<Vec<u8>> {
        self.state
            .raw
            .lock()
            .unwrap()
            .iter()
            .filter(|(p, _)| p.contains(path))
            .map(|(_, b)| b.clone())
            .collect()
    }
}

async fn dispatch(
    State(state): State<MockState>,
    method: Method,
    uri: Uri,
    body: axum::body::Bytes,
) -> Response {
    let path = uri.path().to_string();
    // Multipart bodies are not JSON, so the parsed view falls back to `{}`;
    // the raw view below is what edit wire-format assertions read.
    let parsed: Value = serde_json::from_slice(&body).unwrap_or_else(|_| json!({}));
    state.captured.lock().unwrap().push((path.clone(), parsed));
    state
        .raw
        .lock()
        .unwrap()
        .push((path.clone(), body.to_vec()));

    if path.contains("images/edits") {
        return images_logic(state, &uri).await;
    }
    if path.contains("images/generations") {
        return images_logic(state, &uri).await;
    }
    if path.contains("chat/completions") {
        return chat_logic(state).await;
    }
    if path.starts_with("/img/") {
        return img_file();
    }
    if path.contains("models") {
        return Json(models()).into_response();
    }
    let _ = method;
    (
        StatusCode::NOT_FOUND,
        Json(json!({"error": {"message": "not found"}})),
    )
        .into_response()
}

fn models() -> Value {
    json!({
        "data": [
            {"id": "dall-e-3"},
            {"id": "gpt-image-1"},
            {"id": "gpt-4o"},
            {"id": "stability.stable-image-core-v1:1"},
            {"id": "black-forest-labs/flux-1.1-pro"}
        ]
    })
}

async fn images_logic(state: MockState, uri: &Uri) -> Response {
    // For URL mode we need the request URL host to build image URLs that point
    // back at this mock server.
    let host = uri
        .authority()
        .map(|a| a.to_string())
        .unwrap_or_else(|| state.addr.clone());
    let mode = state.images_mode.lock().unwrap().clone();
    match mode {
        ImagesMode::B64 { count } => {
            let data: Vec<Value> = (0..count)
                .map(|_| json!({"b64_json": PIXEL_PNG_B64, "revised_prompt": "rev"}))
                .collect();
            Json(json!({"data": data})).into_response()
        }
        ImagesMode::Url { count } => {
            let data: Vec<Value> = (0..count)
                .map(|i| json!({"url": format!("http://{host}/img/{i}.png")}))
                .collect();
            Json(json!({"data": data})).into_response()
        }
        ImagesMode::Error { status, message } => (
            StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_REQUEST),
            Json(json!({"error": {"message": message, "type": "test_error"}})),
        )
            .into_response(),
    }
}

async fn chat_logic(state: MockState) -> Response {
    let mode = state.chat_mode.lock().unwrap().clone();
    match mode {
        ChatMode::Text { content } => Json(json!({
            "choices": [{"message": {"content": content}}],
            "usage": {"total_tokens": 42, "input_tokens": 12, "output_tokens": 30}
        }))
        .into_response(),
        ChatMode::Image => Json(json!({
            "choices": [{"message": {"content": format!("data:image/png;base64,{PIXEL_PNG_B64}")}}],
            "usage": {"total_tokens": 42}
        }))
        .into_response(),
    }
}

fn img_file() -> Response {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(PIXEL_PNG_B64)
        .unwrap();
    (StatusCode::OK, [(header::CONTENT_TYPE, "image/png")], bytes).into_response()
}
