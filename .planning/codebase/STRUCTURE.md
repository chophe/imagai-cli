---
last_mapped_commit: f37fbe5c1e16138cf0fbfaf77157b2e28b0f2a9c
last_mapped_at: 2026-10-03
---
# Codebase Structure

**Analysis Date:** 2026-10-03

## Directory Layout

```text
imagai-cli/
├── src/                    # All Rust source (single crate, lib + bin)
│   ├── main.rs             # Binary entry: parse args → load settings → dispatch
│   ├── lib.rs              # Module declarations only (10 lines)
│   ├── cli.rs              # clap definitions + Generate/ListEngines handlers (452 L)
│   ├── core.rs             # Shared generation pipeline (130 L)
│   ├── provider.rs         # Outbound HTTP to AI engines (565 L)
│   ├── config.rs           # Settings / EngineConfig from env + .env (197 L)
│   ├── models.rs           # Request/response DTOs (194 L)
│   ├── utils.rs            # Filenames, PNG metadata, image saving (417 L)
│   ├── tui.rs              # ratatui interactive frontend (964 L)
│   └── web.rs              # axum REST server + handlers (477 L)
├── tests/                  # Integration tests (cargo test)
│   ├── common/
│   │   └── mod.rs          # Shared MockServer + fixtures (199 L)
│   ├── cli.rs              # End-to-end binary tests via assert_cmd (357 L)
│   ├── core.rs             # Library tests for generate_image_core (124 L)
│   └── web.rs              # In-process axum tests via tower::oneshot (246 L)
├── generated_images/       # Default image output dir (gitignored, created at runtime)
├── graft/                  # Local code-graph cache (gitignored, regenerable)
├── web_interface.html      # Embedded single-file web SPA, include_str! into web.rs
├── Cargo.toml              # Crate manifest: lib + bin `imagai`
├── Cargo.lock              # Locked dependencies
├── .env.example            # Documented IMAGAI__* variable template
├── .gitignore              # ignores /target, /generated_images, .env, *.env, /graft/
├── README.md               # Install, CLI usage, engine config docs
├── AGENTS.md / GEMINI.md   # Agent instructions (graft graph usage)
├── opencode.json           # OpenCode MCP config (graft server)
└── .mcp.json               # MCP server config
```

## Directory Purposes

**`src/`:**

- Purpose: every module of the single `imagai` crate — no subdirectories, flat module layout.
- Contains: `*.rs` files, each one self-contained concern with its own `#[cfg(test)] mod tests`.
- Key files: `src/lib.rs` (module list), `src/main.rs` (entry), `src/core.rs` (shared pipeline).

**`tests/`:**

- Purpose: integration/E2E tests compiled as separate crates against the public library API and the built binary.
- Contains: `cli.rs`, `core.rs`, `web.rs`, plus `common/mod.rs` shared helpers.
- Key files: `tests/common/mod.rs` (axum-based `MockServer`, `ImagesMode`/`ChatMode` toggles, `PIXEL_PNG_B64` fixture).

**`generated_images/`:**

- Purpose: default `output_dir` where generated PNGs land (`src/config.rs:43`).
- Generated: Yes (runtime).
- Committed: No (`.gitignore` entry `/generated_images`).

**`graft/`:**

- Purpose: local code-graph cache used by the `graft` CLI (`graft/INDEX.md` links nodes to file spans).
- Generated: Yes (`graft build`).
- Committed: No (`.gitignore` entry `/graft/`).

**Repo root files:**

- `web_interface.html`: the entire browser UI, embedded at compile time via `include_str!("../web_interface.html")` in `src/web.rs:18`.
- `.env.example`: documents the `IMAGAI__*` env vars; `.env` itself is gitignored (exists locally — contents not read).

## Key File Locations

**Entry Points:**

- `src/main.rs`: binary entry, `#[tokio::main]`, `Cli::parse()` → `config::Settings::load()` → `cli::run()`.
- `src/cli.rs:123` `run()`: subcommand dispatch (`Generate`, `ListEngines`, `Tui`, `Web`).
- `src/cli.rs:132` `cmd_generate()`: CLI generation flow.
- `src/cli.rs:276` `cmd_list_engines()`: engine/model listing.
- `src/tui.rs:915` `run()`: TUI bootstrap; `src/tui.rs:924` `run_loop()` event loop.
- `src/web.rs:470` `serve()`: binds TCP and starts axum; `src/web.rs:88` `router()` defines routes.

**Configuration:**

- `src/config.rs:24` `Settings` — `output_dir`, `default_engine`, `engines: HashMap<String, EngineConfig>`.
- `src/config.rs:9` `EngineConfig` — `api_key`, `base_url`, `model`.
- `src/config.rs:39` `Settings::load()` — dotenv + `IMAGAI__*` env parsing.
- `Cargo.toml` — dependencies, `[[bin]]`, release profile (`opt-level = 3`, `strip = true`).
- `.env.example` — variable naming reference (copy to `.env`).

**Core Logic:**

- `src/core.rs:12` `generate_image_core()` — the shared pipeline every frontend calls.
- `src/provider.rs:120` `generate_images()` — provider routing + HTTP POST.
- `src/provider.rs:243` `build_images_body()` — per-engine request body construction.
- `src/provider.rs:293` `openrouter_chat_generate()` — chat-based (Gemini/vision) path.
- `src/provider.rs:83` `chat_completion()` — used by LLM filename generation and model chat.
- `src/models.rs:8` `ImageGenerationRequest`, `src/models.rs:44` `ImageGenerationResponse`.
- `src/utils.rs:234` `inject_png_metadata()` — writes prompt/model into PNG `tEXt` chunks.

