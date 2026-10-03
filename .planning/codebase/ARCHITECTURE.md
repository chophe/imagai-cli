---
last_mapped_commit: f37fbe5c1e16138cf0fbfaf77157b2e28b0f2a9c
last_mapped_at: 2026-10-03
---
<!-- refreshed: 2026-10-03 -->

# Architecture

**Analysis Date:** 2026-10-03

## System Overview

```text
┌──────────────────────────────────────────────────────────────────┐
│                        ENTRY POINTS                              │
│  src/main.rs → imagai::cli::run()                                │
│  subcommands: Generate | ListEngines | Tui | Web                 │
│  `src/cli.rs` (clap derive)                                      │
└───────┬──────────────────────────┬───────────────────┬───────────┘
        │                          │                   │
        ▼                          ▼                   ▼
┌───────────────────┐   ┌────────────────────┐  ┌───────────────────┐
│  CLI renderer     │   │  TUI (ratatui)     │  │  Web (axum)       │
│  `src/cli.rs`     │   │  `src/tui.rs`      │  │  `src/web.rs`     │
│  stdout/exit code │   │  `src/tui.rs:915`  │  │  `src/web.rs:88`  │
└─────────┬─────────┘   └─────────┬──────────┘  └─────────┬─────────┘
          │    all three build an ImageGenerationRequest  │
          └───────────────────────┬───────────────────────┘
                                  ▼
┌──────────────────────────────────────────────────────────────────┐
│                      APPLICATION CORE                            │
│  src/core.rs :: generate_image_core(request, settings)            │
│  engine resolution → provider call → filename choice → save       │
└────────────────────────────┬─────────────────────────────────────┘
                             ▼
┌──────────────────────────────────────────────────────────────────┐
│                     PROVIDER / TRANSPORT                          │
│  src/provider.rs :: generate_images / chat_completion /           │
│  fetch_models — OpenAI-compatible HTTP via reqwest (rustls)       │
└────────────────────────────┬─────────────────────────────────────┘
                             ▼
┌──────────────────────────────────────────────────────────────────┐
│  CONFIG: src/config.rs (Settings, .env + IMAGAI__* vars)          │
│  MODELS: src/models.rs (request/response DTOs)                    │
│  UTILS:  src/utils.rs (filenames, PNG tEXt metadata, file I/O)    │
│  OUTPUT: generated_images/  (default output_dir)                  │
└──────────────────────────────────────────────────────────────────┘
```

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| Binary entry | Parse CLI args, load `Settings`, dispatch | `src/main.rs` |
| Command routing | clap `Cli`/`Commands`, per-subcommand handlers | `src/cli.rs` |
| Core orchestration | Engine lookup → provider call → filename → save → response list | `src/core.rs` |
| HTTP provider | Build request bodies, POST to engines, parse responses | `src/provider.rs` |
| Config | Load `.env`/env vars into `Settings` + `EngineConfig` | `src/config.rs` |
| DTOs | `ImageGenerationRequest`, `ImageGenerationResponse`, chat/model types | `src/models.rs` |
| Utilities | Filename generation/sanitizing, PNG metadata injection, save from URL/b64 | `src/utils.rs` |
| TUI frontend | ratatui form + log pane, async generation via mpsc | `src/tui.rs` |
| Web frontend | axum REST API, static HTML, image gallery/serving | `src/web.rs` |
| Embedded UI | Single-file SPA served at `/` | `web_interface.html` |

## Pattern Overview

**Overall:** Modular monolith with a shared core and three thin frontends (CLI, TUI, Web) — a "ports-and-adapters" shape without the trait indirection.

**Key Characteristics:**

