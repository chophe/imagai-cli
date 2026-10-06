# Phase 1: Image Editing - Research

**Researched:** 2026-10-04
**Domain:** Rust CLI/TUI/web image tool — OpenAI-compatible image-edit transport, multi-surface parity
**Confidence:** MEDIUM

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

#### Edit command shape
- **D-01:** A new `imagai edit` subcommand (own clap `EditArgs`, parallel to `GenerateArgs`) — not `generate --image`. `generate` remains untouched. — **Reversibility:** reversible — clap subcommands are cheap to add/remove; no published contract yet
- **D-02:** The edit prompt is a **change instruction** ("make it sunset"), not a full re-prompt — the source image carries the visual context. Downstream agents should not design prompts that restate the whole scene.

#### Picking a past image (EDIT-02)
- **D-03:** `--image <path>` always accepts an explicit path; when omitted, the tool uses the **most recent image in `output_dir`** (by mtime) and prints which file it picked. TUI/web expose a small picker listing `output_dir` images, newest first. No history index in this phase — that is Phase 2's scope. — **Reversibility:** reversible — mtime heuristic is local to this phase and replaced by the Phase 2 history tree

#### Multi-reference handling and unsupported engines (EDIT-03)
- **D-04:** Multiple references are supplied as a repeatable `--ref <path>` flag (clap `append`), a JSON array on the web API (`GeneratePayload`-style field with `#[serde(default)]`), and a ref list in the TUI.
- **D-05:** If the configured engine cannot accept image input, **fail fast** with a clear error naming the engine (`anyhow::bail!` convention) — never silently fall back to plain text-to-image generation. Spending credits on the wrong request is worse than an error.

#### Where results land
- **D-06:** Edited results go into the **same flat `output_dir`** as generations (keeps the existing web gallery and `list_images` working unchanged). Filename = source stem + `-edit` suffix following the `suffix_numbered` pattern. — **Reversibility:** costly — filename scheme is user-visible in saved files and docs; changing it later orphans existing files
- **D-07:** PNG metadata (via `inject_png_metadata`) records the edit prompt **and the source filename** — minimal lineage until Phase 2's full tree.

#### the agent's Discretion
User delegated all gray-area choices ("answer by yourself") and approved the resulting decision set wholesale. The researcher/planner have full latitude on: provider wire-format details for the edits endpoint, request DTO field naming, internal function decomposition, test strategy, and how the TUI picker is drawn.

### Deferred Ideas (OUT OF SCOPE)

- Iteration history tree / lineage persistence — Phase 2 (BRCH-01..03)
- Named non-OpenAI-compatible provider adapters — Phase 3 (PRVD-01)
- Retry/backoff and cancellation/progress — v2 (ROB-02, ROB-03)
- Video generation — v2 (VID-01)

Phase boundary (from `<domain>`): out of boundary are "iteration history trees, fork/step-back lineage (Phase 2); new provider wire formats (Phase 3 — Phase 1 works against the engines already configured); exit-code fixes and distribution docs (Phase 4)."
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| EDIT-01 | User can edit an existing image from a file path by combining it with a new prompt | New `imagai edit` subcommand (D-01) → shared core → multipart `/images/edits` transport (Standard Stack § Core, Pattern 1) |
| EDIT-02 | User can use any past generation as the source for a new edit | mtime-newest fallback + shared `list_output_images` helper (Pattern 3); TUI/web picker |
| EDIT-03 | User can pass multiple reference images to a single edit request | repeatable `--ref` (clap `append`), `image[]` repeatable multipart parts, multiple `image_url` chat content parts (Pattern 1/2) |

BRCH-04's surface-parity half applies to editing: the same flow must exist on CLI, TUI, and web (Architectural Responsibility Map).
</phase_requirements>

## Summary

Phase 1 adds an edit pipeline beside the existing generate pipeline, converging on the same `generate_image_core` save/persist path. The transport question is the research crux: OpenAI's documented edit endpoint `POST /v1/images/edits` takes **multipart/form-data** with a repeatable `image[]` file field plus `prompt` and `model` — which means the reqwest dependency already in `Cargo.toml` must gain its `multipart` feature (verified: reqwest 0.13.1 ships `multipart: ["dep:mime_guess","dep:futures-util"]`, so no new crate, only a feature flag). A second, JSON-with-data-URL variant of the same endpoint is also documented (`images: [{file_id | image_url}]`, up to 16 input images) and is the fallback if a gateway rejects multipart. For OpenRouter chat engines the existing `openrouter_chat_generate` path must be generalized from a single `extra_params["image_url"]` to a list of `image_url` content parts (source first, then refs, text part first per OpenRouter guidance).

The biggest planning risks are in-repo, not external: (a) the test mock parses request bodies as `String` (`tests/common/mod.rs:110`), which will reject binary multipart bodies with a 400 before any assertion runs — the mock must capture raw `Bytes`; (b) existing edit tests must configure a `gpt-image-1` model because the shared `cmd()` helper hardcodes `dall-e-3` (`tests/cli.rs:21`), and D-05 requires fail-fast for engines that cannot take image input (dall-e-3 cannot — OpenAI lists dall-e-2/dall-e-3 as retired 2026-05-12 and `/images/edits` as "supports GPT Image models"); (c) `ImageGenerationRequest::default()` sends `response_format: "b64_json"`, `quality: "standard"`, `style: "vivid"` — these must NOT be blindly forwarded to `/images/edits`, whose GPT-image quality enum is `low|medium|high|xhigh|max|auto`, so the edit body should be minimal (`model`, `prompt`, `image[]`).

