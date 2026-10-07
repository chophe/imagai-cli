# Phase 1: Image Editing - Pattern Map

**Mapped:** 2026-10-06
**Files analyzed:** 13 (0 new files — Phase 1 extends existing flat modules per STRUCTURE.md)
**Analogs found:** 13 / 13

All analog paths verified git-tracked (`git ls-files` non-empty for every path named below).

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `Cargo.toml` | config | — | `Cargo.toml:17` (same line, feature flag) | exact |
| `src/cli.rs` | controller (CLI) | request-response | `src/cli.rs` `Commands`/`GenerateArgs`/`cmd_generate` | exact |
| `src/models.rs` | model (DTO) | transform | `src/models.rs` `ImageGenerationRequest` | exact |
| `src/provider.rs` | service (transport) | HTTP request-response | `src/provider.rs` `generate_images` + `openrouter_chat_generate` | exact |
| `src/core.rs` | service (pipeline) | batch + file-I/O | `src/core.rs` `generate_image_core`, `suffix_numbered` | exact |
| `src/utils.rs` | utility | file-I/O | `src/utils.rs` `inject_png_metadata` + `src/web.rs` `list_images` (logic to move) | exact |
| `src/web.rs` | controller (HTTP) | request-response | `src/web.rs` `GeneratePayload`/`generate`/`serve_image` | exact |
| `src/tui.rs` | component (TUI) | request-response (async) | `src/tui.rs` Generate tab (rows/draw/handle_key) | exact |
| `web_interface.html` | component (browser SPA) | request-response (fetch) | `web_interface.html:624-660` `executeGeneration` | exact |
| `tests/common/mod.rs` | test (mock server) | event-driven (route dispatch) | `tests/common/mod.rs` `dispatch`/`ImagesMode` | exact |
| `tests/cli.rs` | test | e2e (subprocess) | `tests/cli.rs` `cmd()` + generate tests | exact |
| `tests/core.rs` | test | e2e (in-process) | `tests/core.rs` `unknown_engine_returns_error_response` | exact |
| `tests/web.rs` | test | e2e (tower oneshot) | `tests/web.rs` `generate_endpoint_returns_image_preview` + `serve_image_blocks_traversal` | exact |

## Pattern Assignments

### `src/cli.rs` (controller, request-response) — add `Commands::Edit`, `EditArgs`, `cmd_edit`

**Analog:** same file — `Commands` enum, `GenerateArgs`, `cmd_generate`

**Subcommand wiring** (lines 22-36, 123-130):
```rust
#[derive(Subcommand)]
pub enum Commands {
    /// Generate images from a text prompt
    Generate(Box<GenerateArgs>),
    // … add: /// Edit an existing image with a change instruction
    //        Edit(Box<EditArgs>),
    /// List configured engines (optionally fetch available models)
    ListEngines { #[arg(long)] all: bool },
    /// Launch the interactive terminal UI
    Tui,
    /// Start the web interface (REST API + static UI)
    Web(WebArgs),
}

pub async fn run(cli: Cli, settings: &Settings) -> anyhow::Result<()> {
    match cli.command {
        Commands::Generate(args) => cmd_generate(&args, settings).await,
        // add: Commands::Edit(args) => cmd_edit(&args, settings).await,
        …
    }
}
```

**Args derive pattern** (lines 38-64): `#[derive(Args, Clone)]` + doc comments become help text. `--ref` needs `#[arg(long = "ref")] pub refs: Vec<String>` (clap `append` is Vec default).

**Engine resolution + fail-fast** (lines 134-145) — copy for `cmd_edit` (D-05 bail happens in provider, but engine resolution is identical):
```rust
let selected_engine = args.engine.clone().or_else(|| settings.default_engine.clone());
let Some(selected_engine) = selected_engine else {
    eprintln!("{} No engine specified and no default engine configured. …",
        "[Error]".bold().red());
    print_available_engines(settings);
    std::process::exit(1);
};
```

**Result printing loop** (lines 235-273) — reuse verbatim in `cmd_edit`; post-request provider errors print `[Error]` and return `Ok(())` (Pitfall 8: keep this split, do NOT "fix" exit codes in Phase 1).

