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
        .env("IMAGAI__ENGINES__MOCK3__API_KEY", "test-key")
        .env("IMAGAI__ENGINES__MOCK3__MODEL", "fal-ai/flux/schnell")
        .env("IMAGAI__ENGINES__MOCK3__BASE_URL", "http://127.0.0.1:9/fal")
        .env("IMAGAI__ENGINES__MOCK3__FORMAT", "fal")
        .arg("list-engines")
        .assert()
        .success()
        .stdout(predicate::str::contains("mock"))
        .stdout(predicate::str::contains("mock2"))
        .stdout(predicate::str::contains("✅ Set"))
        .stdout(predicate::str::contains("dall-e-3"))
        .stdout(predicate::str::contains("gpt-image-1"))
        // D-32: the format column names the declared format beside the name.
        .stdout(predicate::str::contains("Format"))
        .stdout(predicate::str::contains("openai"))
        .stdout(predicate::str::contains("fal"));
}

#[tokio::test(flavor = "multi_thread")]
async fn list_engines_shows_a_broken_format_declaration_verbatim() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    cmd(&mock, out.path())
        .env("IMAGAI__ENGINES__MOCK4__FORMAT", "bogus")
        .arg("list-engines")
        .assert()
        .success()
        // D-24/T-03-09: the typo is visible in discovery rather than hidden
        // behind a resolved default.
        .stdout(predicate::str::contains("Format"))
        .stdout(predicate::str::contains("bogus"));
}

#[tokio::test(flavor = "multi_thread")]
async fn list_engines_names_the_base_url_each_format_defaults_to() {
    // WR-01: the base-URL column must name the host the engine actually uses, so
    // a fal engine with no BASE_URL — the documented minimal fal configuration —
    // is never presented as running on OpenAI's endpoint. The fal engine carries
    // a key, so the model-fetch block below the table also runs; the gate from
    // BLOCKER-2 is what makes that line safe to see here.
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    cmd(&mock, out.path())
        .env("IMAGAI__ENGINES__FAL__API_KEY", "fal-secret-key-123")
        .env("IMAGAI__ENGINES__FAL__MODEL", "fal-ai/flux/schnell")
        .env("IMAGAI__ENGINES__FAL__FORMAT", "fal")
        // No API key: the row is still rendered, and the fetch loop skips it, so
        // this test never addresses a real OpenAI host.
        .env("IMAGAI__ENGINES__NOURL__MODEL", "dall-e-3")
        .arg("list-engines")
        .assert()
        .success()
        .stdout(predicate::str::contains("N/A (fal.run)"))
        .stdout(predicate::str::contains("N/A (Official OpenAI)"))
        // BLOCKER-2, through the real binary: the operator is told why nothing
        // was fetched rather than being shown a silent or leaked request.
        .stdout(predicate::str::contains("no model list to fetch"));
}

