//! Library-level tests for `generate_image_core`.

mod common;

use std::collections::HashMap;
use std::path::Path;

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
        // The lineage sidecar also lives in output_dir (D-08).
        .filter(|e| e.file_name() != ".imagai-history.json")
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
async fn generate_writes_history_sidecar() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let out_path = out.path().to_path_buf();
    let mut settings = base_settings(out.path());
    settings.engines.insert(
        "mock".to_string(),
        EngineConfig {
            api_key: "test".to_string(),
            base_url: Some(mock.base_url()),
            model: Some("dall-e-3".to_string()),
        },
    );

    let request = ImageGenerationRequest {
        prompt: "a cat".to_string(),
        engine: "mock".to_string(),
        auto_filename: false,
        random_filename: false,
        ..Default::default()
    };

    let results = generate_image_core(&request, &settings).await;
    assert_eq!(results.len(), 1);
    assert!(
        results[0].error.is_none(),
        "no error: {:?}",
        results[0].error
    );
    let saved = results[0].saved_path.as_ref().expect("saved path");

    // D-08: the sidecar lives inside the configured output_dir.
    let sidecar = out_path.join(".imagai-history.json");
    let text = std::fs::read_to_string(&sidecar).expect("sidecar written");
    let value: serde_json::Value = serde_json::from_str(&text).expect("sidecar parses");
    let records = value.as_array().expect("sidecar holds an array");
    assert_eq!(records.len(), 1, "exactly one record");

    // D-23: the filename is the saved file's basename, never a path.
    let filename = Path::new(saved)
        .file_name()
        .expect("file name")
        .to_string_lossy()
        .to_string();
    assert_eq!(records[0]["filename"], filename.as_str());
    assert_eq!(records[0]["prompt"], "a cat");
    assert_eq!(records[0]["model"], "dall-e-3");

    // D-17: 8 lowercase hex chars.
    let id = records[0]["id"].as_str().expect("id is a string");
    assert_eq!(id.len(), 8, "id is 8 chars: {id}");
    assert!(
        id.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "id is lowercase hex: {id}"
    );
    assert!(records[0]["parent"].is_null(), "a generation is a root");

    // Proven to be the generation path, not an edit.
    assert_eq!(mock.requests_for("/v1/images/edits").len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn edit_writes_child_node_under_its_source() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let out_path = out.path().to_path_buf();
    let mut settings = base_settings(out.path());
    settings.engines.insert(
        "mock".to_string(),
        EngineConfig {
            api_key: "test".to_string(),
            base_url: Some(mock.base_url()),
            model: Some("gpt-image-1".to_string()),
        },
    );

    let request = ImageGenerationRequest {
        prompt: "a cat".to_string(),
        engine: "mock".to_string(),
        auto_filename: false,
        random_filename: false,
        ..Default::default()
    };
    let results = generate_image_core(&request, &settings).await;
    let saved = results[0]
        .saved_path
        .clone()
        .expect("generated image saved");

    let edit_request = ImageGenerationRequest {
        prompt: "make it sunset".to_string(),
        engine: "mock".to_string(),
        source_image: Some(saved.to_string()),
        auto_filename: false,
        random_filename: false,
        ..Default::default()
    };
    let results = generate_image_core(&edit_request, &settings).await;
    assert!(
        results[0].error.is_none(),
        "edit error: {:?}",
        results[0].error
    );
    let edit_saved = results[0].saved_path.clone().expect("edited image saved");

    let text =
        std::fs::read_to_string(out_path.join(".imagai-history.json")).expect("sidecar written");
    let value: serde_json::Value = serde_json::from_str(&text).expect("sidecar parses");
    let records = value.as_array().expect("sidecar holds an array");
    assert_eq!(records.len(), 2, "generation plus edit");

    // D-09: the edit is the child of the node matching its source filename.
    assert_eq!(records[1]["parent"], records[0]["id"]);
    let edit_name = Path::new(&edit_saved)
        .file_name()
        .expect("file name")
        .to_string_lossy()
        .to_string();
    assert!(
        edit_name.ends_with("-edit.png"),
        "D-06 naming scheme: {edit_name}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn out_of_dir_result_is_not_recorded() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let mut settings = base_settings(out.path());
    settings.engines.insert(
        "mock".to_string(),
        EngineConfig {
            api_key: "test".to_string(),
            base_url: Some(mock.base_url()),
            model: Some("dall-e-3".to_string()),
        },
    );

    let request = ImageGenerationRequest {
        prompt: "escape".to_string(),
        engine: "mock".to_string(),
        // `generate_image_core` joins this onto output_dir, so the parent
        // component escapes it (D-22).
        output_filename: Some("../escaped.png".to_string()),
        auto_filename: false,
        random_filename: false,
        ..Default::default()
    };
    let results = generate_image_core(&request, &settings).await;
    assert!(
        results[0].error.is_none(),
        "the image itself still saves: {:?}",
        results[0].error
    );

    // D-22: a result outside output_dir never reaches the store.
    assert!(
        !out.path().join(".imagai-history.json").exists(),
        "no sidecar is written for an out-of-dir result"
    );
    let _ = std::fs::remove_file(out.path().parent().expect("parent").join("escaped.png"));
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

#[tokio::test(flavor = "multi_thread")]
async fn default_filename_with_multiple_images_does_not_collide() {
    // CR-06: the default branch was the only one missing the `suffix_numbered`
    // treatment its siblings use. `generate_filename` embeds a
    // second-resolution timestamp, so with `n: 2` both iterations produced the
    // identical path — one file on disk, two records claiming it, both results
    // reporting success.
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
        prompt: "a cat".to_string(),
        engine: "mock".to_string(),
        n: 2,
        ..Default::default()
    };

    let results = generate_image_core(&request, &settings).await;
    assert_eq!(results.len(), 2);
    let saved: Vec<&str> = results
        .iter()
        .map(|r| r.saved_path.as_deref().expect("saved"))
        .collect();
    assert_ne!(saved[0], saved[1], "two distinct paths");
    assert!(saved[0].ends_with("_1.png"), "{saved:?}");
    assert!(saved[1].ends_with("_2.png"), "{saved:?}");

    let files = list_png_files(&out_path);
    assert_eq!(files.len(), 2, "both images survive: {files:?}");

    // The store records two distinct filenames, not one shared node.
    let text = std::fs::read_to_string(out_path.join(".imagai-history.json")).expect("sidecar");
    let value: serde_json::Value = serde_json::from_str(&text).expect("parses");
    let records = value.as_array().expect("array");
    assert_eq!(records.len(), 2);
    assert_ne!(records[0]["filename"], records[1]["filename"]);
}

fn list_png_files(dir: &std::path::Path) -> Vec<String> {
    let mut files: Vec<String> = std::fs::read_dir(dir)
        .expect("read output dir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".png"))
        .collect();
    files.sort();
    files
}
