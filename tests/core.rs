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
                format: None,
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
                format: None,
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
            format: None,
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
            format: None,
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
            format: None,
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
                format: None,
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
                format: None,
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

/// A declared-`openai` engine pointed at an `openrouter.ai` path segment.
fn declared_openai_gem_settings(mock: &MockServer, out_path: &std::path::Path) -> Settings {
    let openrouter_url = format!("http://{}/openrouter.ai/v1", mock.addr);
    Settings {
        output_dir: out_path.to_path_buf(),
        default_engine: Some("gem".to_string()),
        engines: HashMap::from([(
            "gem".to_string(),
            EngineConfig {
                api_key: "test".to_string(),
                base_url: Some(openrouter_url),
                model: Some("gemini-2.0-flash-image".to_string()),
                format: Some("openai".to_string()),
            },
        )]),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn declared_openai_format_keeps_the_sniffed_transports() {
    // D-29's complement to D-28: a declaration must not change a within-family
    // transport, exactly as a missing declaration must not. Same engine, same
    // mock mode as `openrouter_chat_image_model_saves_png`, plus the
    // declaration.
    let mock = MockServer::start().await;
    mock.set_chat_mode(ChatMode::Image);
    let out = TempDir::new().unwrap();
    let settings = declared_openai_gem_settings(&mock, out.path());

    let request = ImageGenerationRequest {
        prompt: "draw something".to_string(),
        engine: "gem".to_string(),
        ..Default::default()
    };

    let results = generate_image_core(&request, &settings).await;
    assert_eq!(results.len(), 1);
    assert!(results[0].error.is_none(), "{:?}", results[0].error);
    assert!(results[0].saved_path.is_some());

    assert_eq!(mock.requests_for("/v1/chat/completions").len(), 1);
    assert_eq!(mock.requests_for("/v1/images/generations").len(), 0);
    // And it is not the fal adapter's endpoint family either.
    assert_eq!(mock.requests_for("gemini").len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn declared_openai_edit_engine_still_saves_with_lineage() {
    // A `gpt-image-1` engine declared `openai` still takes the multipart edits
    // path and still writes the PNG lineage (criterion 3, D-26, D-29).
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let out_path = out.path().to_path_buf();
    let source = out_path.join("source.png");
    use base64::Engine;
    std::fs::write(
        &source,
        base64::engine::general_purpose::STANDARD
            .decode(common::PIXEL_PNG_B64)
            .expect("decode"),
    )
    .expect("seed png");

    let mut settings = base_settings(out.path());
    settings.engines.insert(
        "mock".to_string(),
        EngineConfig {
            api_key: "test".to_string(),
            base_url: Some(mock.base_url()),
            model: Some("gpt-image-1".to_string()),
            format: Some("openai".to_string()),
        },
    );

    let request = ImageGenerationRequest {
        prompt: "make it sunset".to_string(),
        engine: "mock".to_string(),
        source_image: Some(source.to_string_lossy().to_string()),
        ..Default::default()
    };

    let results = generate_image_core(&request, &settings).await;
    assert_eq!(results.len(), 1);
    assert!(results[0].error.is_none(), "{:?}", results[0].error);
    let saved = results[0].saved_path.as_ref().expect("saved path");
    assert!(saved.ends_with("-edit.png"), "D-06 naming: {saved}");

    // D-07: prompt, model and source filename only — never the path.
    let bytes = std::fs::read(saved).expect("read saved png");
    let has = |needle: &[u8]| bytes.windows(needle.len()).any(|w| w == needle);
    assert!(has(b"tEXt"), "tEXt chunks present");
    assert!(has(b"make it sunset"), "prompt recorded");
    assert!(has(b"gpt-image-1"), "model recorded");
    assert!(has(b"source.png"), "source filename recorded");
    assert_eq!(mock.requests_for("/v1/images/edits").len(), 1);
    assert_eq!(mock.requests_for("/v1/images/generations").len(), 0);
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

#[tokio::test(flavor = "multi_thread")]
async fn edit_against_fal_format_fails_before_any_request() {
    // D-05/D-26/D-30: the capability gate must fire before a socket opens. The
    // empty request set is the executable proof that nothing left the machine —
    // a provider-side refusal would have left one request behind.
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let out_path = out.path().to_path_buf();

    let settings = Settings {
        output_dir: out_path.clone(),
        default_engine: Some("fal".to_string()),
        engines: HashMap::from([(
            "fal".to_string(),
            EngineConfig {
                api_key: "test".to_string(),
                base_url: Some(mock.base_url()),
                model: Some("fal-ai/flux/schnell".to_string()),
                format: Some("fal".to_string()),
            },
        )]),
    };

    let request = ImageGenerationRequest {
        prompt: "make it sunset".to_string(),
        engine: "fal".to_string(),
        // A real temp-dir path so the test does not depend on filesystem
        // details; the gate must fire before this file is ever read.
        source_image: Some(out_path.join("source.png").to_string_lossy().to_string()),
        ..Default::default()
    };

    let results = generate_image_core(&request, &settings).await;
    assert_eq!(results.len(), 1);
    let err = results[0].error.as_deref().expect("error message");
    assert!(err.contains("fal"), "{err}");
    assert!(err.contains("cannot accept image input for edits"), "{err}");
    assert!(
        results[0].saved_path.is_none(),
        "no phantom save accompanies the error"
    );
    assert!(
        mock.requests().is_empty(),
        "the mock recorded no request at all: {:?}",
        mock.requests()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn fal_engine_saves_image_through_the_shared_pipeline() {
    // D-36's checkable property end to end: the request carries the fal keys and
    // no OpenAI images-API key but `prompt`; D-27's: the response maps onto
    // `ImageGenerationResponse`, so the unchanged save path writes it.
    let mock = MockServer::start().await;
    mock.set_fal_mode(common::FalMode::Url { count: 1 });
    let out = TempDir::new().unwrap();
    let out_path = out.path().to_path_buf();

    let settings = Settings {
        output_dir: out_path.clone(),
        default_engine: Some("fal".to_string()),
        engines: HashMap::from([(
            "fal".to_string(),
            EngineConfig {
                api_key: "test".to_string(),
                base_url: Some(mock.base_url()),
                model: Some("fal-ai/flux/schnell".to_string()),
                format: Some("fal".to_string()),
            },
        )]),
    };

    let request = ImageGenerationRequest {
        prompt: "a fal cat".to_string(),
        engine: "fal".to_string(),
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
    let saved = Path::new(saved);

    // D-27: the unchanged save path wrote the PNG lineage metadata itself.
    let bytes = std::fs::read(saved).expect("read saved png");
    assert!(
        bytes.starts_with(&[0x89, b'P', b'N', b'G']),
        "PNG magic bytes"
    );
    let has_chunk = |needle: &[u8]| bytes.windows(needle.len()).any(|w| w == needle);
    assert!(has_chunk(b"tEXt"), "PNG tEXt chunks written");
    assert!(has_chunk(b"a fal cat"), "prompt recorded");
    assert!(has_chunk(b"fal-ai/flux/schnell"), "model recorded");

    // The model id is a URL path, not a body key.
    assert_eq!(mock.requests_for("fal-ai/flux/schnell").len(), 1);
    assert!(mock.requests_for("/v1/images/generations").is_empty());

    let body = &mock.requests_for("fal-ai/flux/schnell")[0];
    assert_eq!(body["prompt"], "a fal cat");
    assert_eq!(body["num_images"], 1);
    assert_eq!(body["image_size"], "square_hd");
    assert_eq!(body["output_format"], "png");
    for openai_key in ["model", "n", "size", "response_format", "quality", "style"] {
        assert!(
            body.get(openai_key).is_none(),
            "no OpenAI images-API key '{openai_key}' in {body}"
        );
    }

    // D-36/T-03-04: the auth scheme is `Key`, not a bearer token.
    assert_eq!(
        mock.auth_for("fal-ai/flux/schnell"),
        vec!["Key test".to_string()]
    );
}

/// A fal engine carrying an API key, a model and a format, pointed at the mock.
fn fal_settings(mock: &MockServer, out_path: &std::path::Path) -> Settings {
    Settings {
        output_dir: out_path.to_path_buf(),
        default_engine: Some("fal".to_string()),
        engines: HashMap::from([(
            "fal".to_string(),
            EngineConfig {
                api_key: "test".to_string(),
                base_url: Some(mock.base_url()),
                model: Some("fal-ai/flux/schnell".to_string()),
                format: Some("fal".to_string()),
            },
        )]),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn fal_data_uri_response_saves_without_a_fetch() {
    let mock = MockServer::start().await;
    mock.set_fal_mode(common::FalMode::DataUri { count: 1 });
    let out = TempDir::new().unwrap();
    let settings = fal_settings(&mock, out.path());

    let request = ImageGenerationRequest {
        prompt: "a fal cat".to_string(),
        engine: "fal".to_string(),
        ..Default::default()
    };

    let results = generate_image_core(&request, &settings).await;
    assert_eq!(results.len(), 1);
    assert!(results[0].error.is_none(), "{:?}", results[0].error);
    assert!(results[0].saved_path.is_some());
    assert_eq!(mock.requests_for("fal-ai/flux/schnell").len(), 1);
    assert!(
        mock.requests_for("/img/").is_empty(),
        "the data URI carries the bytes; no fetch is made: {:?}",
        mock.requests_for("/img/")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn fal_flat_error_body_is_mapped_into_the_envelope() {
    let mock = MockServer::start().await;
    mock.set_fal_mode(common::FalMode::Error {
        status: 504,
        detail: serde_json::json!("Request timed out"),
        error_type: Some("request_timeout".to_string()),
    });
    let out = TempDir::new().unwrap();
    let settings = fal_settings(&mock, out.path());

    let request = ImageGenerationRequest {
        prompt: "a fal cat".to_string(),
        engine: "fal".to_string(),
        ..Default::default()
    };

    let results = generate_image_core(&request, &settings).await;
    assert_eq!(results.len(), 1);
    let err = results[0].error.as_deref().expect("error message");
    assert!(err.contains("fal"), "names the engine: {err}");
    assert!(err.contains("Request timed out"), "{err}");
    assert!(
        err.contains("request_timeout"),
        "the error_type travels too: {err}"
    );
    assert!(err.contains("504"), "the HTTP status travels too: {err}");
    assert!(results[0].saved_path.is_none(), "no phantom save");
    assert!(mock.requests_for("/img/").is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn fal_detail_array_error_body_is_mapped_into_the_envelope() {
    let mock = MockServer::start().await;
    mock.set_fal_mode(common::FalMode::Error {
        status: 422,
        detail: serde_json::json!([
            {"loc": ["body", "prompt"], "msg": "Field 'prompt' is required but was not provided.", "type": "missing"},
            {"loc": ["body", "seed"], "msg": "Input should be a valid integer", "type": "int_parsing"}
        ]),
        error_type: None,
    });
    let out = TempDir::new().unwrap();
    let settings = fal_settings(&mock, out.path());

    let request = ImageGenerationRequest {
        prompt: "a fal cat".to_string(),
        engine: "fal".to_string(),
        ..Default::default()
    };

    let results = generate_image_core(&request, &settings).await;
    assert_eq!(results.len(), 1);
    let err = results[0].error.as_deref().expect("error message");
    assert!(
        err.contains("Field 'prompt' is required but was not provided."),
        "{err}"
    );
    assert!(err.contains("missing"), "{err}");
    assert!(err.contains("Input should be a valid integer"), "{err}");
    assert!(err.contains("int_parsing"), "{err}");
    assert!(err.contains("422"), "{err}");
    assert!(results[0].saved_path.is_none(), "no phantom save");
    assert!(mock.requests_for("/img/").is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn fal_error_body_without_detail_falls_back_to_the_raw_body() {
    // D-37: a body with no usable `detail` falls back to best-effort raw body
    // with the status — never a panic, never a silent success.
    let mock = MockServer::start().await;
    mock.set_fal_mode(common::FalMode::Error {
        status: 500,
        detail: serde_json::json!(null),
        error_type: None,
    });
    let out = TempDir::new().unwrap();
    let settings = fal_settings(&mock, out.path());

    let request = ImageGenerationRequest {
        prompt: "a fal cat".to_string(),
        engine: "fal".to_string(),
        ..Default::default()
    };

    let results = generate_image_core(&request, &settings).await;
    assert_eq!(results.len(), 1);
    let err = results[0].error.as_deref().expect("error message");
    assert!(err.contains("fal"), "{err}");
    assert!(err.contains("500"), "the status travels: {err}");
    assert!(
        err.contains("null") || err.contains("detail") || err.contains('{'),
        "the truncated raw body travels: {err}"
    );
    assert!(results[0].saved_path.is_none(), "no phantom save");
    assert!(mock.requests_for("/img/").is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn fal_unparseable_success_response_bails_naming_engine_status_and_body() {
    let mock = MockServer::start().await;
    mock.set_fal_mode(common::FalMode::Unparseable {
        body: "<html>gateway</html>".to_string(),
    });
    let out = TempDir::new().unwrap();
    let settings = fal_settings(&mock, out.path());

    let request = ImageGenerationRequest {
        prompt: "a fal cat".to_string(),
        engine: "fal".to_string(),
        ..Default::default()
    };

    let results = generate_image_core(&request, &settings).await;
    assert_eq!(results.len(), 1);
    let err = results[0].error.as_deref().expect("error message");
    assert!(err.contains("fal"), "names the engine: {err}");
    assert!(err.contains("200"), "names the HTTP status: {err}");
    assert!(err.contains("unparseable"), "{err}");
    assert!(
        err.contains("<html>gateway</html>"),
        "carries the body prefix: {err}"
    );
    assert!(results[0].saved_path.is_none(), "no phantom save");
    assert!(mock.requests_for("/img/").is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn fal_empty_images_list_reports_no_images() {
    let mock = MockServer::start().await;
    mock.set_fal_mode(common::FalMode::Url { count: 0 });
    let out = TempDir::new().unwrap();
    let settings = fal_settings(&mock, out.path());

    let request = ImageGenerationRequest {
        prompt: "a fal cat".to_string(),
        engine: "fal".to_string(),
        ..Default::default()
    };

    let results = generate_image_core(&request, &settings).await;
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].error.as_deref(),
        Some("API returned no images."),
        "the existing sentence, unchanged"
    );
    assert!(results[0].saved_path.is_none(), "no phantom save");
    assert!(mock.requests_for("/img/").is_empty());
}