#[tokio::test(flavor = "multi_thread")]
async fn list_engines_clips_a_long_format_declaration_to_its_column() {
    // IN-03: D-24 requires a broken declaration to render verbatim, but a
    // 40-character typo used to shift Base URL, Default Model and API Key Set off
    // their columns. The cell is clipped, the full value stays in `/api/engines`
    // and in the first request's error.
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let typo = "a".repeat(40);
    cmd(&mock, out.path())
        .env("IMAGAI__ENGINES__MOCK4__FORMAT", &typo)
        .env("IMAGAI__ENGINES__MOCK4__API_KEY", "test-key")
        .env("IMAGAI__ENGINES__MOCK4__MODEL", "dall-e-3")
        .arg("list-engines")
        .assert()
        .success()
        // Clipped to the 12-wide column, with the next column still starting
        // where the header says it does.
        .stdout(predicate::str::contains("aaaaaaaaaaa… dall-e-3"))
        // D-24: the full value is not lost — the error names the whole typo.
        .stdout(predicate::str::contains(typo.as_str()));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_shipped_env_example_loads_every_engine_it_documents() {
    // CR-01: the example file is a user-facing artifact with one instruction —
    // "Copy this file to .env" — so it is exercised like any other contract. An
    // unparseable line aborts the whole file, which silently drops every engine
    // it documents, including the working OpenAI ones.
    let out = TempDir::new().unwrap();
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join(".env.example");
    std::fs::copy(&example, out.path().join(".env")).expect("copy .env.example");

    Command::cargo_bin("imagai")
        .unwrap()
        // The process reads `.env` from its working directory, so the copy under
        // test is the one it walks up to instead of the repo's own tree.
        .current_dir(out.path())
        .arg("list-engines")
        .assert()
        .success()
        .stdout(predicate::str::contains("openai_dalle3"))
        .stdout(predicate::str::contains("openai_gpt"))
        .stderr(predicate::str::contains("could not parse").not());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_malformed_env_file_is_reported_not_swallowed() {
    // WR-06: a `.env` that fails to parse must name the failure rather than
    // loading nothing in silence, so a broken file can never be mistaken for
    // "the user configured nothing".
    let out = TempDir::new().unwrap();
    std::fs::write(
        out.path().join(".env"),
        "IMAGAI__ENGINES__<NAME>__FORMAT=openai\n\
         IMAGAI__ENGINES__GOOD__API_KEY=test-key\n\
         IMAGAI__ENGINES__GOOD__MODEL=dall-e-3\n",
    )
    .expect("write .env");

    Command::cargo_bin("imagai")
        .unwrap()
        .current_dir(out.path())
        .arg("list-engines")
        .assert()
        .success()
        .stdout(predicate::str::contains("No engines configured"))
        .stderr(predicate::str::contains("could not parse"));
}

#[tokio::test(flavor = "multi_thread")]
async fn openai_default_engine_body_is_unchanged() {
    // Success criterion 3 / D-28's executable form. The expected key set is the
    // pre-phase shape recorded when this phase was planned, so an accidental fal
    // key leaking into the OpenAI-compatible body fails here rather than
    // reaching a provider. A future intentional change must edit this test
    // deliberately.
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();

    cmd(&mock, out.path())
        .args(["generate", "-p", "a red cat"])
        .assert()
        .success();

    let bodies = mock.requests_for("/v1/images/generations");
    assert_eq!(bodies.len(), 1, "exactly one request");
    let body = &bodies[0];
    let obj = body.as_object().expect("body is an object");
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec![
            "model",
            "n",
            "prompt",
            "quality",
            "response_format",
            "size",
            "style"
        ],
        "the pre-phase dall-e-3 key set, in full"
    );
    for fal_key in [
        "num_images",
        "image_size",
        "output_format",
        "images",
        "data",
    ] {
        assert!(
            body.get(fal_key).is_none(),
            "no fal key '{fal_key}' may appear: {body}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn stability_default_engine_body_is_unchanged() {
    // The Stability key set is the regression-sensitive part: `n` and
    // `response_format` are dropped and the three extras plus the injected
    // default `mode` are what remain. `generate_stability_extra_params_sent`
    // asserts the extras survive; this asserts the whole key set.
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
    assert_eq!(bodies.len(), 1, "exactly one request");
    let body = &bodies[0];
    let obj = body.as_object().expect("body is an object");
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec![
            "aspect_ratio",
            "mode",
            "model",
            "negative_prompt",
            "prompt",
            "seed"
        ],
        "the pre-phase Stability key set, in full"
    );
    assert!(body.get("n").is_none(), "n is dropped for Stability");
    assert!(
        body.get("response_format").is_none(),
        "response_format is dropped for Stability"
    );
    assert!(
        body.get("size").is_none(),
        "size is dropped when aspect_ratio is"
    );
    assert_eq!(body["mode"], "text-to-image", "default mode injected");
}

// ---------------------------------------------------------------- fal engine

#[tokio::test(flavor = "multi_thread")]
async fn fal_engine_generates_from_the_cli() {
    // Success criterion 4, CLI surface: `--engine` selects the declared fal
    // engine and the adapter builds its own body. The mock's request count is
    // the proof — "one fal request, zero OpenAI-compatible requests" is
    // observed, not inferred.
    let mock = MockServer::start().await;
    mock.set_fal_mode(common::FalMode::Url { count: 1 });
    let out = TempDir::new().unwrap();

    cmd(&mock, out.path())
        .env("IMAGAI__ENGINES__FAL__API_KEY", "test-key")
        .env("IMAGAI__ENGINES__FAL__BASE_URL", mock.base_url())
        .env("IMAGAI__ENGINES__FAL__MODEL", "fal-ai/flux/schnell")
        .env("IMAGAI__ENGINES__FAL__FORMAT", "fal")
        .args(["generate", "-p", "a fal cat", "--engine", "fal"])
        .assert()
        .success()
        .stdout(predicate::str::contains("generated successfully"))
        .stdout(predicate::str::contains("Saved to:"));

    assert_eq!(list_pngs(out.path()).len(), 1, "one saved PNG");
    assert_eq!(
        mock.requests_for("fal-ai/flux/schnell").len(),
        1,
        "one fal request"
    );
    assert_eq!(mock.requests_for("/v1/images/generations").len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn unknown_format_fails_the_cli_run() {
    // D-24/D-05's fail-fast reaching the operator through the real binary: an
    // unrecognised FORMAT must not spend a request on a guess. The operator sees
    // the engine, the rejected value and both supported names on stderr, and the
    // load-time `[warning]` says the same thing earlier.
    //
    // DEVIATION from 03-05 Task 2: the plan also asks for a non-zero exit. It is
    // not asserted here because `cmd_generate` returns `Ok(())` for per-item
    // errors today, so the process exits 0 — that is Phase 4's success criterion
    // ("Non-zero exit on failure", ROADMAP § Phase 4) and 03-05's own acceptance
    // criteria forbid touching `src/`. Recorded as an open gap, not silently
    // satisfied; see 03-05-SUMMARY.md.
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();

    cmd(&mock, out.path())
        .env("IMAGAI__ENGINES__BAD__API_KEY", "test-key")
        .env("IMAGAI__ENGINES__BAD__BASE_URL", mock.base_url())
        .env("IMAGAI__ENGINES__BAD__MODEL", "gpt-image-1")
        .env("IMAGAI__ENGINES__BAD__FORMAT", "bogus")
        .args(["generate", "-p", "x", "--engine", "bad"])
        .assert()
        .stderr(predicate::str::contains("bad"))
        .stderr(predicate::str::contains("bogus"))
        .stderr(predicate::str::contains("openai"))
        .stderr(predicate::str::contains("fal"));

    assert!(
        mock.requests().is_empty(),
        "no request is spent on an unknown format: {:?}",
        mock.requests()
    );
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
async fn edit_without_image_uses_most_recent_output() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let out_path = out.path().to_path_buf();
    seed_source_png(out.path(), "older.png");
    seed_source_png(out.path(), "newer.png");

    // D-03: mtime decides which image is "most recent". Force the two files
    // apart deterministically instead of sleeping — a sleep-based fixture is
    // both slow and flaky on a coarse filesystem tick.
    let now = std::time::SystemTime::now();
    for (name, offset) in [("older.png", -60i64), ("newer.png", 60i64)] {
        std::fs::File::options()
            .write(true)
            .open(out.path().join(name))
            .unwrap()
            .set_modified(now + std::time::Duration::from_secs(offset.unsigned_abs()))
            .unwrap();
    }

    cmd_edit(&mock, out.path())
        .args(["edit", "-p", "make it sunset"])
        .assert()
        .success()
        // The pick must be visible: a silent pick edits a file the user never
        // named (T-01-07).
        .stdout(predicate::str::contains("Using most recent image"))
        .stdout(predicate::str::contains("newer.png"));

    // D-06: the saved name derives from the auto-picked source.
    assert!(
        out_path.join("newer-edit.png").exists(),
        "newer-edit.png written"
    );

    let edits = mock
        .requests()
        .into_iter()
        .filter(|(p, _)| p.contains("images/edits"))
        .count();
    assert_eq!(edits, 1, "exactly one edits request");
    let raw = mock.raw_requests_for("images/edits");
    assert_eq!(raw.len(), 1);
    assert!(
        raw[0].windows(7).any(|w| w == b"image[]"),
        "multipart body carries an image[] part name"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn edit_without_image_fails_when_output_dir_is_empty() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();

    cmd_edit(&mock, out.path())
        .args(["edit", "-p", "make it sunset"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains(
            "No images found in output directory",
        ))
        .stderr(predicate::str::contains("--image"));
}

#[tokio::test(flavor = "multi_thread")]
async fn edit_explicit_image_overrides_the_newest() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let out_path = out.path().to_path_buf();
    let older = seed_source_png(out.path(), "older.png");
    seed_source_png(out.path(), "newer.png");

    // `newer.png` holds the later mtime, but the explicit path must win.
    let now = std::time::SystemTime::now();
    std::fs::File::options()
        .write(true)
        .open(out.path().join("older.png"))
        .unwrap()
        .set_modified(now - std::time::Duration::from_secs(60))
        .unwrap();
    std::fs::File::options()
        .write(true)
        .open(out.path().join("newer.png"))
        .unwrap()
        .set_modified(now + std::time::Duration::from_secs(60))
        .unwrap();

    cmd_edit(&mock, out.path())
        .args(["edit", "--image"])
        .arg(&older)
        .args(["-p", "make it sunset"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Using most recent image").not());

    assert!(
        out_path.join("older-edit.png").exists(),
        "older-edit.png written"
    );
    assert!(
        !out_path.join("newer-edit.png").exists(),
        "newer-edit.png must not be written"
    );
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

#[tokio::test(flavor = "multi_thread")]
async fn edit_sends_multiple_reference_images_in_order() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let source = seed_source_png(out.path(), "source.png");
    let ref_a = seed_source_png(out.path(), "ref-a.png");
    let ref_b = seed_source_png(out.path(), "ref-b.png");

    cmd_edit(&mock, out.path())
        .args(["edit", "--image"])
        .arg(&source)
        .arg("--ref")
        .arg(&ref_a)
        .arg("--ref")
        .arg(&ref_b)
        .args(["-p", "blend them"])
        .assert()
        .success();

    let bodies = mock.raw_requests_for("images/edits");
    assert_eq!(bodies.len(), 1);
    let body = &bodies[0];

    // The multipart body is what makes ordering observable — the JSON view is
    // empty for a multipart request.
    let offsets: Vec<usize> = (0..body.len())
        .filter(|i| body[*i..].starts_with(b"image[]"))
        .collect();
    assert_eq!(offsets.len(), 3, "one image[] part per image");
    assert!(
        offsets[0] < offsets[1] && offsets[1] < offsets[2],
        "source first, then refs in the given order: {offsets:?}"
    );

    // Source filename precedes both reference filenames in the wire order.
    let pos = |needle: &[u8]| (0..body.len()).find(|i| body[*i..].starts_with(needle));
    let (s, a, b) = (
        pos(b"source.png").expect("source filename"),
        pos(b"ref-a.png").expect("ref-a filename"),
        pos(b"ref-b.png").expect("ref-b filename"),
    );
    assert!(s < a && a < b, "source, ref-a, ref-b in order");
}

#[tokio::test(flavor = "multi_thread")]
async fn edit_rejects_more_than_sixteen_images() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let source = seed_source_png(out.path(), "source.png");

    let mut c = cmd_edit(&mock, out.path());
    c.arg("edit")
        .arg("--image")
        .arg(&source)
        .arg("-p")
        .arg("too many");
    for _ in 0..16 {
        c.arg("--ref").arg(&source);
    }
    c.assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("17"))
        .stderr(predicate::str::contains("16"));

    assert!(
        mock.requests().is_empty(),
        "an over-limit request must not be sent"
    );
}

// ---------------------------------------------------------------- history

/// Seed a `.imagai-history.json` sidecar directly, so the tree shape is
/// deterministic: fixed ids, filenames, prompts and timestamps. No PNGs are
/// written, so every node reads as `missing` (D-20) — the case these tests
/// assert.
fn seed_history_store(dir: &Path, records: &[serde_json::Value]) {
    let json = serde_json::to_string(records).expect("serialize store");
    std::fs::write(dir.join(".imagai-history.json"), json).expect("seed store");
}

/// One tree row, built with the documented grammar: three spaces of indent per
/// ancestor level, then the connector, then `✗ ` for a missing file, then the
/// id8, two spaces, the fixed 32-character prompt field, two spaces, the
/// filename, two spaces and `YYYY-MM-DD HH:MM`.
fn history_row(
    id: &str,
    prompt: &str,
    filename: &str,
    time: &str,
    depth: usize,
    connector: Option<&str>,
    missing: bool,
) -> String {
    let field = if prompt.chars().count() > 32 {
        format!("{}...", prompt.chars().take(29).collect::<String>())
    } else {
        format!("{:<32}", prompt)
    };
    let indent = "   ".repeat(depth);
    let connector = connector.unwrap_or("");
    let marker = if missing { "✗ " } else { "" };
    format!("{indent}{connector}{marker}{id}  {field}  {filename}  {time}")
}

#[tokio::test(flavor = "multi_thread")]
async fn history_tree_prints_connectors() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let out_path = out.path().to_path_buf();
    seed_history_store(
        out.path(),
        &[
            serde_json::json!({
                "id": "1a2b3c4d", "parent": null, "filename": "cat.png",
                "prompt": "a cat wearing a hat", "model": "dall-e-3",
                "created": "2026-10-09T11:40:00"
            }),
            serde_json::json!({
                "id": "2b3c4d5e", "parent": "1a2b3c4d", "filename": "cat-edit.png",
                "prompt": "make it sunset over the mountains with a warm glow",
                "model": "gpt-image-1", "created": "2026-10-09T11:41:00"
            }),
            serde_json::json!({
                "id": "3c4d5e6f", "parent": "1a2b3c4d", "filename": "cat-edit-1.png",
                "prompt": "add a party hat", "model": "gpt-image-1",
                "created": "2026-10-09T11:42:00"
            }),
        ],
    );

    cmd(&mock, out.path())
        .arg("history")
        .assert()
        .success()
        // One session: the session clause is omitted entirely.
        .stdout(predicate::str::contains(format!(
            "🌳 Iteration history — {} (3 nodes)",
            out_path.to_string_lossy()
        )))
        .stdout(predicate::str::contains(history_row(
            "1a2b3c4d",
            "a cat wearing a hat",
            "cat.png",
            "2026-10-09 11:40",
            0,
            None,
            true,
        )))
        // Non-last child: three spaces of indent plus the `+- ` connector.
        .stdout(predicate::str::contains(history_row(
            "2b3c4d5e",
            "make it sunset over the mountains with a warm glow",
            "cat-edit.png",
            "2026-10-09 11:41",
            1,
            Some("+- "),
            true,
        )))
        // Last child: six spaces of indent (two ancestor levels) plus `` `- ``.
        .stdout(predicate::str::contains(history_row(
            "3c4d5e6f",
            "add a party hat",
            "cat-edit-1.png",
            "2026-10-09 11:42",
            1,
            Some("`- "),
            true,
        )));

    // Three nodes are missing, so the plural warning line prints.
    let assert = cmd(&mock, out.path()).arg("history").assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf-8 stdout");
    assert!(
        stdout.contains(
            "[warning] 3 nodes point at files that are no longer in the output directory."
        ),
        "the plural warning line: {stdout}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn history_tree_marks_missing_nodes() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    seed_history_store(
        out.path(),
        &[serde_json::json!({
            "id": "1a2b3c4d", "parent": null, "filename": "gone.png",
            "prompt": "rainy street at night", "model": "dall-e-3",
            "created": "2026-10-09T11:30:00"
        })],
    );

    cmd(&mock, out.path())
        .arg("history")
        .assert()
        .success()
        // `✗ ` sits immediately after the (absent) connector, before the id8.
        .stdout(predicate::str::contains(history_row(
            "1a2b3c4d",
            "rainy street at night",
            "gone.png",
            "2026-10-09 11:30",
            0,
            None,
            true,
        )))
        .stdout(predicate::str::contains(
            "[warning] 1 node points at a file that is no longer in the output directory.",
        ));

    // Plural wording: two missing nodes.
    seed_history_store(
        out.path(),
        &[
            serde_json::json!({
                "id": "1a2b3c4d", "parent": null, "filename": "gone.png",
                "prompt": "one", "model": "dall-e-3", "created": "2026-10-09T11:30:00"
            }),
            serde_json::json!({
                "id": "2b3c4d5e", "parent": null, "filename": "also-gone.png",
                "prompt": "two", "model": "dall-e-3", "created": "2026-10-09T11:31:00"
            }),
        ],
    );
    cmd(&mock, out.path())
        .arg("history")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "[warning] 2 nodes point at files that are no longer in the output directory.",
        ));
}

#[tokio::test(flavor = "multi_thread")]
async fn history_empty_state() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();

    cmd(&mock, out.path())
        .arg("history")
        .assert()
        .success()
        .stdout(predicate::eq(
            "🌳 No history yet — generate or edit an image first.\n",
        ))
        .stdout(predicate::str::contains("Iteration history —").not())
        .stdout(predicate::str::contains("[warning]").not());
}

#[tokio::test(flavor = "multi_thread")]
async fn history_reports_a_non_utf8_sidecar_instead_of_an_empty_tree() {
    // CR-02: `read_records` mapped every `read_to_string` failure to an empty
    // store, so a non-UTF-8 sidecar read as "no history" and the next write
    // destroyed it. `Could not read history at` must reach the operator.
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    std::fs::write(out.path().join(".imagai-history.json"), b"\xff\xfe\x00{\"")
        .expect("write non-utf8 sidecar");

    cmd(&mock, out.path())
        .arg("history")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("Could not read history at"))
        .stderr(predicate::str::contains(".imagai-history.json"))
        .stdout(predicate::str::contains("No history yet").not());
}