- One binary (`imagai`) exposes all three interfaces as clap subcommands.
- All frontends converge on the same two functions: `generate_image_core` (`src/core.rs:12`) and `Settings::load` (`src/config.rs:39`).
- No traits/abstractions for providers — dispatch is runtime `if`/`match` on `EngineConfig` (`src/provider.rs:120`).
- Errors are carried as values (`ImageGenerationResponse.error: Option<String>`), not as `Result`, through the generation path; `anyhow::Result` is used at the edges (CLI/main/web server startup).
- Async everywhere: `tokio` multi-thread runtime; a single `#[tokio::main]` in `src/main.rs`.

## Layers

**Presentation (adapters):**

- Purpose: translate user input into an `ImageGenerationRequest`, render results.
- Location: `src/cli.rs`, `src/tui.rs`, `src/web.rs`
- Contains: clap arg structs, ratatui draw functions, axum handlers.
- Depends on: `core`, `config`, `models`, `utils`, `provider` (for `fetch_models`/`chat_completion` only).
- Used by: `src/main.rs`.

**Application core:**

- Purpose: the single generation pipeline — resolve engine, call provider, choose filename, persist image, report outcome.
- Location: `src/core.rs`
- Contains: `generate_image_core`, `suffix_numbered`.
- Depends on: `config`, `models`, `provider`, `utils`.
- Used by: `src/cli.rs`, `src/tui.rs`, `src/web.rs` (and integration tests in `tests/core.rs`).

**Infrastructure / transport:**

- Purpose: all outbound HTTP to AI providers.
- Location: `src/provider.rs`
- Contains: `http_client`, `resolve_url`, `build_images_body`, `generate_images`, `openrouter_chat_generate`, `chat_completion`, `fetch_models`.
- Depends on: `config::EngineConfig`, `models`, `reqwest`.
- Used by: `src/core.rs`, `src/cli.rs` (list-engines), `src/tui.rs` (models fetch).

**Data / contracts:**

- Purpose: serde DTOs for request and response payloads.
- Location: `src/models.rs`
- Contains: `ImageGenerationRequest`, `ImageGenerationResponse`, `ImagesResponse`, `ImageData`, `ChatResponse`, `ModelsListResponse`.
- Depends on: `serde`, `std`.
- Used by: every layer.

**Shared utilities:**

- Purpose: pure-ish helpers with no business branching.
- Location: `src/utils.rs`
- Contains: `sanitize_filename`, `generate_filename`, `generate_filename_from_prompt_llm`, `save_image_from_url`, `save_image_from_b64`, `inject_png_metadata`.
- Depends on: `config` (for LLM filename call), `reqwest`, `crc32fast`.
- Used by: `src/core.rs`, `src/cli.rs`.

**Configuration:**

- Purpose: env-driven settings, engine registry.
- Location: `src/config.rs`
- Contains: `Settings`, `EngineConfig`, `PLACEHOLDER_KEY`.
- Depends on: `dotenvy`, `std::env`.
- Used by: every layer.

## Data Flow

### Primary Request Path (CLI `imagai generate`)

1. `src/main.rs:6` — `Cli::parse()` then `Settings::load()` then `cli::run()`.
2. `src/cli.rs:123` — `run()` matches `Commands::Generate` → `cmd_generate()`.
3. `src/cli.rs:132` — resolve engine (flag → `settings.default_engine`), read prompt (arg or stdin), build `extra_params` map (Stability/OpenRouter-specific keys).
4. `src/cli.rs:228` — construct `ImageGenerationRequest`, call `generate_image_core(&request, settings)`.
5. `src/core.rs:12` — `settings.get_engine()` lookup (error response if missing) → `provider::generate_images()`.
6. `src/provider.rs:120` — route: OpenRouter+Gemini → `openrouter_chat_generate` (`src/provider.rs:293`); otherwise `build_images_body` → POST `{base_url}/images/generations` with bearer auth.
7. `src/core.rs:12` (loop) — pick filename (`output_filename` / `auto_filename` LLM / `random_filename` / prompt-derived), then `save_image_from_url` or `save_image_from_b64` into `settings.output_dir`; PNG metadata injected by `inject_png_metadata` (`src/utils.rs:234`).
8. `src/cli.rs:238` — iterate results, print success/error + `print_usage_cost()`.