**Anti-pattern from research:** do NOT copy the `extra_params` assembly block (lines 194-215) — that duplication across CLI/web/TUI is the documented ARCHITECTURE.md anti-pattern. Edit fields go on `ImageGenerationRequest` as typed fields.

---

### `src/models.rs` (model, transform) — add `source_image` / `ref_images` to `ImageGenerationRequest`

**Analog:** same file — `ImageGenerationRequest` (lines 6-40)

```rust
#[derive(Debug, Clone)]
pub struct ImageGenerationRequest {
    pub prompt: String,
    pub engine: String,
    …
    pub random_filename: bool,
    // ADD (Option/Vec keep existing struct literals compiling via ..Default::default()):
    // pub source_image: Option<String>,
    // pub ref_images: Vec<String>,
}
```

Note: several call sites build this struct **without** `..Default::default()` (e.g. `src/cli.rs:217-230`, `src/web.rs:181-194`, `src/tui.rs:302-322`) — those literals must gain the new fields; test literals using `..Default::default()` (`tests/core.rs:27-31`) compile untouched.

**Unit-test pattern** (lines 124-194): `#[cfg(test)] mod tests` in the same file; `request_defaults` (185-193) is the test to extend.

---

### `src/provider.rs` (service, HTTP request-response) — add `generate_edits()`, `edit_transport()`, widen chat parts

**Analog:** same file — `generate_images` (120-234) and `openrouter_chat_generate` (293-407)

**Capability/dispatch gate precedent** (lines 128-143) — `edit_transport` (research Pattern 4) follows this shape:
```rust
let is_openrouter = config
    .base_url
    .as_deref()
    .map(|u| u.contains("openrouter.ai"))
    .unwrap_or(false);
// edit_transport: same substring inspection + anyhow::bail! naming engine+model
```

**HTTP call + error convention** (lines 154-199) — copy for `generate_edits` (swap `.json(&body)` for `.multipart(form)`):
```rust
let url = resolve_url(&config.base_url, "images/generations"); // → "images/edits"
…
let resp = match client.post(&url).bearer_auth(&config.api_key).json(&body).send().await { … };
let status = resp.status();
let text = resp.text().await?;
if !status.is_success() {
    let msg = extract_api_error(&text)
        .unwrap_or_else(|| format!("HTTP {status}: {}", truncate(&text, 400)));
    return vec![ImageGenerationResponse { error: Some(msg), ..Default::default() }];
}
let parsed: ImagesResponse = match serde_json::from_str(&text) { … };  // reuse DTO
```

**Single-image content part to widen (lines 310-317)** — EDIT-03 needs this to become a list (source first, then refs; text part already first):
```rust
let mut content_items: Vec<Value> = vec![json!({"type": "text", "text": request.prompt})];
if let Some(img_url) = request.extra_params.get("image_url").and_then(|v| v.as_str()) {
    content_items.push(json!({"type": "image_url", "image_url": {"url": img_url}}));
}
```

**Unit-test pattern** (lines 453-564): `request_with_extra` + `dalle_config()` helpers + `build_images_body` assertions — mirror for `edit_transport` tests (gpt-image → Ok, dall-e-3 → Err, openrouter → Ok, stability → Err).

**Body-building caution** (Pitfall 3): `build_images_body` (243-291) shows the JSON body style; the **edit** form must be built from scratch — only `model`, `prompt`, `image[]`. Never forward `response_format`/`quality`/`style` from `ImageGenerationRequest::default()` (`src/models.rs:29-33`).

---

### `src/core.rs` (service, batch + file-I/O) — edit filename derivation + lineage

**Analog:** same file — `generate_image_core` (12-119) and `suffix_numbered` (121-130)

**Filename derivation for D-06** (extends the `output_filename` branch at 45-57):
```rust
fn suffix_numbered(filename: &str, index: usize, total: u32) -> String {
    if total <= 1 { return filename.to_string(); }
    let path = Path::new(filename);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("image");
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("png");
    format!("{stem}_{}.{ext}", index + 1)
}
// Edit: output_filename = "<source-stem>-edit.<ext>" → n>1 branch yields _1/_2 automatically
```