**Frontends:**

- `src/cli.rs:49` `GenerateArgs` — full CLI surface incl. Stability-specific flags.
- `src/tui.rs:109` `App` — TUI state; `src/tui.rs:267` `build_request()`.
- `src/web.rs:27` `GeneratePayload` / `src/web.rs:67` `CliPayload` — API request bodies.

**Testing:**

- `tests/common/mod.rs:49` `MockServer` — in-process OpenAI-compatible fake.
- `tests/cli.rs:16` `cmd()` — builds `assert_cmd::Command` wired to the mock.
- `tests/web.rs:19` `test_settings()` — `Settings` fixture for axum handlers.
- Unit tests: `mod tests` at the bottom of `src/cli.rs:418`, `src/config.rs:129`, `src/models.rs:125`, `src/provider.rs:454`, `src/utils.rs:276`.

## Naming Conventions

**Files:**

- One module per file, named after the concern, all lowercase snake_case: `core.rs`, `provider.rs`, `config.rs`, `models.rs`, `utils.rs`, `tui.rs`, `web.rs`, `cli.rs`.
- No file matches the struct name — modules are areas, not types.

**Functions:**

- `snake_case`, imperative verbs: `generate_image_core`, `build_images_body`, `save_image_from_b64`.
- Frontend handlers named for their route/action: `generate`, `generate_cli`, `list_engines`, `serve_image`.
- Command handlers prefixed `cmd_`: `cmd_generate`, `cmd_list_engines`.
- Private helpers are unprefixed lowercase: `suffix_numbered`, `resolve_url`, `truncate`.

**Types:**

- `PascalCase` structs: `Settings`, `EngineConfig`, `ImageGenerationRequest`, `GeneratePayload`, `AppState`.
- Response/request DTOs suffixed `Request`/`Response`/`Payload`.
- Enums: `Commands` (clap), `LogKind`, `ImagesMode`, `ChatMode`.

**Constants:**

- `SCREAMING_SNAKE_CASE`: `PLACEHOLDER_KEY`, `WEB_INTERFACE_HTML`, `ROW_PROMPT`, `IMAGE_EXTENSIONS`, `PIXEL_PNG_B64`.

**Env vars:**

- Double-underscore namespaced: `IMAGAI__OUTPUT_DIR`, `IMAGAI__DEFAULT_ENGINE`, `IMAGAI__ENGINES__<ENGINE_NAME>__API_KEY` (engine name upper-cased in env, lower-cased on load at `src/config.rs:63`).

**Test names:**

- Descriptive snake_case sentences stating the assertion: `unknown_engine_returns_error_response` (`tests/core.rs:29`), `stability_body_merges_extra_params` (`src/provider.rs:509`), `placeholder_key_is_not_set` (`src/config.rs:177`).

**Docs/comments:**

- Module-level `//!` doc on `src/lib.rs` only; `///` doc comments on nearly every public item.

## Where to Add New Code

**New generation feature or flag:**

- CLI flag: `GenerateArgs` in `src/cli.rs:49` + map into `extra_params` in `cmd_generate` (`src/cli.rs:187`).
- Web field: `GeneratePayload` in `src/web.rs:27` + mapping in `generate` (`src/web.rs:152`).
- TUI row: add `ROW_*` constant (`src/tui.rs:17`), bump `ROWS`, update `text_field`/`bool_field`/`row_label` and `draw_form` (`src/tui.rs:509`).
- Provider body: `build_images_body` in `src/provider.rs:243`.
- Shared field on the request: `ImageGenerationRequest` in `src/models.rs:8`.

**New engine/provider family:**

- Detection + routing in `src/provider.rs:120`; URL resolution in `resolve_url` (`src/provider.rs:71`).
- Config comes free via `IMAGAI__ENGINES__<NAME>__*` env vars — no config code needed.
- Add a mock mode in `tests/common/mod.rs` (`ImagesMode`/`ChatMode`).

**New HTTP endpoint:**

- Add route in `src/web.rs:88` `router()`, handler function in `src/web.rs`.
- Tests: in-process via `tower::oneshot` helpers `get`/`post_json` in `tests/web.rs:38`.

**New TUI tab:**

- `TABS` const (`src/tui.rs:30`), a `draw_<name>_tab` function, wire into `draw` (`src/tui.rs:457`).

**Utilities:**

- Shared pure helpers go in `src/utils.rs`; keep them free of provider branching.

**Tests:**

- Library behavior → unit `mod tests` at the bottom of the owning `src/*.rs` file.
- Cross-module/HTTP/binary behavior → new `tests/<area>.rs`, sharing `tests/common/mod.rs`.

**Do not create:** `src/` subdirectories or a `modules/` tree — the crate deliberately uses a flat sibling-module layout declared in `src/lib.rs`. Do not add a logging framework or config file parser without a phase that explicitly asks for one.

## Special Directories

**`generated_images/`:**

- Purpose: runtime image output (default `output_dir`).
- Generated: Yes.
- Committed: No.

**`graft/`:**

- Purpose: regenerable code-graph nodes (`graft/INDEX.md`, linked markdown with file:line spans).
- Generated: Yes (`graft build`).
- Committed: No (ignored in `.gitignore`).

**`target/` (implied):**

- Purpose: Cargo build artifacts.
- Generated: Yes.
- Committed: No.

**`.planning/`:**

- Purpose: GSD planning artifacts, including `.planning/codebase/` docs.
- Generated: Yes.
- Committed: Per project policy.

**`.env` (present, gitignored):**

- Purpose: local API keys and engine config. Contents not read during analysis.

---

*Structure analysis: 2026-10-03*