### Web API Flow (`POST /api/generate`)

1. `src/web.rs:88` — `router(settings)` builds `Router` with `AppState { settings: Arc<Settings> }` and permissive CORS (`tower-http`).
2. `src/web.rs:136` — `generate()` deserializes `GeneratePayload`, resolves engine, maps Stability extras into `extra_params`.
3. Calls `generate_image_core` (same core as CLI).
4. Response JSON includes per-image `saved_path`, base64 `image_data` data-URI (re-read from disk), `image_url`, `text_content`.

### Web Shell Flow (`POST /api/generate-cli`)

1. `src/web.rs:240` — allow-list check on the command prefix (`imagai`, `cargo run -- imagai`, `cargo run --quiet -- imagai`).
2. Runs the command through `sh -c` under a 300s `tokio::time::timeout`.
3. On success, scans `settings.output_dir` for PNGs modified recently (`is_recent`, `src/web.rs:380`) and returns them base64-encoded.

### TUI Flow

1. `src/tui.rs:915` — `run(settings)` sets up `ratatui`/`crossterm` terminal and calls `run_loop`.
2. `src/tui.rs:241` — `start_generation()` builds `ImageGenerationRequest` via `build_request()` (`src/tui.rs:267`) and spawns generation on a task, delivering results over `mpsc::UnboundedReceiver` (`ModelsFetch`-style channel pattern, `src/tui.rs:138`).
3. `src/tui.rs:325` — `drain_messages()` moves completed results into the log pane (`push_log`, `LogKind`).
4. `src/tui.rs:457` — immediate-mode `draw()` per frame: header, active tab (`Generate`/`Engines`/`About`), form rows indexed by `ROW_*` constants (`src/tui.rs:17`), logs, footer.

**State Management:**

- `Settings` is loaded once in `main` and passed by reference (`&Settings`) to CLI/TUI; the web server clones it into `Arc<Settings>` inside `AppState` (`src/web.rs:22`) — the only shared state in the app.
- No database, no global mutable state at runtime. Test-only globals: `ENV_LOCK` mutex in `src/config.rs:134` serializes env-mutating config tests.
- TUI keeps all UI state in a single `App` struct (`src/tui.rs:109`); channels carry async results into the render loop.

## Key Abstractions

**`Settings` / `EngineConfig`:**

- Purpose: engine registry — which API keys, base URLs, and models exist.
- Examples: `src/config.rs:24`, `src/config.rs:9`
- Pattern: plain structs loaded from env; `key_set()` guards placeholder keys (`src/config.rs:5`).

**`ImageGenerationRequest`:**

- Purpose: the single normalized input shape all three frontends produce.
- Examples: `src/models.rs:8`
- Pattern: struct with `Default` (`src/models.rs:24`) — frontends build it with `..Default::default()`-style full literal or default fallbacks. Provider-specific knobs travel in `extra_params: HashMap<String, Value>` rather than new fields.

**`ImageGenerationResponse`:**

- Purpose: the single normalized output — carries `error`, `image_url`, `image_b64_json`, `text_content`, `saved_path`, `usage`.
- Examples: `src/models.rs:44`
- Pattern: error-as-data. Core returns `Vec<ImageGenerationResponse>` instead of `Result` so one failed image doesn't sink the batch.

**Provider dispatch (implicit strategy):**

- Purpose: choose request shape per engine family.
- Examples: `src/provider.rs:120` (`generate_images`), `src/provider.rs:243` (`build_images_body`)
- Pattern: runtime inspection of `base_url` (e.g. contains `openrouter.ai`) and `model` name — no trait, no enum of providers.

**`MockServer`:**

- Purpose: in-process OpenAI-compatible fake for integration tests.
- Examples: `tests/common/mod.rs:49`
- Pattern: axum app with `ImagesMode`/`ChatMode` toggles, records requests for assertion (`tests/common/mod.rs:92`).

