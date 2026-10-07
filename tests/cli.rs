//! End-to-end tests that run the compiled `imagai` binary against a mock
//! OpenAI-compatible server.

mod common;

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

use common::{ChatMode, ImagesMode, MockServer};

/// Build a `imagai` command configured to use the mock server and write into
/// `out_dir`.
fn cmd(mock: &MockServer, out_dir: &Path) -> Command {
    let mut c = Command::cargo_bin("imagai").unwrap();
    c.env("IMAGAI__DEFAULT_ENGINE", "mock")
        .env("IMAGAI__ENGINES__MOCK__API_KEY", "test-key")
        .env("IMAGAI__ENGINES__MOCK__BASE_URL", mock.base_url())
        .env("IMAGAI__ENGINES__MOCK__MODEL", "dall-e-3")
        .env("IMAGAI__OUTPUT_DIR", out_dir);
    c
}

/// Like [`cmd`] but with an edit-capable model. `cmd` defaults to `dall-e-3`,
/// which the capability gate rejects — reusing it unmodified would assert the
/// wrong branch.
fn cmd_edit(mock: &MockServer, out_dir: &Path) -> Command {
    let mut c = cmd(mock, out_dir);
    c.env("IMAGAI__ENGINES__MOCK__MODEL", "gpt-image-1");
    c
}

/// Write a 1x1 PNG to `dir/name` and return its path.
fn seed_source_png(dir: &Path, name: &str) -> PathBuf {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(common::PIXEL_PNG_B64)
        .expect("decode PIXEL_PNG_B64");
    let path = dir.join(name);
    std::fs::write(&path, bytes).expect("seed source png");
    path
}

fn list_pngs(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("read output dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("png"))
        .collect();
    files.sort();
    files
}

fn png_has_metadata(path: &Path, prompt: &str, model: &str, source: Option<&str>) -> bool {
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    let base = bytes.windows(prompt.len()).any(|w| w == prompt.as_bytes())
        && bytes.windows(model.len()).any(|w| w == model.as_bytes())
        && bytes.windows(4).any(|w| w == b"tEXt");
    match source {
        Some(s) => base && bytes.windows(s.len()).any(|w| w == s.as_bytes()),
        None => base,
    }
}

// ---------------------------------------------------------------- basics

#[tokio::test(flavor = "multi_thread")]
async fn help_shows_subcommands() {
    Command::cargo_bin("imagai")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("generate"))
        .stdout(predicate::str::contains("list-engines"))
        .stdout(predicate::str::contains("tui"))
        .stdout(predicate::str::contains("web"));
}

