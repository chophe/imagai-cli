---
last_mapped_commit: f37fbe5c1e16138cf0fbfaf77157b2e28b0f2a9c
last_mapped_at: 2026-10-03
---
# Coding Conventions

**Analysis Date:** 2026-10-03

Rust project (`imagai`, edition 2021). No `.eslintrc`/formatter config exists — conventions below are derived from the actual code in `src/` and `tests/`.

## Naming Patterns

**Files:**

- All-lowercase single-word module files, one module per concern: `src/cli.rs`, `src/config.rs`, `src/core.rs`, `src/models.rs`, `src/provider.rs`, `src/tui.rs`, `src/utils.rs`, `src/web.rs`
- Library root `src/lib.rs` re-exports every module as `pub mod` (flat, no nesting beyond one level)
- Binary entry `src/main.rs` is minimal (~11 lines): parse CLI → load settings → delegate to `imagai::cli::run`

**Functions:**

- `snake_case` throughout; no exceptions found
- Command handlers prefixed `cmd_` in `src/cli.rs` (`cmd_generate`, `cmd_list_engines`)
- Async handlers named after their route in `src/web.rs` (`index`, `list_engines`, `generate`, `generate_cli`, `list_images`, `serve_image`)
- Predicates return `bool` and read as questions: `key_set()` (`src/config.rs:17`), `is_image_model()` (`src/cli.rs:397`)
- TUI draw helpers prefixed `draw_` (`draw_header`, `draw_form`, `draw_logs` in `src/tui.rs`)

**Types:**

- `PascalCase` structs/enums; request/response DTOs end in `Request`/`Response` (`ImageGenerationRequest`, `ImageGenerationResponse`, `ImagesResponse`, `ChatResponse` in `src/models.rs`)
- CLI arg structs end in `Args` (`GenerateArgs`, `WebArgs` in `src/cli.rs`)
- Mock test types are prefixed with `Mock`/named for behavior: `MockServer`, `ImagesMode`, `ChatMode` (`tests/common/mod.rs`)
- Field-level enums for test behavior use `B64`/`Url`/`Error` variants (`tests/common/mod.rs:20`)

**Constants:**

- `SCREAMING_SNAKE_CASE`: `PLACEHOLDER_KEY` (`src/config.rs:5`), `WEB_INTERFACE_HTML` (`src/web.rs:16`), `ROWS`/`ROW_PROMPT` (`src/tui.rs:17`)
- Test fixtures as `const`: `PIXEL_PNG_B64` appears both in `src/utils.rs:280` and `tests/common/mod.rs:16`

**Variables:**

- Short locals in tight scopes are fine (`req`, `cfg`, `out`, `f`, `v`) — see `src/provider.rs` and `src/core.rs`
- Env-var-parsed names lowercased before use: `engine_name.to_lowercase()` (`src/config.rs:62`)

## Code Style

**Formatting:**

- `cargo fmt --check` passes clean (verified 2026-10-03) — **default rustfmt settings, no `rustfmt.toml`**. Run `cargo fmt` before committing.
- Max width default (100); line breaks match rustfmt's choices exactly.

**Linting:**

- No `clippy.toml`, no `#![deny]`/`#![warn]` attributes in source. `cargo clippy --all-targets` compiles clean (verified 2026-10-03) — treat clippy-clean as the baseline.
- Only one allow-attribute in the tree: `#![allow(dead_code)]` in `tests/common/mod.rs:5` (shared helpers not used by every test binary). Do not add allows in `src/`.

**Attribute style:**

- `#[derive(...)]` on every public struct; serde derives only where parsing happens (`#[derive(Deserialize)]` in `src/models.rs`, `src/web.rs`)
- `#[serde(default)]` on every optional API field so unknown/missing JSON never fails (`src/models.rs:59-121`) — follow this for any new provider field.

## Import Organization

**Order (observed consistently, rustfmt-grouped with blank lines between groups):**

1. `std::…`
2. External crates (`clap`, `axum`, `ratatui`, `serde_json`, `colored`, …)
3. `crate::…` (in `src/`) or `common::…` + `imagai::…` (in `tests/`)

Examples: `src/web.rs:1-18`, `src/tui.rs:1-15`, `tests/web.rs:3-18`.

**Path Aliases:**

- None (no `Cargo.toml` `[patch]`/workspace; single crate). Tests refer to the library as `imagai::…` (`tests/core.rs:10-12`).
- Axum's `Path` is renamed on import to avoid colliding with `std::path::Path`: `use axum::extract::{Path as AxumPath, State};` (`src/web.rs:5`) — reuse this alias when both are needed.

## Error Handling

**Strategy:** `anyhow::Result` for anything that propagates to a boundary (CLI/TUI/web/provider). Structured `Option<String>` error fields for per-item results that must not abort a batch.

**Patterns:**