## Entry Points

**Binary `imagai`:**

- Location: `src/main.rs` → `[[bin]] name = "imagai"` in `Cargo.toml`
- Triggers: process start; `Cli::parse()` panics/exits on bad args (clap behavior).
- Responsibilities: load settings, dispatch to subcommand.

**Subcommand `generate`:**

- Location: `src/cli.rs:132`
- Triggers: `imagai generate -p ...`
- Responsibilities: build request, run core, print results, `print_usage_cost`.

**Subcommand `list-engines`:**

- Location: `src/cli.rs:276`
- Triggers: `imagai list-engines [--all]`
- Responsibilities: print configured engines; optionally `provider::fetch_models` per engine.

**Subcommand `tui`:**

- Location: `src/tui.rs:915` (`run`), loop at `src/tui.rs:924`
- Triggers: `imagai tui`
- Responsibilities: full-screen interactive form + generation.

**Subcommand `web`:**

- Location: `src/web.rs:470` (`serve`), routes at `src/web.rs:88`
- Triggers: `imagai web --host 0.0.0.0 --port 5000`
- Responsibilities: bind `TcpListener`, `axum::serve`.

**Web routes:**

- `GET /` → embedded `web_interface.html` (`src/web.rs:18`, handler `src/web.rs:107`)
- `GET /api/engines` → `src/web.rs:115`
- `POST /api/generate` → `src/web.rs:136`
- `POST /api/generate-cli` → `src/web.rs:237`
- `GET /api/images` → `src/web.rs:327`
- `GET /api/images/{filename}` → `src/web.rs:380` (path-traversal guarded)

## Architectural Constraints

- **Threading:** async tokio multi-thread runtime; blocking work is done inline in async contexts (e.g. `std::fs::read` inside web handlers `src/web.rs:136`, `std::fs::read_dir` in `list_images` `src/web.rs:327`) rather than `spawn_blocking`.
- **Global state:** none at runtime. `static ENV_LOCK: Mutex<()>` in `src/config.rs:134` is test-only. `Settings` is cloned/`Arc`-wrapped, never mutated after load.
- **Circular imports:** none — dependency direction is strictly `frontends → core → provider/utils → models/config`. `src/lib.rs` declares flat sibling modules only.
- **Runtime config:** engine config comes only from environment variables prefixed `IMAGAI__` (loaded via `dotenvy` from cwd or repo root `.env`, `src/config.rs:39`); there is no config file parser.
- **No persistence layer:** images are the only output; output dir defaults to `generated_images/` (`src/config.rs:43`).
- **Runtime codegen:** the web UI is `include_str!("../web_interface.html")` — recompilation required to change it (`src/web.rs:18`).

## Anti-Patterns

### Duplicated `extra_params` assembly across frontends

**What happens:** The Stability/OpenRouter extra-parameter mapping (`negative_prompt`, `seed`, `strength`, `output_format`, `aspect_ratio`, `mode`/`image_url`) is written three times: `src/cli.rs:187`, `src/web.rs:152`, `src/tui.rs:267`.
**Why it's wrong:** Adding a provider parameter requires editing three files; the web variant already diverges (`input_image` accepted-but-unused at `src/web.rs:175`, CLI uses `image_url`).
**Do this instead:** Extract a `fn build_extra_params(...) -> HashMap<String, Value>` into `src/models.rs` or `src/core.rs` and call it from all three frontends.

### Prefix-based shell allow-list for `/api/generate-cli`

**What happens:** `src/web.rs:240` permits any command whose trimmed start matches `imagai` or `cargo run -- imagai`, then executes it via `sh -c`.
**Why it's wrong:** `imagai ; rm -rf /` and `imagai $(...)` pass the check — prefix matching on a shell command is bypassable. Server binds `0.0.0.0` by default (`src/cli.rs:41`).
**Do this instead:** Spawn the `imagai` binary with an argv array (`tokio::process::Command::new(bin).args(...)`) and never route through `sh`; drop the prefix allow-list. CORS is also fully permissive (`src/web.rs:94`).