#[tokio::test(flavor = "multi_thread")]
async fn version_flag() {
    Command::cargo_bin("imagai")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[tokio::test(flavor = "multi_thread")]
async fn generate_help_shows_flags() {
    Command::cargo_bin("imagai")
        .unwrap()
        .args(["generate", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--prompt"))
        .stdout(predicate::str::contains("--engine"))
        .stdout(predicate::str::contains("--num-images"))
        .stdout(predicate::str::contains("--auto-filename"));
}

// ---------------------------------------------------------------- engines

#[tokio::test(flavor = "multi_thread")]
async fn list_engines_shows_configured_engines() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    cmd(&mock, out.path())
        .env("IMAGAI__ENGINES__MOCK2__API_KEY", "test-key")
        .env("IMAGAI__ENGINES__MOCK2__MODEL", "gpt-image-1")
        .arg("list-engines")
        .assert()
        .success()
        .stdout(predicate::str::contains("mock"))
        .stdout(predicate::str::contains("mock2"))
        .stdout(predicate::str::contains("✅ Set"))
        .stdout(predicate::str::contains("dall-e-3"))
        .stdout(predicate::str::contains("gpt-image-1"));
}

#[tokio::test(flavor = "multi_thread")]
async fn list_engines_all_fetches_models() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    cmd(&mock, out.path())
        .args(["list-engines", "--all"])
        .assert()
        .success()
        .stdout(predicate::str::contains("dall-e-3"))
        .stdout(predicate::str::contains("gpt-4o"))
        .stdout(predicate::str::contains("stability.stable-image-core-v1:1"));
}

// ---------------------------------------------------------------- generate

#[tokio::test(flavor = "multi_thread")]
async fn generate_saves_b64_image_with_metadata() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let out_path = out.path().to_path_buf();

    cmd(&mock, out.path())
        .args(["generate", "-p", "a red cat", "--verbose"])
        .assert()
        .success()
        .stdout(predicate::str::contains("generated successfully"))
        .stdout(predicate::str::contains("Saved to:"));

    let files = list_pngs(&out_path);
    assert_eq!(files.len(), 1, "one image saved");
    assert!(
        png_has_metadata(&files[0], "a red cat", "dall-e-3", None),
        "saved PNG carries prompt+model tEXt metadata"
    );

    // The request body sent to the mock matches expectations.
    let bodies = mock.requests_for("/v1/images/generations");
    assert_eq!(bodies.len(), 1);
    assert_eq!(bodies[0]["model"], "dall-e-3");
    assert_eq!(bodies[0]["prompt"], "a red cat");
    assert_eq!(bodies[0]["n"], 1);
    assert_eq!(bodies[0]["size"], "1024x1024");
    assert_eq!(bodies[0]["response_format"], "b64_json");
    assert_eq!(bodies[0]["quality"], "standard");
    assert_eq!(bodies[0]["style"], "vivid");
}