**Primary recommendation:** add `provider::generate_edits` (multipart `/images/edits`) + a single `edit_transport(&EngineConfig)` capability gate that routes OpenRouter engines to the chat path and bails (`anyhow::bail!`) for everything else; extend `ImageGenerationRequest` with `source_image: Option<String>` + `ref_images: Vec<String>`; resolve source/refs once in a shared helper used by all three frontends; keep the web payload filename-based (files inside `output_dir`) rather than data-URI-based.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Edit command parsing (EDIT-01 flags, `--ref`) | CLI (`src/cli.rs` clap) | — | clap owns arg semantics; maps into the shared request DTO |
| Source/ref resolution + mtime picker (EDIT-02) | Application core / utils (shared helper) | — | all three frontends need identical "newest image" logic; duplicating it repeats the documented `extra_params` anti-pattern |
| Edit wire format (multipart `image[]`, chat content parts) | Provider/transport (`src/provider.rs`) | — | only layer that knows HTTP bodies; Phase 3 adapters swap in here |
| Engine capability gate (D-05 fail-fast) | Provider/transport | core | capability is a property of `EngineConfig` (base_url + model); core surfaces the bail message |
| Save, `-edit` filename, PNG lineage (D-06/D-07) | Core + utils (`src/core.rs`, `src/utils.rs`) | — | existing persistence pipeline is already shared |
| Web edit API (`POST /api/edit`) | API/backend (`src/web.rs`) | — | JSON payload validation, path-traversal guard |
| Web source picker UI | Browser (embedded `web_interface.html`) | Web API `GET /api/images` | static SPA lists output_dir images via the existing endpoint |
| TUI edit form + picker | TUI (`src/tui.rs`) | — | ratatui immediate-mode drawing, App state |

## Standard Stack

No new libraries. Two feature flags on already-locked dependencies.

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| reqwest | 0.13.1 (locked) | HTTP client; gains **`multipart` feature** for `/images/edits` bodies | already the project's only HTTP client; multipart is a first-party feature, no new crate |
| axum | 0.8.9 (locked) | web router + (optionally) `multipart` feature for mock tests | already the web/test server; `multipart` = `dep:multer` if ever needed |
| clap 4.5 `derive` | locked | `EditArgs` subcommand, `--ref` with `append` | existing CLI framework; matches `GenerateArgs` pattern |
| serde/serde_json | locked | `EditPayload` with `#[serde(default)]` | project convention for optional API fields (CONVENTIONS.md) |
| anyhow | locked | `bail!` fail-fast for incapable engines (D-05) | established error convention (`src/provider.rs:30`, `src/utils.rs:186`) |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| base64 0.22 | locked | data-URI encoding of local files for the OpenRouter chat edit path | chat-path engines only |
| crc32fast | locked | PNG tEXt chunk CRC for the new `Source` metadata keyword (D-07) | via existing `inject_png_metadata` |
| ratatui 0.30 / crossterm 0.29 | locked | TUI edit tab + picker | TUI surface only |
| tempfile / assert_cmd / tower (dev) | locked | integration tests for edit flows | existing test harness |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| multipart `image[]` (recommended) | JSON body `images: [{image_url: "data:image/...;base64,..."}]` (also documented by OpenAI) | JSON avoids the reqwest feature flag and matches `build_images_body`'s JSON style, but multipart is the format every OpenAI-compatible gateway proxies and the format the official curl example uses; keep JSON as documented fallback if a gateway 400s on multipart |
| `edit_transport` heuristic gate | new `IMAGAI__ENGINES__<NAME>__*` config field (would touch `src/config.rs:73-78` match) | config override is more precise but adds config surface now; heuristic + bail satisfies D-05 and Phase 3 replaces it with real adapters |
| filename-based web source/refs (recommended) | base64 data-URI upload in the edit payload | data URIs hit axum's default request-body size limit and repeat the known "base64 round-tripped through JSON" bottleneck; filenames reuse `/api/images` and the existing guard pattern |

**Installation:** no `cargo add` needed. Edit `Cargo.toml`:

```bash
# Cargo.toml:17  before
reqwest = { version = "0.13", default-features = false, features = ["json", "rustls", "webpki-roots"] }
# after
reqwest = { version = "0.13", default-features = false, features = ["json", "rustls", "webpki-roots", "multipart"] }
```

**Version verification:** locked versions confirmed from `Cargo.lock` this session: `name = "reqwest"` / `version = "0.13.1"`, `name = "axum"` / `version = "0.8.9"`. Feature flags confirmed from the crates.io API for those exact versions (see Package Legitimacy Audit / Sources).

## Package Legitimacy Audit

**No external packages are installed by this phase.** Two feature flags on crates already in `Cargo.lock` are toggled; no package-legitimacy gate is required.

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| reqwest 0.13.1 (feature `multipart` only) | crates.io | since 2025-12-30 | 9.7M | github.com/seanmonstar/reqwest | n/a (existing dep) | Approved — feature flag only |
| axum 0.8.9 (optional feature `multipart`) | crates.io | since 2026-04-14 | 79.7M | github.com/tokio-rs/axum | n/a (existing dep) | Approved — likely not even needed (raw `Bytes` capture recommended) |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