**Save + metadata plumbing** (lines 87-108) — `save_image_from_url`/`save_image_from_b64` take `prompt`/`model`; for D-07 they (or the request) gain a source-filename argument threaded from `request.source_image`.

**Error convention** (lines 16-24): unconfigured engine → `ImageGenerationResponse { error: Some(...) }`, never a panic.

---

### `src/utils.rs` (utility, file-I/O) — `list_output_images()`, PNG `Source` metadata

**Analog A:** `src/utils.rs` `inject_png_metadata` (234-253) + `png_text_chunk` (256-273) — extend with a third keyword:
```rust
pub fn inject_png_metadata(bytes: &[u8], prompt: &str, model: &str) -> Vec<u8> {
    …
    if ctype == b"IEND" {
        out.extend_from_slice(&png_text_chunk("Prompt", prompt));
        out.extend_from_slice(&png_text_chunk("Model", model));
        // ADD: out.extend_from_slice(&png_text_chunk("Source", source));  (D-07)
        …
    }
}
```
Existing test at `src/utils.rs:375` asserts exactly 2 tEXt chunks — must be updated/parameterized (Pitfall 6: Latin-1/NUL rule; lossy-map or skip).

**Analog B:** `src/web.rs:327-378` `list_images` — the private read_dir loop to extract as the shared `list_output_images(dir) -> Vec<(PathBuf, SystemTime)>` newest-first helper (Pattern 3). Its extension filter is `IMAGE_EXTENSIONS` (`src/web.rs:19`), which moves to `utils.rs` here.

**Filename helpers** (lines 7-48): `sanitize_filename` and `get_image_extension` — use for the `-edit` name; do NOT pass user paths through `truncate` (Pitfall 7).

---

### `src/web.rs` (controller, request-response) — `POST /api/edit` + `EditPayload`

**Analog:** same file — `GeneratePayload` (26-64), route table (88-105), `generate` handler (136-235), `serve_image` guard (380-396)

**Payload pattern** (copy exactly, `#[serde(default)]` on optional fields — CONVENTIONS.md):
```rust
#[derive(Deserialize)]
struct GeneratePayload {
    prompt: String,
    #[serde(default)]
    engine: Option<String>,
    …
    #[serde(default)]
    verbose: bool,
}
```

**Route registration** (lines 96-103):
```rust
Router::new()
    .route("/", get(index))
    .route("/api/generate", post(generate))
    // ADD: .route("/api/edit", post(edit)),
    .route("/api/images", get(list_images))
    .route("/api/images/{filename}", get(serve_image))
```

**Path-traversal guard for `source`/`refs`** (Pitfall 9 / ASVS V5) — copy `serve_image` (384-388):
```rust
// Path traversal protection: only take the final path component.
let Some(name) = Path::new(&filename).file_name().and_then(|n| n.to_str()) else {
    return json_not_found();
};
let path = state.settings.output_dir.join(name);
```

**Engine validation shape** (lines 141-155): `{"success": false, "error": …}` JSON, HTTP 200.

**Result serialization** (lines 198-234): reuse `result_items` loop for `/api/edit` responses (saved_path + image_data preview).

---

### `src/tui.rs` (component, async request-response) — new Edit tab + picker

**Analog:** same file — Generate tab row constants (17-30), `handle_key` (364-440), `draw` tab dispatch (457-473), run-loop Tab interception (934-954)

**Tab registration** (line 30): `const TABS: [&str; 3] = ["Generate", "Engines", "About"];` → add `"Edit"` (decide index position once; `draw` match at 467-471 and the `self.tab == 1` model-fetch check at 947 must stay consistent).

**Row constants** (17-28) — give the Edit tab its **own** constants and tab-scoped `focus`/`editing` (Pitfall 5: the four match blocks `text_field`/`text_field_mut`/`bool_field`/`row_label` end in `unreachable!()` at line 200 — do not extend `ROWS = 11`):
```rust
const ROWS: usize = 11;
const ROW_PROMPT: usize = 0;
…
const ROW_SUBMIT: usize = 10;
```