#[tokio::test(flavor = "multi_thread")]
async fn generate_downloads_url_images() {
    let mock = MockServer::start().await;
    mock.set_images_mode(ImagesMode::Url { count: 1 });
    let out = TempDir::new().unwrap();
    let out_path = out.path().to_path_buf();

    cmd(&mock, out.path())
        .args(["generate", "-p", "url mode"])
        .assert()
        .success()
        .stdout(predicate::str::contains("generated successfully"));

    let files = list_pngs(&out_path);
    assert_eq!(files.len(), 1, "image downloaded and saved");
    assert!(files[0].metadata().unwrap().len() > 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn generate_multi_image_numbering() {
    let mock = MockServer::start().await;
    mock.set_images_mode(ImagesMode::B64 { count: 2 });
    let out = TempDir::new().unwrap();
    let out_path = out.path().to_path_buf();

    cmd(&mock, out.path())
        .args(["generate", "-p", "two cats", "-n", "2", "-o", "custom.png"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Image 1 generated successfully"))
        .stdout(predicate::str::contains("Image 2 generated successfully"));

    let files = list_pngs(&out_path);
    assert_eq!(
        files,
        vec![out_path.join("custom_1.png"), out_path.join("custom_2.png")]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn generate_random_filename() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let out_path = out.path().to_path_buf();

    cmd(&mock, out.path())
        .args(["generate", "-p", "anything", "--random-filename"])
        .assert()
        .success();

    let files = list_pngs(&out_path);
    assert_eq!(files.len(), 1);
    let name = files[0].file_name().unwrap().to_string_lossy();
    assert!(name.starts_with("image_"), "random filename prefix: {name}");
}

#[tokio::test(flavor = "multi_thread")]
async fn generate_auto_filename_uses_llm() {
    let mock = MockServer::start().await;
    mock.set_images_mode(ImagesMode::B64 { count: 1 });
    mock.set_chat_mode(ChatMode::Text {
        content: "sunset_over_the_hills".to_string(),
    });
    let out = TempDir::new().unwrap();
    let out_path = out.path().to_path_buf();

    cmd(&mock, out.path())
        .env("IMAGAI__ENGINES__FILENAME_GENERATION__API_KEY", "test-key")
        .env(
            "IMAGAI__ENGINES__FILENAME_GENERATION__BASE_URL",
            mock.base_url(),
        )
        .env("IMAGAI__ENGINES__FILENAME_GENERATION__MODEL", "gpt-4o-mini")
        .args(["generate", "-p", "a nice scene", "--auto-filename"])
        .assert()
        .success();

    // An LLM chat call was made for the filename.
    assert_eq!(mock.requests_for("/v1/chat/completions").len(), 1);

    let files = list_pngs(&out_path);
    assert_eq!(files.len(), 1);
    let name = files[0].file_name().unwrap().to_string_lossy();
    assert!(
        name.starts_with("sunset_over_the_hills_"),
        "LLM-provided filename used: {name}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn generate_stability_extra_params_sent() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();

    let mut c = cmd(&mock, out.path());
    c.env(
        "IMAGAI__ENGINES__MOCK__MODEL",
        "stability.stable-image-core-v1:1",
    )
    .args([
        "generate",
        "-p",
        "portrait",
        "--negative-prompt",
        "blurry, text",
        "--seed",
        "42",
        "--aspect-ratio",
        "16:9",
    ])
    .assert()
    .success();

    let bodies = mock.requests_for("/v1/images/generations");
    assert_eq!(bodies.len(), 1);
    let body = &bodies[0];
    assert_eq!(body["negative_prompt"], "blurry, text");
    assert_eq!(body["seed"], 42);
    assert_eq!(body["aspect_ratio"], "16:9");
    assert_eq!(body["mode"], "text-to-image", "default mode injected");
    assert!(
        body.get("size").is_none(),
        "size dropped when aspect_ratio set"
    );
    assert!(body.get("n").is_none(), "n dropped for stability");
}

#[tokio::test(flavor = "multi_thread")]
async fn generate_reports_api_errors() {
    let mock = MockServer::start().await;
    mock.set_images_mode(ImagesMode::Error {
        status: 400,
        message: "bad words in prompt".to_string(),
    });
    let out = TempDir::new().unwrap();

    cmd(&mock, out.path())
        .args(["generate", "-p", "something"])
        .assert()
        .success()
        .stderr(predicate::str::contains("bad words in prompt"));
}

#[tokio::test(flavor = "multi_thread")]
async fn generate_openrouter_gemini_text_response() {
    let mock = MockServer::start().await;
    mock.set_chat_mode(ChatMode::Text {
        content: "here is a textual description".to_string(),
    });
    let out = TempDir::new().unwrap();

    // The URL contains "openrouter.ai" (as a path segment) to trigger the
    // chat-completions branch, while routing to the mock server.
    let openrouter_url = format!("http://{}/openrouter.ai/v1", mock.addr);

    let mut c = cmd(&mock, out.path());
    c.env("IMAGAI__ENGINES__MOCK__BASE_URL", openrouter_url)
        .env("IMAGAI__ENGINES__MOCK__MODEL", "google/gemini-2.0-flash")
        .args(["generate", "-p", "describe this"])
        .assert()
        .success()
        .stdout(predicate::str::contains("here is a textual description"));

    assert_eq!(mock.requests_for("/v1/chat/completions").len(), 1);
}

// ---------------------------------------------------------------- edit

#[tokio::test(flavor = "multi_thread")]
async fn edit_saves_source_derived_image_with_lineage_metadata() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let out_path = out.path().to_path_buf();
    let source = seed_source_png(out.path(), "source.png");

    cmd_edit(&mock, out.path())
        .args(["edit", "--image"])
        .arg(&source)
        .args(["-p", "make it sunset"])
        .assert()
        .success()
        .stdout(predicate::str::contains("generated successfully"))
        .stdout(predicate::str::contains("Saved to:"));

    // D-06: filename derives from the source stem, never from the prompt.
    let saved = out_path.join("source-edit.png");
    assert!(saved.exists(), "source-edit.png written");
    assert!(saved.metadata().unwrap().len() > 0);

    // D-07: Prompt, Model and Source lineage, filename only — no output path.
    assert!(
        png_has_metadata(&saved, "make it sunset", "gpt-image-1", Some("source.png")),
        "saved PNG carries prompt+model+source tEXt metadata"
    );
    let bytes = std::fs::read(&saved).unwrap();
    let out_str = out_path.to_string_lossy().to_string();
    assert!(
        !bytes
            .windows(out_str.len())
            .any(|w| w == out_str.as_bytes()),
        "output directory path is not written into the PNG"
    );

    // Multipart POST to /images/edits carrying an `image[]` part.
    let edits = mock
        .requests()
        .into_iter()
        .filter(|(p, _)| p.contains("images/edits"))
        .count();
    assert_eq!(edits, 1, "exactly one edits request");
    let generations = mock
        .requests()
        .into_iter()
        .filter(|(p, _)| p.contains("images/generations"))
        .count();
    assert_eq!(generations, 0, "no generations request on the edit path");

    let raw = mock.raw_requests_for("images/edits");
    assert_eq!(raw.len(), 1);
    assert!(
        raw[0].windows(7).any(|w| w == b"image[]"),
        "multipart body carries an image[] part name"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn edit_rejects_incapable_engine_without_sending_a_request() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let source = seed_source_png(out.path(), "source.png");

    // dall-e-3 cannot take image input: must fail before any HTTP call, so no
    // credits are spent.
    cmd_edit(&mock, out.path())
        .env("IMAGAI__ENGINES__MOCK__MODEL", "dall-e-3")
        .args(["edit", "--image"])
        .arg(&source)
        .args(["-p", "make it sunset"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains(
            "cannot accept image input for edits",
        ))
        .stderr(predicate::str::contains("dall-e-3"));

    assert!(
        mock.requests().is_empty(),
        "no request may be issued on the incapable-engine path"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn edit_missing_source_file_fails() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();

    cmd_edit(&mock, out.path())
        .args(["edit", "--image"])
        .arg(out.path().join("does-not-exist.png"))
        .args(["-p", "make it sunset"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains(
            "Source image not found or unreadable",
        ));
}

#[tokio::test(flavor = "multi_thread")]
async fn edit_without_image_fails_before_auto_pick_lands() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();

    cmd_edit(&mock, out.path())
        .args(["edit", "-p", "make it sunset"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("--image"));
}

#[tokio::test(flavor = "multi_thread")]
async fn edit_empty_prompt_fails() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let source = seed_source_png(out.path(), "source.png");

    cmd_edit(&mock, out.path())
        .args(["edit", "--image"])
        .arg(&source)
        .args(["-p", ""])
        .write_stdin("")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("Prompt cannot be empty"));
}

#[tokio::test(flavor = "multi_thread")]
async fn edit_unknown_engine_fails() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let source = seed_source_png(out.path(), "source.png");

    cmd_edit(&mock, out.path())
        .args(["edit", "--image"])
        .arg(&source)
        .args(["-p", "make it sunset", "--engine", "nope"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("not configured"));
}

// ---------------------------------------------------------------- errors

#[tokio::test(flavor = "multi_thread")]
async fn generate_without_engine_fails() {
    let _mock = MockServer::start().await;
    let out = TempDir::new().unwrap();

    let mut c = Command::cargo_bin("imagai").unwrap();
    c.env("IMAGAI__OUTPUT_DIR", out.path())
        .args(["generate", "-p", "a prompt"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("No engine specified"));
}

#[tokio::test(flavor = "multi_thread")]
async fn generate_unknown_engine_fails() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();

    let mut c = cmd(&mock, out.path());
    c.args(["generate", "-p", "x", "--engine", "nope"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("not configured"));
}

#[tokio::test(flavor = "multi_thread")]
async fn generate_empty_prompt_fails() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();

    cmd(&mock, out.path())
        .args(["generate", "-p", ""])
        .write_stdin("")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("Prompt cannot be empty"));
}