Feature-flag evidence, verbatim from the crates.io version API [VERIFIED: crates.io API `/api/v1/crates/reqwest/0.13.1`]:

- reqwest: `"multipart":["dep:mime_guess","dep:futures-util"]` — no `stream` dependency, so `Part::bytes(...)` + `.file_name(...)` + `.mime_str(...)` is available without further features.
- axum: `"multipart":["dep:multer"]` and `"default":["form","http1","json","matched-path","original-uri","query","tokio","tower-log","tracing"]` — `multipart` is NOT in axum's defaults, so the current `axum = { version = "0.8", features = ["json", "macros"] }` (`Cargo.toml:18`) does not expose `axum::extract::Multipart`.

## Architecture Patterns

### System Architecture Diagram

```
CLI `imagai edit` ──┐                    (edit flow: source + prompt + refs)
TUI Edit tab ───────┤
Web POST /api/edit ─┤
                    ▼
     ┌──────────────────────────────┐
     │ shared source/ref resolution │  newest-mtime pick (no --image) / filename
     │ (utils: list_output_images)  │  validation + D-05-capable engine check
     └──────────────┬───────────────┘
                    ▼
     ┌──────────────────────────────┐
     │ ImageGenerationRequest       │  + source_image: Option<String>
     │ (models.rs)                  │  + ref_images: Vec<String>
     └──────────────┬───────────────┘
                    ▼
     ┌──────────────────────────────┐
     │ edit_transport(&EngineConfig)│─── not capable ──► anyhow::bail! (D-05)
     │ capability gate              │                    names the engine
     └──────┬───────────────┬───────┘
            ▼               ▼
   gpt-image model    openrouter.ai base_url
   multipart POST     chat/completions JSON
   {base}/images/edits  N × image_url content parts
            │               │
            └───────┬───────┘
                    ▼
     ┌──────────────────────────────┐
     │ generate_image_core          │  filename = <source-stem>-edit[.N].<ext>
     │ → save_image_from_{url,b64}  │  inject_png_metadata(prompt, model,
     └──────────────┬───────────────┘            source)  [D-06, D-07]
                    ▼
          output_dir/ (flat; gallery & /api/images unchanged)
```

### Recommended Project Structure

No new files are required. New code lands in existing flat modules (STRUCTURE.md "Do not create: `src/` subdirectories"):

```
src/
├── cli.rs      # + Commands::Edit, EditArgs, cmd_edit
├── models.rs   # + ImageGenerationRequest.source_image / ref_images
├── provider.rs # + generate_edits(), edit_transport(), multi-image chat parts
├── core.rs     # + edit filename derivation (source-stem + "-edit"), passes source to save
├── utils.rs    # + list_output_images() (moved IMAGE_EXTENSIONS), inject_png_metadata gains source
├── web.rs      # + POST /api/edit route, EditPayload; reuses moved IMAGE_EXTENSIONS
└── tui.rs      # + "Edit" tab (TABS, draw_edit_tab, picker state)
tests/
├── common/mod.rs  # dispatch captures Bytes; + edits route/mode
├── cli.rs         # + edit e2e tests (engine model gpt-image-1)
├── core.rs        # + edit pipeline tests
└── web.rs         # + /api/edit tests
web_interface.html # edit section: source picker (via /api/images) + refs + prompt
```

### Pattern 1: Multipart edit transport (gpt-image engines)

**What:** POST `{base}/images/edits` as `multipart/form-data` with repeatable `image[]` parts.
**When to use:** engine model contains `gpt-image` (OpenAI-compatible `/images/edits`).
**Example:**

```rust
// Source: [CITED: developers.openai.com/api/reference/resources/images/methods/edit]
// curl form equivalent documented by OpenAI:
//   -F "model=gpt-image-1.5" -F "image[]=@a.png" -F "image[]=@b.png" -F 'prompt=...'
use reqwest::multipart::{Form, Part};

let mut form = Form::new()
    .text("model", model.to_string())
    .text("prompt", request.prompt.clone());
for path in std::iter::once(&source).chain(refs.iter()) {
    let bytes = std::fs::read(path)?;                 // bail! with path context on error
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("image.png");
    let part = Part::bytes(bytes)
        .file_name(name.to_string())
        .mime_str(mime_for(name))?;                   // "image/png" | "image/jpeg" | "image/webp"
    form = form.part("image[]".to_string(), part);    // repeatable field name
}
// do NOT add response_format / quality / style — see Pitfall 3
let resp = client.post(&url).bearer_auth(&config.api_key).multipart(form).send().await?;
// parse with the existing ImagesResponse DTO — response shape matches /images/generations
```

### Pattern 2: Chat-path edit (OpenRouter engines)

**What:** source + refs as `image_url` content parts in one user message.
**When to use:** `base_url` contains `openrouter.ai` (existing dispatch convention at `src/provider.rs:128-143`).
**Example:**