// ---------------------------------------------------------------- fork

/// Seed a root node and the matching PNG, returning the root's id.
fn seed_root_with_file(dir: &Path, filename: &str) -> String {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(common::PIXEL_PNG_B64)
        .expect("decode PIXEL_PNG_B64");
    std::fs::write(dir.join(filename), bytes).expect("seed png");
    let id = "1a2b3c4d".to_string();
    seed_history_store(
        dir,
        &[serde_json::json!({
            "id": id, "parent": null, "filename": filename,
            "prompt": "a cat", "model": "dall-e-3", "created": "2026-10-09T11:40:00"
        })],
    );
    id
}

fn read_history_record_ids(dir: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(dir.join(".imagai-history.json")).expect("sidecar");
    let value: serde_json::Value = serde_json::from_str(&text).expect("sidecar parses");
    value
        .as_array()
        .expect("array")
        .iter()
        .map(|r| r["id"].as_str().expect("id").to_string())
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn fork_from_node_id_records_child() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let root_id = seed_root_with_file(out.path(), "cat.png");

    cmd_edit(&mock, out.path())
        .args(["fork", "--from", &root_id, "-p", "make it sunset"])
        .assert()
        .success()
        .stdout(predicate::str::contains("🔀 Forking from"))
        .stdout(predicate::str::contains("cat.png"));

    // One edits request, zero generations: the shared core took the edit path.
    assert_eq!(mock.requests_for("/v1/images/edits").len(), 1);
    assert_eq!(mock.requests_for("/v1/images/generations").len(), 0);

    let text = std::fs::read_to_string(out.path().join(".imagai-history.json")).expect("sidecar");
    let value: serde_json::Value = serde_json::from_str(&text).expect("parses");
    let records = value.as_array().expect("array");
    assert_eq!(records.len(), 2, "root plus fork child");
    assert_eq!(records[1]["parent"], records[0]["id"], "child of the root");
    assert!(
        records[1]["filename"]
            .as_str()
            .expect("filename")
            .ends_with("-edit.png"),
        "D-06 naming scheme"
    );
    // The original record is untouched.
    assert_eq!(records[0]["id"], root_id.as_str());
    assert!(out.path().join("cat-edit.png").is_file());
}