**Async submit pattern** (`start_generation`, ~241-265): spawn task → `mpsc::UnboundedSender<Vec<ImageGenerationResponse>>` → drain in `drain_messages` (325-360). Edit submit reuses this channel verbatim.

**Key routing** (364-440): `q` quits only when `editing.is_none()`; Tab/BackTab cycle focus inside a tab, but the **run loop** (938-945) intercepts Tab for tab-switching when not editing. A picker overlay must consume arrow keys **before** both (research Pitfall 5).

**Tab draw shape** (502-507):
```rust
fn draw_generate_tab(&mut self, frame: &mut Frame, area: Rect) {
    let layout = Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);
    self.draw_form(frame, layout[0]);
    self.draw_logs(frame, layout[1]);
}
```

---

### `web_interface.html` (component, fetch request-response) — edit section

**Analog:** `web_interface.html:624-660` `executeGeneration` — the fetch + result-render pattern:
```javascript
const response = await fetch('/api/generate', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(requestData)
});
const result = await response.json();
if (result.success) { … render res.image_data as <img> … }
```
Source picker: call the **existing** `GET /api/images` (tested at `tests/web.rs:176-200`) and list `images[].filename` newest-first; the dead upload widget (`initializeImageUpload`, lines 779-858, `uploadedImageData` at 647-649) is optional reuse (A8), not required for MVP.

---

### `tests/common/mod.rs` (test, event-driven dispatch) — Bytes capture + edits mode

**Analog:** same file — `dispatch` (106-134), `ImagesMode` (19-27), `images_logic` (148-175)

**The critical fix (Pitfall 1)** — `body: String` (line 110) rejects binary multipart with 400:
```rust
async fn dispatch(
    State(state): State<MockState>,
    method: Method,
    uri: Uri,
    body: String,   // ← change to axum::body::Bytes (always available, no feature)
) -> Response {
    let path = uri.path().to_string();
    let parsed: Value = serde_json::from_str(&body).unwrap_or_else(|_| json!({}));
    state.captured.lock().unwrap().push((path.clone(), parsed));
    if path.contains("images/generations") { return images_logic(state, &uri).await; }
    // ADD: if path.contains("images/edits") { return images_logic(state, &uri).await; }
    …
}
```
Mode enum extension: add an `EditsMode`-style variant mirroring `ImagesMode`, or route `images/edits` into the existing `images_logic` and assert raw bytes (`body.contains(b"image[]")`).

---

### `tests/cli.rs` (test, subprocess e2e) — edit e2e tests

**Analog:** same file — `cmd()` helper (16-24) + `generate_saves_b64_image_with_metadata` (119-148)

```rust
fn cmd(mock: &MockServer, out_dir: &Path) -> Command {
    let mut c = Command::cargo_bin("imagai").unwrap();
    c.env("IMAGAI__DEFAULT_ENGINE", "mock")
        .env("IMAGAI__ENGINES__MOCK__API_KEY", "test-key")
        .env("IMAGAI__ENGINES__MOCK__BASE_URL", mock.base_url())
        .env("IMAGAI__ENGINES__MOCK__MODEL", "dall-e-3")   // ← Pitfall 4: override to gpt-image-1 for edit tests
        .env("IMAGAI__OUTPUT_DIR", out_dir);
    c
}
```
**Warning sign:** an edit test passing with model `dall-e-3` tests the wrong branch. Override with `.env("IMAGAI__ENGINES__MOCK__MODEL", "gpt-image-1")` (precedent: `tests/cli.rs:244-248` overrides the model for the stability test). Fail-fast bail asserts: `.failure().code(1)` (precedent: 318-330).

**Metadata assertion helper** `png_has_metadata` (37-44) extends to the `Source` keyword.

---

### `tests/core.rs` (test, in-process) — edit pipeline tests

**Analog:** same file — `unknown_engine_returns_error_response` (22-38) and `openrouter_chat_image_model_saves_png` (40-87)

