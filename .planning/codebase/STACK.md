---
last_mapped_commit: f37fbe5c1e16138cf0fbfaf77157b2e28b0f2a9c
last_mapped_at: 2026-10-03
---
# Technology Stack

**Analysis Date:** 2026-10-03

## Languages

**Primary:**

- Rust (edition 2021) - All application code: `src/*.rs` (~3,417 lines across 10 modules)
  - `src/main.rs` (11 lines, binary entry) → `src/cli.rs` (452) → `src/core.rs` (130) → `src/provider.rs` (565)
  - UI surfaces: `src/tui.rs` (964), `src/web.rs` (477)
  - Support: `src/config.rs` (197), `src/models.rs` (194), `src/utils.rs` (417), `src/lib.rs` (module barrel)

**Secondary:**

- HTML + inline vanilla JavaScript/CSS - `web_interface.html` (~33 KB), embedded into the binary via `include_str!` at `src/web.rs:17`. No build step, no bundler, no npm.
- Markdown - project instructions (`AGENTS.md`, `GEMINI.md`, `README.md`), code-graph nodes under `graft/`.

## Runtime

**Environment:**

- Rust toolchain via `rustup` — no `rust-toolchain.toml` / `rust-toolchain` pin, no `.nvmrc` equivalent; edition 2021 is the only constraint declared in `Cargo.toml`.
- Async runtime: `tokio` 1.53.1, multi-threaded, started with `#[tokio::main]` in `src/main.rs:7`.

**Package Manager:**

- `cargo`
- Lockfile: `Cargo.lock` present and committed (352 resolved crates)
- No `Makefile`, `justfile`, or script wrappers — commands are plain `cargo` invocations.

## Frameworks

**Core:**

- `clap` 4.6.6 (`derive`, `env` features) - CLI parsing and self-documenting help; `src/cli.rs:11-121`
- `axum` 0.8.9 + `tower-http` 0.6.11 (`cors`) - Embedded REST server for `imagai web`; router built at `src/web.rs:88-105`, listener at `src/web.rs:470-476`
- `ratatui` 0.30.2 + `crossterm` 0.29 - Full-screen interactive TUI (`imagai tui`); `src/tui.rs`
- `reqwest` 0.13.1 (`default-features = false`, `json`, `rustls`, `webpki-roots`) - All outbound HTTP; TLS is rustls with webpki roots (no OpenSSL / no system-native TLS)

**Testing:**

- `cargo test` with three integration-test binaries + inline `#[cfg(test)]` unit tests
- `assert_cmd` 2 - drives the real compiled `imagai` binary (`tests/cli.rs:17`)
- `predicates` 3 - output assertions
- `tempfile` 3 - isolated output dirs / env sandboxes
- `tower` 0.5 (`util`) - test helpers for the in-process mock server
- Mock OpenAI-compatible server: `tests/common/mod.rs` (axum on `127.0.0.1:0`)

**Build/Dev:**

- `cargo build --release` with `[profile.release] opt-level = 3, strip = true` (`Cargo.toml`)
- No CI (`.github/` absent), no `rustfmt.toml`, no `clippy.toml`, no lint config — formatting/linting are tool defaults.
- Repo code-graph tooling: `graft` MCP server wired in `.mcp.json` and `opencode.json`; graph nodes in `graft/` (gitignored cache per `.gitignore`).

## Key Dependencies

**Critical:**

- `reqwest` 0.13.1 - every AI API call and image download; configured once in `provider::http_client()` (`src/provider.rs:12-17`, User-Agent `imagai/<version>`)
- `serde` 1.0.229 / `serde_json` - request/response modelling in `src/models.rs`, hand-built bodies via `json!()` in `src/provider.rs:243-291`
- `axum` 0.8.9 - the entire web surface (`src/web.rs`)
- `clap` 4.6.6 - the entire CLI surface (`src/cli.rs`)
- `anyhow` 1 - error propagation everywhere (no custom error enum)

**Infrastructure (local, no external services):**

- `dotenvy` 0.15 - loads `.env` from cwd or repo root (walks up looking for `.git`), `src/config.rs:46-55, 116-126`
- `colored` 3 - terminal styling for CLI/TUI messages (`src/cli.rs`, `src/tui.rs`)
- `base64` 0.22 - decodes `b64_json` image payloads (`src/utils.rs:194-206`), encodes previews for the web UI (`src/web.rs`)
- `chrono` 0.4 + `uuid` 1 (`v4`) - filename generation (`src/utils.rs:51-59`)
- `crc32fast` 1 - hand-rolled PNG `tEXt` chunk CRC when injecting prompt/model metadata (`src/utils.rs:234-265`); no `image`/`png` crate is used

## Configuration

**Environment:**

- Flat `IMAGAI__*` namespace read directly from `std::env::vars()` (no config crate, no TOML/YAML/JSON config file) — parsing in `src/config.rs:57-86`
- `.env` file supported (loaded through `dotenvy`); `.env.example` exists as a template — its contents are not reproduced here
- Required variables:
  - `IMAGAI__OUTPUT_DIR` (default `generated_images`)
  - `IMAGAI__DEFAULT_ENGINE`
  - `IMAGAI__ENGINES__<NAME>__API_KEY`, `IMAGAI__ENGINES__<NAME>__BASE_URL`, `IMAGAI__ENGINES__<NAME>__MODEL`
- Optional: `OPENROUTER_HTTP_REFERER`, `OPENROUTER_X_TITLE` (OpenRouter ranking headers, `src/provider.rs:303-308`)
- Undeclared keys are ignored; engines with `YOUR_OPENAI_API_KEY` are treated as unconfigured (`PLACEHOLDER_KEY`, `src/config.rs:5, 17-19`)

**Build:**

- `Cargo.toml` (manifest + release profile), `Cargo.lock` (pinned versions)
- No `.cargo/config.toml`, no build scripts (`build.rs` absent)

**Editor/agent tooling (not runtime):**

- `.mcp.json`, `opencode.json` (graft MCP), `.claude/`, `.cursor/`, `.gemini/`, `.grok/`, `.kiro/`, `.windsurf/`, `.adal/` — agent configuration only, zero effect on the shipped binary.

## Platform Requirements

**Development:**

- Rust toolchain (edition 2021), `cargo`
- Network egress to AI provider endpoints for real generation; tests need none (mock server binds `127.0.0.1:0`)
- Tests mutate process-wide env vars and are serialized behind `ENV_LOCK` (`src/config.rs:134`)

**Production:**

- Single native binary `target/release/imagai` (stripped, `opt-level = 3`)
- Web mode binds `0.0.0.0:5000` by default (`src/cli.rs:39-47`), serves from the local filesystem
- Writes generated images to `IMAGAI__OUTPUT_DIR` (default `./generated_images`)
- No database, no container, no service manager, no cloud runtime required

---

*Stack analysis: 2026-10-03*