```rust
// Source: [CITED: openrouter.ai/docs/guides/overview/multimodal/image-understanding]
// "multiple images can be sent in separate content array entries… we recommend
//  sending the text prompt first, then the images"; local files use data URIs.
let mut content_items: Vec<Value> = vec![json!({"type": "text", "text": request.prompt})];
for p in std::iter::once(&source).chain(refs.iter()) {
    let b64 = base64::engine::general_purpose::STANDARD.encode(std::fs::read(p)?);
    let mime = mime_for(p); // "image/png"…
    content_items.push(json!({"type": "image_url",
        "image_url": {"url": format!("data:{mime};base64,{b64}")}}));
}
// existing openrouter_chat_generate already handles modalities + b64/url extraction;
// only its single extra_params["image_url"] read (src/provider.rs:311-317) must widen to this list.
```

### Pattern 3: Shared source resolution (EDIT-02)

**What:** one helper that lists `output_dir` images newest-first; callers either take `[0]` (CLI default) or render a picker (TUI/web).
**When to use:** every edit entry point on every surface.

```rust
// Recommended signature (new, in src/utils.rs — IMAGE_EXTENSIONS moves here from src/web.rs:19)
/// All image files in `dir`, sorted newest-first by mtime.
/// `pub const IMAGE_EXTENSIONS: [&str; 4] = ["png", "jpg", "jpeg", "webp"];`  [VERIFIED: src/web.rs:19]
pub fn list_output_images(dir: &Path) -> Vec<(PathBuf, SystemTime)>;  // newest first
```

CLI behavior when `--image` omitted: take the first entry and print
`"Using most recent image: <path>"` (D-03); `bail!` with a clear message when the list is empty.

### Pattern 4: Capability gate (D-05)

**What:** one function deciding the transport before any network call, returning the engine name in its error.

```rust
// Recommended (provider.rs). Values it inspects are existing substrings:
//   src/provider.rs:131  base_url contains "openrouter.ai"
//   src/provider.rs:256  model contains "dall-e-3"   // retired + no image input
pub enum EditTransport { MultipartEdits, ChatVision }
pub fn edit_transport(cfg: &EngineConfig) -> anyhow::Result<EditTransport> { /* … */ }
// bail text must name the engine: anyhow::bail!(
//   "Engine '{name}' (model '{model}') cannot accept image input for edits. \
//    Use a gpt-image model engine or an OpenRouter vision/image engine.")
```