- Fallible top-level fns return `anyhow::Result<T>`: `main` (`src/main.rs:7`), `cli::run` (`src/cli.rs:123`), `web::serve` (`src/web.rs:470`), `provider::http_client` (`src/provider.rs:12`)
- Fail fast with `anyhow::bail!` carrying status + truncated body: `src/provider.rs:30`, `src/utils.rs:186`
- Wrap with `map_err(|e| anyhow::anyhow!("failed to build HTTP client: {e}"))` (`src/provider.rs:16`)
- Batch operations return `Vec<ImageGenerationResponse>` where each item carries `error: Option<String>` — errors are collected, never panics: `src/core.rs:12-119`
- User-facing CLI errors print a colored `"[Error]"` tag to stderr then `std::process::exit(1)` — `src/cli.rs:139-144`, `src/cli.rs:173`, `src/cli.rs:184`. **Exit code 1 is asserted by tests** (`tests/cli.rs:328,342,356`).
- Non-fatal problems print `"[warning]"`/`"Warning:"` and continue (`src/config.rs:90`, `src/cli.rs:278`)
- `unwrap()`/`expect()` in `src/` appear **only inside `#[cfg(test)]` blocks** (plus two `.ok()`-style lazy paths). Do not introduce `unwrap()` on I/O or network results in production code paths.

## Logging

**Framework:** none (`log`/`tracing` not used). Output goes through `println!`/`eprintln!` (41 call sites in `src/`).

**Patterns:**

- Decorated tags via `colored::Colorize`: `"[Error]".bold().red()`, `"[warning]".bold().yellow()`, `"[Prompt]".bold().yellow()` (`src/cli.rs:173`, `src/cli.rs:278`)
- `--verbose` gates debug dumps of request bodies: `eprintln!("--- API Request Body ---")` in `src/provider.rs:158-163`, `src/provider.rs:329-334`, `src/utils.rs:131`
- TUI logs are in-memory lines, not stdout: `push_log`/`LogKind` (`src/tui.rs:33-40,168`)
- When adding user-visible output, use an emoji + colored tag prefix (`"🖼️"`, `"✅"`, `"⚪"`) matching `src/cli.rs:187-270`. Tests assert on substrings like `"generated successfully"` (`tests/cli.rs:128`), so keep those phrases stable.

## Comments

**When to Comment:**

- Module-level `//!` doc on `src/lib.rs:1` and on every integration test file (`tests/cli.rs:1`, `tests/core.rs:1`, `tests/web.rs:1`, `tests/common/mod.rs:1`)
- `///` doc comment on every `pub fn`/`pub struct`/`pub const` and on non-obvious private helpers (`src/config.rs:16`, `src/utils.rs:6-57`, `src/provider.rs:11-20`)
- Section separators as `// ---- name` rules in test files: `// ---------------------------------------------------------------- generate` (`tests/cli.rs:46,84,116,316`)
- Inline `//` explains *why*, especially around provider-specific quirks: "n / response_format dropped for stability" (`src/provider.rs:527`), "The URL contains `openrouter.ai` … to trigger the chat-completions branch" (`tests/core.rs:47`)
- Doc comments on test-only helpers that explain intent: `/// Tests that mutate process-wide env vars must be serialized.` (`src/config.rs:133`)

**JSDoc/TSDoc:** Not applicable (Rust). Use rustdoc `///`; `cargo doc` style with `{e}` inline format-args in doc text is fine.

## Function Design

**Size:**

- Small pure helpers preferred: `suffix_numbered` (`src/core.rs:122`), `png_text_chunk` (`src/utils.rs:255`), `truncate` (`src/provider.rs`), `extract_text_content` (`src/provider.rs:391`)
- One large function per view is tolerated in the TUI (`draw_form` ~90 lines in `src/tui.rs:509`) — TUI draw code is the exception, not the model to copy elsewhere.

**Parameters:**

- Context passed as `&Settings` and `&EngineConfig` rather than globals: `generate_image_core(&request, &settings)` (`src/core.rs:12`), `provider::generate_images(request, engine_config)` (`src/core.rs:27`)
- Options use `Option<T>` + `or_else`/`let … else` chains: engine resolution at `src/cli.rs:134-145`, response parsing at `src/provider.rs:366`
- Builder-ish request structs support `..Default::default()` in tests (`tests/core.rs:27-31`) — keep `impl Default` on `ImageGenerationRequest` (`src/models.rs:23`)

**Return Values:**

- Fallible → `anyhow::Result<T>`; partial failure → `Vec<…>` with per-item `error` field; lookup → `Option<&T>` (`Settings::get_engine`, `src/config.rs:104`)
- Never return `Result` from a TUI draw helper; report through `App::push_log` instead (`src/tui.rs:168`)

## Module Design

**Exports:**

- Flat: `src/lib.rs` is only `pub mod` lines. Reach code as `imagai::core::generate_image_core`, `imagai::config::Settings`.
- `pub` on API surface, private (`fn`) for everything else — e.g. `pub fn http_client` vs private `fn truncate` in `src/provider.rs`
- Web router factory is `pub fn router(settings: Settings) -> Router` (`src/web.rs:96`) so tests can build it in-process without binding a port

**Barrel Files:** None beyond `src/lib.rs`. Test helpers live in `tests/common/mod.rs` and are pulled in with `mod common;` at the top of each integration test file (`tests/cli.rs:4`, `tests/core.rs:3`, `tests/web.rs:3`).

**Adding new code — quick rules:**

- New provider logic → `src/provider.rs`; request/response DTOs → `src/models.rs`
- New filename/IO/PNG helpers → `src/utils.rs` (unit tests go in the `mod tests` at the bottom)
- New CLI flag → `GenerateArgs` in `src/cli.rs` + wire into `ImageGenerationRequest`; new REST field → `GeneratePayload` in `src/web.rs` with `#[serde(default)]`

---

*Convention analysis: 2026-10-03*