### Blocking I/O inside async handlers

**What happens:** `std::fs::read`/`read_dir` runs directly in axum handlers and in `generate_image_core` (`src/core.rs:90`), and base64-encodes whole image files per request (`src/web.rs:207`).
**Why it's wrong:** Blocks the runtime worker thread; large galleries re-read and re-encode every image on each `/api/generate` call.
**Do this instead:** Use `tokio::fs` or `spawn_blocking`; serve saved images by redirecting to `GET /api/images/{filename}` instead of inlining data URIs.

### Error-as-string accumulation in core

**What happens:** `generate_image_core` returns `Vec<ImageGenerationResponse>` where failure is `Some(error)` and callers re-check it (`src/core.rs:12`, `src/cli.rs:238`, `src/web.rs:195`).
**Why it's wrong:** Callers must remember to check `error` first; `.ok()` on the save result (`src/core.rs:100`) collapses distinct I/O failures into a generic message.
**Do this instead:** Keep the batch semantics but return `Result<ImageGenerationResponse, anyhow::Error>` per item and convert at the presentation boundary.

### `web.rs` mixed responsibilities

**What happens:** `src/web.rs` (477 lines) contains routing, JSON payloads, shell execution, filesystem scanning, base64 encoding, and hand-rolled calendar math (`civil_from_days` `src/web.rs:421`, `datetime_iso` `src/web.rs:406`).
**Why it's wrong:** Unrelated concerns in one file raise the cost of any web change; chrono is already a dependency but unused here.
**Do this instead:** Split handlers into `src/web/handlers.rs` (or a `web` module dir) and use `chrono` for ISO timestamps instead of manual conversion.

## Error Handling

**Strategy:** Hybrid — `anyhow::Result` at process/server boundaries, error-as-data inside the generation pipeline.

**Patterns:**

- `anyhow::Result` in `main` (`src/main.rs:5`), `cli::run` (`src/cli.rs:123`), `web::serve` (`src/web.rs:470`), `provider` helpers.
- `ImageGenerationResponse { error: Some(...) , ..Default::default() }` for per-image failures inside core/provider (`src/core.rs:19`, `src/provider.rs:134`).
- Direct `std::process::exit(1)` with colored `eprintln!` for CLI usage errors (`src/cli.rs:140`, `src/cli.rs:171`).
- JSON error envelopes `{ "success": false, "error": ... }` from web handlers (`src/web.rs:144`, `src/web.rs:151`).
- Provider error extraction from response bodies: `extract_api_error` (`src/provider.rs:431`), `truncate` for messages (`src/provider.rs:445`).

## Cross-Cutting Concerns

**Logging:** No logging framework. CLI prints via `colored` + `println!`/`eprintln!`; TUI buffers `LogLine { kind, text }` into an on-screen pane (`src/tui.rs:40`); web returns JSON. `--verbose` dumps the request body to stderr (`src/provider.rs:155`).

**Validation:** Minimal and per-frontend — engine existence checked against `Settings` (`src/core.rs:19`, `src/cli.rs:168`, `src/web.rs:151`), empty prompt rejected (`src/cli.rs:165`), filename sanitization in `src/utils.rs:7`, path-traversal guard on `GET /api/images/{filename}` (`src/web.rs:380`).

**Authentication:** No user auth on the web server. Security is entirely outbound: `IMAGAI__ENGINES__<NAME>__API_KEY` bearer tokens (`src/provider.rs:163`), with `PLACEHOLDER_KEY` detection (`src/config.rs:17`). Web exposes CORS `Any` and an unauthenticated shell-exec endpoint.

---

*Architecture analysis: 2026-10-03*