This keeps substring routing in one place (CONCERNS flags substring routing as fragile; a single `edit_transport` is the seam Phase 3's `kind`-field refactor will replace).

### Anti-Patterns to Avoid

- **Re-assembling edit fields three times across CLI/web/TUI:** the documented `extra_params` duplication (`src/cli.rs:187`, `src/web.rs:152`, `src/tui.rs:267`) is explicitly called out in ARCHITECTURE.md — resolve source/refs and build the request through ONE shared function.
- **Silent fallback to text-to-image when the engine can't edit:** forbidden by D-05; it burns credits on the wrong request.
- **Routing edit capability on `is_image_model()`:** CONCERNS.md marks that heuristic as display-only ("never gate request routing on it") — `dall-e-3` passes `is_image_model` yet cannot edit.
- **New `src/` subdirectories or a logging framework:** STRUCTURE.md forbids both without an explicit phase.
- **Copying all of `GenerateArgs` into `EditArgs`:** Stability/OpenRouter flags (`--negative-prompt`, `--seed`, `--mode`, `--image-url`) belong to generation; edit needs only prompt/engine/image/ref/output/verbose.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Multipart encoding + boundary/CRLF correctness | manual `Content-Type: multipart/form-data` string building | `reqwest::multipart::Form` / `Part` (feature `multipart`) | boundary bugs are invisible in JSON tests and only surface against real providers |
| Data-URI encoding for chat-path images | ad-hoc base64 concat | `base64::engine::general_purpose::STANDARD` + `format!("data:{mime};base64,{b64}")` | already a dependency, used by `save_image_from_b64` (`src/utils.rs:194-206`) |
| PNG lineage metadata (D-07) | new tEXt writer | extend `inject_png_metadata` (`src/utils.rs:234`) with a `Source` keyword | existing CRC-correct chunk builder with unit tests |
| Filename sanitation for `-edit` names | new sanitizer | `sanitize_filename` / `get_image_extension` (`src/utils.rs:6,38`) | already handles separators/control chars |
| "Newest image in output_dir" per surface | three ad-hoc `read_dir` loops | one `list_output_images` helper (Pattern 3) | web already has a private copy (`src/web.rs:327-378`); a third copy repeats the anti-pattern |
| Image decoding/validation of source bytes | decode with an image crate | send the file bytes as-is; let the provider validate | provider 4xx already surfaces via `extract_api_error` (`src/provider.rs:431`) |
| Parsing multipart in test assertions | adding axum `multipart` feature + multer parsing | capture raw `Bytes` in the mock, assert `body.contains(b"image[]")` / field names | zero new deps; wire-format assertions don't need full form parsing |

**Key insight:** every helper above already exists in this repo for the generation path — Phase 1 is composition (new transport + new entry points) over the existing pipeline, not new infrastructure.

## Common Pitfalls

### Pitfall 1: Mock server rejects binary multipart bodies
**What goes wrong:** every edit integration test fails with HTTP 400 and `captured` stays empty.
**Why it happens:** `async fn dispatch(State(state), Method, Uri, body: String)` (`tests/common/mod.rs:110`) uses axum's `String` extractor, which refuses bodies that are not valid UTF-8 — PNG bytes never are.
**How to avoid:** change the capture to `axum::body::Bytes` (always available, no feature flag) and store raw bytes; keep a lossy/parsed JSON view only for the existing JSON routes.
**Warning signs:** 400s from the mock on `images/edits`; `serde_json::from_str(&body).unwrap_or_else(|_| json!({}))` silently recording `{}`.

### Pitfall 2: Missing reqwest `multipart` feature
**What goes wrong:** compile error — `multipart` is not a member of `reqwest` (or `RequestBuilder::multipart` doesn't exist).
**Why it happens:** `Cargo.toml:17` is `features = ["json", "rustls", "webpki-roots"]` [VERIFIED: Cargo.toml:17] with `default-features = false`.
**How to avoid:** add `"multipart"` in the same edit; verify with `cargo check` before writing test code.

### Pitfall 3: Forwarding generation defaults into the edit body
**What goes wrong:** provider 400 (`invalid quality`, `unknown parameter response_format`) or silently wrong output.
**Why it happens:** `ImageGenerationRequest::default()` sets `quality: "standard".to_string()`, `style: "vivid".to_string()`, `response_format: "b64_json".to_string()` [VERIFIED: src/models.rs:29-33] — values shaped for dall-e-3 `/images/generations`. OpenAI's `/images/edits` documents quality `low|medium|high|xhigh|max|auto` and has no `response_format` in its current parameter list.
**How to avoid:** build the multipart form from scratch (Pattern 1): only `model`, `prompt`, `image[]`; add `n`/`size` only if a test proves the endpoint accepts them.
**Warning signs:** `HTTP 400` mentioning `quality`/`response_format` in provider error output.

### Pitfall 4: Fail-fast gate vs. the default engine
**What goes wrong:** `imagai edit` always bails for users whose default engine is `openai_dalle3` (`.env.example:16-17` configures `MODEL="dall-e-3"`), or — worse — silently generates text-to-image (forbidden by D-05).
**Why it happens:** OpenAI documents `/images/edits` as "supports GPT Image models", with dall-e-2/dall-e-3 retired 2026-05-12 [CITED: developers.openai.com/api/reference/resources/images/methods/generate].
**How to avoid:** the bail message must (1) name the engine and model, (2) point at a capable engine ("configure `IMAGAI__ENGINES__OPENAI_GPT__MODEL=gpt-image-1` or an OpenRouter vision engine"). Test it: `edit_transport` unit tests for gpt-image → Ok, dall-e-3 → Err, openrouter+gemini → Ok, stability → Err.
**Warning signs:** edit tests that pass with model `dall-e-3` (they're testing the wrong branch — the shared `cmd()` helper defaults to it, `tests/cli.rs:21`).

### Pitfall 5: TUI row-constant lockstep panic
**What goes wrong:** runtime `unreachable!()` panic when opening the Edit tab.
**Why it happens:** row behavior is spread across four `match` blocks (`text_field`, `text_field_mut` `src/tui.rs:200` ends in `unreachable!()`, `bool_field`, `row_label`) keyed on `ROW_*` constants [VERIFIED: src/tui.rs:17-28: `const ROWS: usize = 11;` … `const ROW_SUBMIT: usize = 10;`].
**How to avoid:** give the Edit tab its own row constants and tab-scoped `focus`/`editing` state plus its own `draw_edit_tab`/`handle_key` branch — do not extend the Generate tab's `ROWS` range. Keep `Tab`/`BackTab` interception (`src/tui.rs:938-945`) working while a picker overlay is open (the picker should consume arrow keys first).
**Warning signs:** `focus >= ROWS` reaching `text_field_mut`.

### Pitfall 6: `Source` filename in a PNG tEXt chunk
**What goes wrong:** malformed tEXt (Latin-1/NUL rule) silently drops all metadata.
**Why it happens:** `inject_png_metadata` writes ISO-8859-1 tEXt chunks [VERIFIED: src/utils.rs:234: `pub fn inject_png_metadata(bytes: &[u8], prompt: &str, model: &str) -> Vec<u8>`] and is already listed as a known bug for non-Latin-1 prompts (CONCERNS.md).
**How to avoid:** lossily map the source filename to Latin-1 (or skip the `Source` chunk when non-representable); the existing test asserting exactly two tEXt chunks (`src/utils.rs:375`: `assert_eq!(texts.len(), 2, "Prompt + Model tEXt chunks");`) must be updated/parameterized for the third keyword.

### Pitfall 7: Error-path panics with user filenames
**What goes wrong:** panic inside error formatting when a source path is missing/non-ASCII.
**Why it happens:** `truncate()` slices bytes blindly (`src/provider.rs:445-451`) and `sanitize_filename` truncates at byte 100 (`src/utils.rs:31-33`) — both are documented panic bugs on the error path (CONCERNS.md "Known Bugs").
**How to avoid:** do not pass user-controlled paths through `truncate`; format bail messages with `{path:?}` (Debug of Path is byte-safe). Do not encode these panics as expected behavior in new tests (explicitly warned in 01-CONTEXT.md canonical refs).

### Pitfall 8: Exit-code inconsistency on edit failures
**What goes wrong:** `imagai edit` failure behavior diverges from `imagai generate`.
**Why it happens:** `cmd_generate` prints `[Error]` then returns `Ok(())` → exit 0 (known bug, ROB-01 = Phase 4). D-05 mandates `anyhow::bail!` for incapable engines, which exits non-zero via `main`.
**How to avoid:** accept the split for Phase 1 — pre-flight capability errors `bail!` (exit 1, consistent with D-05), post-request provider errors mirror `cmd_generate`'s print loop. New edit tests may assert `.failure().code(1)` for bail cases but must NOT assert exit 0 as *desired* behavior for failures, and `src/cli.rs`/`tests/cli.rs` generate behavior stays untouched (Phase 4 scope).

### Pitfall 9: Web source path traversal / body size
**What goes wrong:** `POST /api/edit` with `source: "../../.env"` reads outside `output_dir`, or a data-URI payload trips axum's default 2 MB body limit [ASSUMED].
**Why it happens:** the write side already joins user strings unchecked (`src/core.rs:45-73`, CONCERNS.md); the read side guard exists only in `serve_image` (`src/web.rs:385-387`: `Path::new(&filename).file_name()`).
**How to avoid:** treat web `source`/`refs` as **filenames**, reduce to final component and join under `output_dir` exactly like `serve_image`; 404-style JSON error when missing. This also keeps payloads small (Pitfall 3 of Alternatives).

## Code Examples

Verified patterns from official sources:

### Edit subcommand wiring (clap)

```rust
// Source: existing pattern — src/cli.rs:22-36 [VERIFIED: src/cli.rs:22-36]
// pub enum Commands {
//     /// Generate images from a text prompt
//     Generate(Box<GenerateArgs>),
//     /// List configured engines (optionally fetch available models)
//     ListEngines { … },
//     /// Launch the interactive terminal UI
//     Tui,
//     /// Start the web interface (REST API + static UI)
//     Web(WebArgs),
// }
#[derive(Subcommand)]
pub enum Commands {
    /// Edit an existing image with a change instruction
    Edit(Box<EditArgs>),
    // … existing variants untouched
}

#[derive(Args, Clone)]
pub struct EditArgs {
    /// Source image path. If omitted, the most recent image in output_dir is used.
    #[arg(long)]
    pub image: Option<String>,
    /// Reference image path (repeatable).
    #[arg(long = "ref",)]
    pub refs: Vec<String>,
    /// Change instruction ("make it sunset") — the source carries the scene.
    #[arg(short, long)]
    pub prompt: Option<String>,
    #[arg(long)]
    pub engine: Option<String>,
    #[arg(short, long)]
    pub output: Option<String>,
    #[arg(long)]
    pub verbose: bool,
}
```

### Web route + payload (JSON, `#[serde(default)]`)

```rust
// Source: existing pattern — src/web.rs:27-64 GeneratePayload, routes at src/web.rs:96-105
#[derive(Deserialize)]
struct EditPayload {
    prompt: String,
    #[serde(default)]
    engine: Option<String>,
    /// Filename inside output_dir (final path component only).
    #[serde(default)]
    source: Option<String>,
    /// Reference filenames inside output_dir.
    #[serde(default)]
    refs: Vec<String>,
    #[serde(default)]
    output: Option<String>,
    #[serde(default)]
    verbose: bool,
}

// router(): .route("/api/edit", post(edit))  // alongside /api/generate
```

### Mock edits route (raw bytes)

```rust
// Source: existing pattern — tests/common/mod.rs:106-134, body capture at :110
async fn dispatch(State(state): State<MockState>, method: Method, uri: Uri, body: Bytes) -> Response {
    let path = uri.path().to_string();
    let parsed: Value =
        String::from_utf8_lossy(&body).parse().unwrap_or_else(|_| json!({}));
    state.captured.lock().unwrap().push((path.clone(), parsed)); // keep JSON view…
    state.raw.lock().unwrap().push((path.clone(), body.to_vec())); // …plus raw bytes for multipart
    if path.contains("images/edits") { return edits_logic(state).await; }
    // …existing routes unchanged
}
```

### Derived output filename (D-06)

```rust
// Source: naming pattern from src/core.rs:122-130 suffix_numbered [VERIFIED: src/core.rs:121-130]
// /// Add a `_<i+1>` suffix before the extension when generating multiple images.
// fn suffix_numbered(filename: &str, index: usize, total: u32) -> String
//
// Edit naming: source stem + "-edit"; core's existing n>1 branch then yields
//   single : sunset_edit.png            (source "sunset_20261004_120000.png")
//   n = 2  : sunset_edit_1.png / sunset_edit_2.png   (same "{stem}_{i+1}.{ext}" shape)
fn edit_output_filename(source: &Path) -> String {
    let stem = source.file_stem().and_then(|s| s.to_str()).unwrap_or("image");
    let ext = get_image_extension(source);
    format!("{stem}-edit.{ext}")
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| dall-e-2/dall-e-3 as editable models | OpenAI: `/images/edits` "supports GPT Image models"; dall-e-2/dall-e-3 **retired 2026-05-12** | 2026-05-12 [CITED: developers.openai.com] | capability gate must treat `dall-e-3` engines as edit-incapable (D-05), even though they remain valid for generation on gateways |
| edits = multipart-only | dual shape: multipart `image[]` **and** JSON `images: [{file_id \| image_url}]` (data URLs ≤ 20,971,520 chars) | 2025-2026 docs [CITED: developers.openai.com/api/reference/resources/images/methods/edit] | multipart is primary; JSON data-URL variant is a verified fallback |
| single reference image | OpenAI: up to **16** input images for GPT image models | current docs [CITED: same] | cap `--ref` count at 16 for gpt-image engines (soft cap; chat path varies per provider) |
| image input via one `extra_params["image_url"]` | OpenRouter: N `image_url` content parts, text part first, data URIs for local files | current docs [CITED: openrouter.ai/docs/guides/overview/multimodal/image-understanding] | chat-path edit = widen the existing single-image read at `src/provider.rs:311-317` |
| — | OpenRouter also ships a dedicated **Image API** with optional reference images | current docs [CITED: openrouter.ai/docs/guides/overview/multimodal/image-generation] | new wire format → Phase 3 (PRVD-01) scope, not Phase 1 |

**Deprecated/outdated:**
- `dall-e-2`/`dall-e-3` on official OpenAI: retired 2026-05-12; use GPT image models (still fine to *generate* through third-party gateways — do not remove them from generation paths).
- `--image-url` flag on `generate`: it feeds only the OpenRouter chat branch today; leave it alone (D-01: `generate` untouched).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | The user's real engines for edit UAT include a `gpt-image*` model or an OpenRouter vision/image engine (`.env` was not read) | Summary, Pattern 4 | If all configured engines are edit-incapable, the phase can only be verified against the mock; needs a `checkpoint:human-verify` with a live key |
| A2 | Sending only `model`, `prompt`, `image[]` (no `response_format`/`quality`/`style`/`n`) is accepted by OpenAI and OpenAI-compatible gateways on `/images/edits` | Pattern 1, Pitfall 3 | 400s at first live call; fallback = add fields per-endpoint after observing errors |
| A3 | Third-party OpenAI-compatible gateways expose `/images/edits` multipart rather than only the JSON variant | Standard Stack, Pitfall 2 | multipart rejected → switch that engine to the documented JSON `images:[{image_url}]` body (feature flag already verified) |
| A4 | axum's default request-body limit is 2 MB (affects any data-URI design; reason to prefer filename payloads) | Pitfall 9 | larger edits on web fail with 413; mitigated by filename-based payload |
| A5 | Capability detection by substring (`gpt-image`, `openrouter.ai`) is adequate for Phase 1 | Pattern 4 | a gateway with a non-matching model id bails incorrectly — message must tell users how to proceed; Phase 3 replaces this with explicit adapters |
| A6 | `IMAGE_EXTENSIONS` (web.rs) is the right filter for the picker — note it excludes `gif` while `get_image_extension` accepts `gif` [VERIFIED: src/web.rs:19 `const IMAGE_EXTENSIONS: [&str; 4] = ["png", "jpg", "jpeg", "webp"];`; src/utils.rs:44-48 accepts `"gif"`] | Pattern 3 | gif sources unselectable by picker; decide once when moving the constant to `utils.rs` |
| A7 | TUI Edit surface = new tab (vs. extra rows on the Generate tab) | Pitfall 5 | either works; new tab avoids touching the fragile four-match lockstep; UI-SPEC may constrain this (`ui_phase: true`) |
| A8 | Web edit source = files inside `output_dir` only (no browser-side new upload) in MVP | Alternatives, Pitfall 9 | if the UI hint expects upload-to-edit, the existing dead `input_image` widget (`web_interface.html:647-649`) gets wired to `/api/edit` as a data-URI source instead |

**If this table is empty:** n/a — 8 assumed claims above need confirmation during discuss/plan (A1 and A2 are the two that can block live verification).

## Open Questions

1. **Which live engine will prove EDIT-01 at UAT?**
   - What we know: mock covers the wire format; `.env` exists but was not read (gitignored, per project convention).
   - What's unclear: whether a gpt-image/OpenRouter engine with a real key is available.
   - Recommendation: verify against the mock in phase tests; add an end-of-phase `checkpoint:human-verify` (`human_verify_mode: "end-of-phase"`) for one live edit.
2. **Should `--ref` count be capped per engine (16 for gpt-image)?**
   - What we know: OpenAI documents up to 16 input images for GPT image models; chat path is provider-dependent.
   - What's unclear: whether to enforce client-side or let the provider 400.
   - Recommendation: soft-cap at 16 for the multipart transport with a warning, let chat path pass through; keeps D-05's fail-fast for *engines*, not ref counts.
3. **Does the web edit UI reuse the existing upload widget as an additional source?**
   - Recommendation: MVP = picker from `/api/images` (+ optional reuse of the upload widget as *reference*, A8); flag for UI-SPEC phase (`ui_phase: true`, `ui_safety_gate: true` in `.planning/config.json`).

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| cargo / rustc | build, tests | ✓ | 1.97.1 (c980f4866 / 8bab26f4f) | — |
| git | commits (`commit_docs: true`) | ✓ | present; worktree clean at 871948e | — |
| reqwest `multipart` feature | edit transport | ✓ (locked dep, feature not yet enabled) | 0.13.1 | JSON data-URL variant (A3) |
| External provider APIs (OpenAI/OpenRouter) | live UAT only | ✗ (keys not read this session) | — | mock server (`tests/common/mod.rs`) + human verify at phase end |
| graft CLI (AGENTS.md workflow) | repo context graph | ✗ (binary not installed; `graft/` nodes exist as markdown) | — | read `graft/INDEX.md` nodes directly; fall back to `.planning/codebase/` docs |
| Docker / databases / services | — | n/a | — | none needed: all tests are in-process |

**Missing dependencies with no fallback:**
- Live provider keys for real edit verification — mitigated by `checkpoint:human-verify` at phase end (mock covers CI).

**Missing dependencies with fallback:**
- `graft` CLI — markdown nodes + `.planning/codebase/` docs substitute.

## Security Domain

`security_enforcement: true` in `.planning/config.json` (verbatim: `"security_enforcement": true`), so this section is included. ASVS level 1 (`"security_asvs_level": 1`).

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|------------------|
| V2 Authentication | no | web stays localhost-only by convention (PROJECT.md Out of Scope) |
| V3 Session Management | no | stateless local tool |
| V4 Access Control | no | single-user local surface |
| V5 Input Validation | **yes** | web `source`/`refs`: reduce to final path component + join under `output_dir` (mirror `serve_image`, `src/web.rs:385-387`); `#[serde(default)]` typed `EditPayload`; cap `refs.len()`; CLI `--image` path is caller-owned local input — validate existence with a clear bail |
| V6 Cryptography | no | no crypto; API keys stay bearer tokens from env |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Path traversal via edit `source`/`refs` on web | Tampering | `Path::file_name()` reduction before `output_dir.join` (same guard as `serve_image`); add a traversal test beside `tests/web.rs:202-213` |
| Credit theft: unauthenticated `/api/edit` (like `/api/generate`) | Tampering/DoS | inherit existing posture (localhost convention); cap `refs` and re-use `n` bounds — do not expand the attack surface (no shell-out, no new endpoints beyond `/api/edit`) |
| Prompt/filename leakage via PNG `tEXt` (now includes `Source`) | Information Disclosure | document in README; filenames are local already; non-Latin-1 handled per Pitfall 6 |
| Uploaded bytes executed/parsed unsafely | Tampering | bytes are forwarded verbatim to the provider, never written outside `output_dir`, never executed |

## Sources

### Primary (MEDIUM — official docs via Context7, `classify-confidence --provider context7` = MEDIUM)

- `/websites/developers_openai_api_reference` → https://developers.openai.com/api/reference/resources/images/methods/edit — multipart fields (`model`, `image[]`, `prompt`), multi-image support, response shape
- `/websites/developers_openai_api` → https://developers.openai.com/api/docs/guides/images — JSON `images: [{file_id|image_url}]` variant, GPT-image-only edits, dall-e-2/3 retirement (2026-05-12)
- `/seanmonstar/reqwest` (Context7) → `RequestBuilder::multipart`, `multipart::Form`/`Part` API, `multipart` cargo feature
- `/websites/openrouter_ai` → https://openrouter.ai/docs/guides/overview/multimodal/image-understanding — multiple `image_url` parts, text-first ordering, data URIs; Image Generation API (noted as Phase 3 scope)
- crates.io version API `/api/v1/crates/reqwest/0.13.1` and `/api/v1/crates/axum/0.8.9` — feature-flag evidence quoted verbatim above [VERIFIED: crates.io API]

### Secondary (HIGH for this repo — read directly this session)

- `src/` modules: `cli.rs`, `core.rs`, `provider.rs`, `models.rs`, `utils.rs`, `web.rs`, `tui.rs`, `config.rs`
- `.planning/codebase/`: ARCHITECTURE.md, STRUCTURE.md, CONVENTIONS.md, CONCERNS.md, TESTING.md
- `.planning/phases/01-image-editing/01-CONTEXT.md`, `.planning/REQUIREMENTS.md`, `.planning/STATE.md`, `.planning/PROJECT.md`, `.planning/config.json`
- `Cargo.toml`, `Cargo.lock`, `.env.example`, `tests/cli.rs`, `tests/common/mod.rs`, `web_interface.html`

### Tertiary (LOW)

- None — no websearch-only claims retained (the one OpenRouter websearch returned no results and was superseded by official docs).

## Metadata

**Confidence breakdown:**
- Standard stack: **HIGH** — locked versions read from `Cargo.lock`; feature flags confirmed on the registry for those exact versions; no new packages
- Architecture: **MEDIUM** — layered on verified in-repo patterns, but the transport choice (multipart vs JSON data-URL) and capability matrix rest on official docs (MEDIUM) plus gateway-behavior assumptions (A2/A3)
- Pitfalls: **HIGH** — every pitfall cites code read this session (mock `String` body, row-constant lockstep, default request fields, tEXt constraints)

**Confidence per seam:** `classify-confidence --provider context7` → MEDIUM (even `--verified`); `--provider websearch` → LOW (unused). Overall phase confidence: MEDIUM.

**Research date:** 2026-10-04
**Valid until:** 2026-11-03 (30 days — stable stack, locked deps; re-check OpenAI edits docs if the phase slips past that)

*Validation Architecture omitted: `.planning/config.json` sets `"nyquist_validation": false` (read this session). Runtime State Inventory omitted: this is not a rename/refactor/migration phase.*

