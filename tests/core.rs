//! Library-level tests for `generate_image_core`.

mod common;

use std::collections::HashMap;

use tempfile::TempDir;

use common::{ChatMode, MockServer};
use imagai::config::{EngineConfig, Settings};
use imagai::core::generate_image_core;
use imagai::models::ImageGenerationRequest;

fn base_settings(out_dir: &std::path::Path) -> Settings {
    Settings {
        output_dir: out_dir.to_path_buf(),
        default_engine: Some("mock".to_string()),
        engines: HashMap::new(),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn unknown_engine_returns_error_response() {
    let out = TempDir::new().unwrap();
    let settings = base_settings(out.path());

    let request = ImageGenerationRequest {
        prompt: "x".to_string(),
        engine: "missing".to_string(),
        ..Default::default()
    };

    let results = generate_image_core(&request, &settings).await;
    assert_eq!(results.len(), 1);
    let err = results[0].error.as_deref().expect("error message");
    assert!(err.contains("not configured"), "{err}");
    assert!(err.contains("Available engines"));
}

#[tokio::test(flavor = "multi_thread")]
async fn openrouter_chat_image_model_saves_png() {
    let mock = MockServer::start().await;
    mock.set_chat_mode(ChatMode::Image);
    let out = TempDir::new().unwrap();
    let out_path = out.path().to_path_buf();

    // The URL contains "openrouter.ai" (as a path segment) to trigger the
    // chat-completions branch, while routing to the mock server.
    let openrouter_url = format!("http://{}/openrouter.ai/v1", mock.addr);
    let settings = Settings {
        output_dir: out_path.clone(),
        default_engine: Some("gem".to_string()),
        engines: HashMap::from([(
            "gem".to_string(),
            EngineConfig {
                api_key: "test".to_string(),
                base_url: Some(openrouter_url),
                model: Some("gemini-2.0-flash-image".to_string()),
            },
        )]),
    };

    let request = ImageGenerationRequest {
        prompt: "draw something".to_string(),
        engine: "gem".to_string(),
        ..Default::default()
    };

    let results = generate_image_core(&request, &settings).await;
    assert_eq!(results.len(), 1);
    assert!(
        results[0].error.is_none(),
        "no error: {:?}",
        results[0].error
    );
    assert!(results[0].saved_path.is_some(), "image saved from data URL");

    // Exactly one chat request, zero images-API requests.
    assert_eq!(mock.requests_for("/v1/chat/completions").len(), 1);
    assert_eq!(mock.requests_for("/v1/images/generations").len(), 0);

    let files: Vec<_> = std::fs::read_dir(&out_path)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(files.len(), 1, "png written to output dir");
}

#[tokio::test(flavor = "multi_thread")]
async fn edit_via_openrouter_chat_sends_data_uri_parts() {
    let mock = MockServer::start().await;
    mock.set_chat_mode(ChatMode::Image);
    let out = TempDir::new().unwrap();
    let out_path = out.path().to_path_buf();

    let seed = |name: &str| -> std::path::PathBuf {
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(common::PIXEL_PNG_B64)
            .expect("decode PIXEL_PNG_B64");
        let p = out_path.join(name);
        std::fs::write(&p, bytes).expect("seed png");
        p
    };
    let source = seed("source.png");
    let ref_a = seed("ref-a.png");
    let ref_b = seed("ref-b.png");

    // The URL contains "openrouter.ai" so the substring routing picks the chat
    // branch while traffic still reaches the mock.
    let openrouter_url = format!("http://{}/openrouter.ai/v1", mock.addr);
    let settings = Settings {
        output_dir: out_path.clone(),
        default_engine: Some("gem".to_string()),
        engines: HashMap::from([(
            "gem".to_string(),
            EngineConfig {
                api_key: "test".to_string(),
                base_url: Some(openrouter_url),
                model: Some("gemini-2.0-flash-image".to_string()),
            },
        )]),
    };

    let request = ImageGenerationRequest {
        prompt: "blend them".to_string(),
        engine: "gem".to_string(),
        source_image: Some(source.to_string_lossy().to_string()),
        ref_images: vec![
            ref_a.to_string_lossy().to_string(),
            ref_b.to_string_lossy().to_string(),
        ],
        ..Default::default()
    };

    let results = generate_image_core(&request, &settings).await;
    assert_eq!(results.len(), 1);
    assert!(
        results[0].error.is_none(),
        "no error: {:?}",
        results[0].error
    );
    assert!(results[0].saved_path.is_some(), "image saved");

    let bodies = mock.requests_for("/v1/chat/completions");
    assert_eq!(bodies.len(), 1);
    let content = &bodies[0]["messages"][0]["content"];
    let content = content.as_array().expect("content is an array");
    assert_eq!(content.len(), 4, "text + source + two refs");
    assert_eq!(content[0]["type"], "text", "text part first");
    for part in &content[1..] {
        assert_eq!(part["type"], "image_url");
        let url = part["image_url"]["url"].as_str().expect("url string");
        assert!(
            url.starts_with("data:image/png;base64,"),
            "local file sent as a data URI"
        );
    }

    // The chat engine must never have touched the multipart endpoint.
    assert_eq!(mock.requests_for("/v1/images/edits").len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn explicit_output_with_multiple_images() {
    let mock = MockServer::start().await;
    mock.set_images_mode(common::ImagesMode::B64 { count: 2 });
    let out = TempDir::new().unwrap();
    let out_path = out.path().to_path_buf();
    let settings = Settings {
        output_dir: out_path.clone(),
        default_engine: Some("mock".to_string()),
        engines: HashMap::from([(
            "mock".to_string(),
            EngineConfig {
                api_key: "test".to_string(),
                base_url: Some(mock.base_url()),
                model: Some("dall-e-3".to_string()),
            },
        )]),
    };

    let request = ImageGenerationRequest {
        prompt: "x".to_string(),
        engine: "mock".to_string(),
        output_filename: Some("out.png".to_string()),
        n: 2,
        ..Default::default()
    };

    let results = generate_image_core(&request, &settings).await;
    assert_eq!(results.len(), 2);
    for r in &results {
        assert!(r.error.is_none());
        assert!(r.saved_path.is_some());
    }
    assert!(out_path.join("out_1.png").exists());
    assert!(out_path.join("out_2.png").exists());
}