Pattern: build `Settings` inline with `HashMap::from([("mock", EngineConfig { … })])`, construct request with `..Default::default()`, call `generate_image_core`, assert `results[0].saved_path` and `mock.requests_for("/v1/images/edits")`. The openrouter URL trick (`format!("http://{}/openrouter.ai/v1", mock.addr)`, line 49) is how you trigger the chat path against the mock.

---

### `tests/web.rs` (test, tower oneshot) — `/api/edit` tests

**Analog:** same file — `post_json` (53-72), `generate_endpoint_returns_image_preview` (101-129), `serve_image_blocks_traversal` (202-213)

```rust
let payload = json!({"prompt": "a web cat", "n": 1});
let (status, value) = post_json(app, "/api/generate", &payload).await;
assert_eq!(status, StatusCode::OK);
assert_eq!(value["success"], true);
```
Add a traversal test **beside** 202-213 for `source: "../../.env"` → `{"success": false, ...}` error.

---

### `Cargo.toml` (config) — reqwest `multipart` feature

**Analog:** `Cargo.toml:17`
```toml
# before
reqwest = { version = "0.13", default-features = false, features = ["json", "rustls", "webpki-roots"] }
# after
reqwest = { version = "0.13", default-features = false, features = ["json", "rustls", "webpki-roots", "multipart"] }
```
Verify with `cargo check` before writing test code (Pitfall 2). No new crates.

---

## Shared Patterns

### Fail-fast error convention (D-05)
**Source:** `src/provider.rs:30`, `src/utils.rs:186`
**Apply to:** `edit_transport` and any pre-flight edit validation
```rust
anyhow::bail!("Engine '{name}' (model '{model}') cannot accept image input for edits. \
    Configure IMAGAI__ENGINES__<NAME>__MODEL=gpt-image-1 or an OpenRouter vision engine.");
```

### `#[serde(default)]` on optional API fields
**Source:** `src/web.rs:26-64` (`GeneratePayload`), CONVENTIONS.md
**Apply to:** `EditPayload` in `src/web.rs`

### Path-traversal reduction (web filenames)
**Source:** `src/web.rs:384-388` (`serve_image`)
**Apply to:** every `source`/`refs` filename in `POST /api/edit` + traversal test beside `tests/web.rs:202-213`

### Result pipeline (all three frontends converge here)
**Source:** `src/core.rs:12-119` `generate_image_core` → `src/utils.rs` `save_image_from_{url,b64}`
**Apply to:** CLI `cmd_edit`, web `edit` handler, TUI edit submit — build one request DTO each, share resolution logic via one helper (anti-pattern: re-assembling edit fields per surface, `src/cli.rs:187`/`src/web.rs:152`/`src/tui.rs:267` precedent)

### Response serialization
**Source:** `src/web.rs:198-234` result_items loop; `src/cli.rs:235-273` print loop; `src/tui.rs:341-353` `drain_messages` log loop
**Apply to:** edit responses on each surface

### Known-bug guardrails (do NOT encode as expected behavior)
**Source:** `.planning/codebase/CONCERNS.md` + CONTEXT.md canonical refs
- `truncate()` byte-slice panic (`src/provider.rs:445-451`) — format bail messages with `{path:?}`, not `truncate`
- exit-0-on-failure in `cmd_generate` (`src/cli.rs` print loop) — Phase 4; new tests must not assert exit 0 on failure as *desired*

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `provider::generate_edits` (multipart builder) | service | HTTP multipart | First multipart usage in repo — no existing analog; follow research Pattern 1 (`reqwest::multipart::{Form, Part}`) and the `generate_images` HTTP/error skeleton above |
| `provider::edit_transport` (capability gate) | service | pure function | New concept — closest scaffold is the `is_openrouter` dispatch (`src/provider.rs:128-143`); unit tests modeled on `src/provider.rs:453-564` |

## Metadata

**Analog search scope:** `src/`, `tests/`, `web_interface.html`, `Cargo.toml` (flat repo, no subdirs)
**Files scanned:** 16 (whole repo — STRUCTURE.md forbids `src/` subdirectories)
**Tracked-source gate:** all named paths pass `git ls-files -- <path>` (non-empty)
**Pattern extraction date:** 2026-10-06