#[tokio::test(flavor = "multi_thread")]
async fn fork_from_filename_of_adopted_file() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    // No history store at all: the file is adopted at fork time (D-12).
    let source = seed_source_png(out.path(), "loose.png");
    let before = std::fs::read(&source).expect("source bytes before");

    cmd_edit(&mock, out.path())
        .args(["fork", "--from", "loose.png", "-p", "make it sunset"])
        .assert()
        .success();

    let text = std::fs::read_to_string(out.path().join(".imagai-history.json")).expect("sidecar");
    let value: serde_json::Value = serde_json::from_str(&text).expect("parses");
    let records = value.as_array().expect("array");
    assert_eq!(records.len(), 2, "adopted root plus the child");
    assert_eq!(records[0]["filename"], "loose.png");
    assert!(records[0]["parent"].is_null(), "adopted as a root");
    assert_eq!(records[1]["parent"], records[0]["id"]);

    // The fork references the same file rather than copying it (D-13).
    assert_eq!(
        std::fs::read(&source).expect("source bytes after"),
        before,
        "the source file is untouched"
    );
    let child = out.path().join("loose-edit.png");
    assert!(child.is_file(), "the fork's own result");
    assert_ne!(child, source);
}

#[tokio::test(flavor = "multi_thread")]
async fn fork_from_a_file_that_lives_only_in_the_cwd_is_refused() {
    // CR-01: the resolver's file step resolved a bare name against the process
    // working directory before looking inside `output_dir`, so a CWD-resident
    // file was read and uploaded. The web surface binds 0.0.0.0 with
    // `allow_origin(Any)`, which made that a trust-boundary break.
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    let cwd = TempDir::new().unwrap();
    seed_source_png(cwd.path(), "probe_cwd_only.png");

    cmd_edit(&mock, out.path())
        .current_dir(cwd.path())
        .args(["fork", "--from", "probe_cwd_only.png", "-p", "x"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains(
            "No history node or image named 'probe_cwd_only.png' in",
        ));

    // Nothing was read, nothing was sent, and nothing was adopted.
    assert!(
        mock.requests().is_empty(),
        "no provider call for a CWD-only file"
    );
    assert!(
        !out.path().join(".imagai-history.json").exists(),
        "a CWD-only file is never adopted as a root"
    );
    assert!(
        !out.path().join("probe_cwd_only.png").exists(),
        "no file was created inside output_dir"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn fork_from_a_bare_file_when_cwd_is_output_dir_adopts_it() {
    // CR-03: when the CWD *is* `output_dir`, the old step (3) compared the
    // literal's parent (`Some("")`) against `Some(dir)`, so the adoption branch
    // was unreachable — the child recorded as a root and its lineage was
    // silently dropped.
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    seed_source_png(out.path(), "cat.png");

    cmd_edit(&mock, out.path())
        .current_dir(out.path())
        .args(["fork", "--from", "cat.png", "-p", "make it sunset"])
        .assert()
        .success()
        .stdout(predicate::str::contains("🔀 Forking from cat.png"));

    let text = std::fs::read_to_string(out.path().join(".imagai-history.json")).expect("sidecar");
    let value: serde_json::Value = serde_json::from_str(&text).expect("parses");
    let records = value.as_array().expect("array");
    assert_eq!(records.len(), 2, "adopted root plus the fork child");
    assert_eq!(records[0]["filename"], "cat.png");
    assert!(records[0]["parent"].is_null(), "adopted as a root");
    assert_eq!(
        records[1]["parent"], records[0]["id"],
        "CR-03: the child keeps its parent instead of recording as a root"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn fork_keeps_both_branches_visible() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    seed_history_store(
        out.path(),
        &[
            serde_json::json!({
                "id": "1a2b3c4d", "parent": null, "filename": "cat.png",
                "prompt": "a cat", "model": "dall-e-3", "created": "2026-10-09T11:40:00"
            }),
            serde_json::json!({
                "id": "2b3c4d5e", "parent": "1a2b3c4d", "filename": "cat-edit.png",
                "prompt": "make it sunset", "model": "gpt-image-1",
                "created": "2026-10-09T11:41:00"
            }),
        ],
    );
    seed_source_png(out.path(), "cat.png");

    cmd_edit(&mock, out.path())
        .args(["fork", "--from", "1a2b3c4d", "-p", "make it night"])
        .assert()
        .success();

    // BRCH-02: the original branch and the new child are both listed, and the
    // original parent node still appears exactly once.
    let assert = cmd(&mock, out.path()).arg("history").assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf-8 stdout");
    assert!(
        stdout.contains("cat-edit.png"),
        "the original child is still listed: {stdout}"
    );
    // The permissive `||` alternative is dropped on purpose: the seeded store
    // already claims `cat-edit.png` through node `2b3c4d5e`, so the record half
    // of the probe forces the fork to bump and the name is deterministic —
    // `cat-edit_1.png`, the `_1` the shared `suffix_numbered` scheme produces.
    // A filename collision must never pass here silently again (G-1).
    assert!(
        stdout.contains("cat-edit_1.png"),
        "the fork's child is listed: {stdout}"
    );
    assert_eq!(
        stdout.matches("1a2b3c4d").count(),
        1,
        "the root appears exactly once: {stdout}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn two_forks_from_one_source_keep_both_images() {
    // G-1: a second fork from the same source used to derive the first fork's
    // `{stem}-edit.<ext>` name and overwrite its image, leaving two records
    // naming one file while the tree still looked perfectly healthy.
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    seed_root_with_file(out.path(), "cat.png");

    cmd_edit(&mock, out.path())
        .args(["fork", "--from", "1a2b3c4d", "-p", "make it night"])
        .assert()
        .success();
    cmd_edit(&mock, out.path())
        .args(["fork", "--from", "1a2b3c4d", "-p", "make it forest"])
        .assert()
        .success();

    // Both forks survive as distinct files: the second bumped because the disk
    // half of the probe claims the canonical name — the overwrite G-1 committed.
    assert!(
        out.path().join("cat-edit.png").is_file(),
        "the first fork's image survives"
    );
    assert!(
        out.path().join("cat-edit_1.png").is_file(),
        "the second fork bumps to a distinct name"
    );

    // The store stays truthful: three records, three distinct filenames, both
    // children parented to the root — the condition G-1 created, inverted.
    let text = std::fs::read_to_string(out.path().join(".imagai-history.json")).expect("sidecar");
    let value: serde_json::Value = serde_json::from_str(&text).expect("parses");
    let records = value.as_array().expect("array");
    assert_eq!(records.len(), 3, "root plus two fork children");
    let filenames: Vec<&str> = records
        .iter()
        .map(|r| r["filename"].as_str().expect("filename"))
        .collect();
    assert_eq!(
        filenames,
        vec!["cat.png", "cat-edit.png", "cat-edit_1.png"],
        "three distinct filenames"
    );
    assert_eq!(records[1]["prompt"], "make it night");
    assert_eq!(records[2]["prompt"], "make it forest");
    assert_eq!(records[1]["parent"], records[0]["id"], "child of the root");
    assert_eq!(records[2]["parent"], records[0]["id"], "child of the root");

    // The tree lists both children, the root exactly once, and no missing-file
    // marker: D-20's marker would return if either fork had silently replaced
    // the other's file.
    let assert = cmd(&mock, out.path()).arg("history").assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf-8 stdout");
    assert!(
        stdout.contains("cat-edit.png"),
        "the first fork is listed: {stdout}"
    );
    assert!(
        stdout.contains("cat-edit_1.png"),
        "the second fork is listed: {stdout}"
    );
    assert_eq!(
        stdout.matches("1a2b3c4d").count(),
        1,
        "the root appears exactly once: {stdout}"
    );
    assert!(
        !stdout.contains("✗") && !stdout.contains("no longer in"),
        "both images are really on disk: {stdout}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn fork_rejects_unknown_source() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();

    cmd_edit(&mock, out.path())
        .args(["fork", "--from", "nope.png", "-p", "x"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains(
            "No history node or image named 'nope.png' in",
        ))
        .stderr(predicate::str::contains(
            "run 'imagai history' to see what is available.",
        ));
}

#[tokio::test(flavor = "multi_thread")]
async fn fork_rejects_deleted_file_of_recorded_node() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    // A recorded node whose file was deleted externally (D-20 keeps the node,
    // but a fork cannot start from it).
    seed_history_store(
        out.path(),
        &[serde_json::json!({
            "id": "1a2b3c4d", "parent": null, "filename": "gone.png",
            "prompt": "a cat", "model": "dall-e-3", "created": "2026-10-09T11:40:00"
        })],
    );

    cmd_edit(&mock, out.path())
        .args(["fork", "--from", "1a2b3c4d", "-p", "x"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("which is no longer in"));
}

/// A sidecar record whose `filename` carries a `../` component must never be
/// joined raw under `output_dir` (T-02-01, T-02-05): the CLI reduces it through
/// `history::node_path` and refuses a record that is not a plain filename — with
/// no request reaching the provider and nothing recorded.
#[tokio::test(flavor = "multi_thread")]
async fn fork_rejects_a_record_filename_that_escapes_output_dir() {
    let mock = MockServer::start().await;
    let parent = TempDir::new().unwrap();
    let out = parent.path().join("output_dir");
    std::fs::create_dir(&out).unwrap();
    // Two files with the same final component: the traversal target one level
    // above `output_dir`, and the in-dir file whose presence lets the record
    // resolve at all. Before the guard, the unreduced join read the first.
    std::fs::write(
        parent.path().join("escape-target.png"),
        b"OUTSIDE-OUTPUT_DIR",
    )
    .expect("seed traversal target");
    seed_source_png(&out, "escape-target.png");
    seed_history_store(
        &out,
        &[serde_json::json!({
            "id": "1a2b3c4d", "parent": null, "filename": "../escape-target.png",
            "prompt": "a cat", "model": "dall-e-3", "created": "2026-10-09T11:40:00"
        })],
    );

    cmd_edit(&mock, &out)
        .args(["fork", "--from", "1a2b3c4d", "-p", "x"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains(
            "'../escape-target.png' is not a plain filename inside",
        ));

    assert!(
        mock.requests_for("/v1/images/edits").is_empty(),
        "no edit request reaches the provider: {:?}",
        mock.requests_for("/v1/images/edits")
    );
    assert_eq!(
        read_history_record_ids(&out),
        vec!["1a2b3c4d".to_string()],
        "the refused fork records nothing"
    );
    assert!(
        parent.path().join("escape-target.png").is_file(),
        "the traversal target is untouched"
    );
}

/// The same guard on the `edit --from` path, for the separator and absolute-path
/// shapes a hand-edited sidecar can carry. Each is refused, none reaches the
/// provider. `(record filename, file that must exist inside output_dir for the
/// node to resolve at all)`.
#[tokio::test(flavor = "multi_thread")]
async fn edit_from_refuses_a_separator_carrying_record_filename() {
    for (filename, present) in [
        ("../escape-target.png", "escape-target.png"),
        ("sub/dir/cat.png", "cat.png"),
        ("/etc/passwd", "passwd"),
    ] {
        let mock = MockServer::start().await;
        let parent = TempDir::new().unwrap();
        let out = parent.path().join("output_dir");
        std::fs::create_dir(&out).unwrap();
        seed_source_png(&out, present);
        seed_history_store(
            &out,
            &[serde_json::json!({
                "id": "1a2b3c4d", "parent": null, "filename": filename,
                "prompt": "a cat", "model": "dall-e-3", "created": "2026-10-09T11:40:00"
            })],
        );

        cmd_edit(&mock, &out)
            .args(["edit", "--from", "1a2b3c4d", "-p", "x"])
            .assert()
            .failure()
            .code(1)
            .stderr(predicate::str::contains("is not a plain filename inside"));

        assert!(
            mock.requests_for("/v1/images/edits").is_empty(),
            "{filename:?}: no edit request reaches the provider"
        );
    }
}

/// A record whose filename has no final component at all (`..`, ``, `.`) has no
/// path inside `output_dir` to fork from, so resolution refuses the node before
/// any read — and nothing reaches the provider.
#[tokio::test(flavor = "multi_thread")]
async fn fork_refuses_a_record_filename_with_no_final_component() {
    for filename in ["..", "", "."] {
        let mock = MockServer::start().await;
        let out = TempDir::new().unwrap();
        seed_history_store(
            out.path(),
            &[serde_json::json!({
                "id": "1a2b3c4d", "parent": null, "filename": filename,
                "prompt": "a cat", "model": "dall-e-3", "created": "2026-10-09T11:40:00"
            })],
        );

        cmd_edit(&mock, out.path())
            .args(["fork", "--from", "1a2b3c4d", "-p", "x"])
            .assert()
            .failure()
            .code(1)
            .stderr(predicate::str::contains("no longer in"));

        assert!(
            mock.requests_for("/v1/images/edits").is_empty(),
            "{filename:?}: no edit request reaches the provider"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn fork_rejects_more_than_sixteen_images() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    seed_source_png(out.path(), "cat.png");

    let mut c = cmd_edit(&mock, out.path());
    c.arg("fork")
        .arg("--from")
        .arg("cat.png")
        .arg("-p")
        .arg("too many");
    for _ in 0..16 {
        c.arg("--ref").arg(out.path().join("cat.png"));
    }
    c.assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("17"))
        .stderr(predicate::str::contains("16"));
}

#[tokio::test(flavor = "multi_thread")]
async fn edit_from_resolves_a_past_node_without_an_image_flag() {
    let mock = MockServer::start().await;
    let out = TempDir::new().unwrap();
    seed_root_with_file(out.path(), "cat.png");

    cmd_edit(&mock, out.path())
        .args(["edit", "--from", "1a2b3c4d", "-p", "make it sunset"])
        .assert()
        .success()
        .stdout(predicate::str::contains("🔀 Forking from cat.png"));

    assert_eq!(mock.requests_for("/v1/images/edits").len(), 1);
    assert!(out.path().join("cat-edit.png").is_file());
    assert_eq!(read_history_record_ids(out.path()).len(), 2);
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
