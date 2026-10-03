---
last_mapped_commit: f37fbe5c1e16138cf0fbfaf77157b2e28b0f2a9c
last_mapped_at: 2026-10-03
---
# Testing Patterns

**Analysis Date:** 2026-10-03

## Test Framework

**Runner:**

- Built-in Rust test harness (`cargo test`), Rust edition 2021
- No separate config file — no `[[test]]` entries, no test profile overrides in `Cargo.toml`; integration tests are auto-discovered from `tests/*.rs`
- Dev-dependencies (`Cargo.toml` `[dev-dependencies]`):
  - `assert_cmd = "2"` — spawn the compiled `imagai` binary
  - `predicates = "3"` — stdout/stderr assertions
  - `tempfile = "3"` — isolated output dirs
  - `tower = { version = "0.5", features = ["util"] }` — in-process HTTP via `oneshot`

**Assertion Library:**

- std `assert!` / `assert_eq!` / `assert_ne!` with a trailing message string, always with the failing value interpolated:
  ```rust
  assert_eq!(parts.len(), 4, "parts: {parts:?}");        // src/utils.rs:325
  assert!(err.contains("not configured"), "{err}");      // tests/core.rs:36
  ```
- `predicates::prelude::*` for binary stdout/stderr: `.stdout(predicate::str::contains("generate"))` (`tests/cli.rs:55`)

**Run Commands:**

```bash
cargo test                          # everything (51 tests: 22 unit + 29 integration)
cargo test --lib                    # unit tests in src/ only
cargo test --test cli               # end-to-end binary tests (tests/cli.rs, 16)
cargo test --test core              # generation pipeline (tests/core.rs, 3)
cargo test --test web               # REST API in-process (tests/web.rs, 10)
cargo test <filter>                 # e.g. cargo test sanitize_replaces
cargo fmt --check && cargo clippy --all-targets   # lint gate (both clean as of 2026-10-03)
```

Documented in `README.md:110-122`. All 51 tests pass as of 2026-10-03.

## Test File Organization

**Location:**

- Unit tests: `#[cfg(test)] mod tests { … }` at the **bottom of each source file**, `use super::*;` first
  - `src/utils.rs:275`, `src/models.rs:124`, `src/provider.rs:453`, `src/config.rs:128`, `src/cli.rs:417`
- Integration tests: top-level `tests/` directory, one file per layer (binary / library / web)
- Shared helpers: `tests/common/mod.rs` — included with `mod common;` in each test file

**Naming:**

- Test fn names are snake_case descriptive sentences of the behavior, no `test_` prefix:
  `generate_saves_b64_image_with_metadata`, `serve_image_blocks_traversal`, `stability_body_merges_extra_params`
- Integration files mirror source modules: `tests/cli.rs` ↔ `src/cli.rs`, `tests/core.rs` ↔ `src/core.rs`, `tests/web.rs` ↔ `src/web.rs`

**Structure:**

```
tests/
├── common/
│   └── mod.rs     # MockServer + shared fixtures (199 lines)
├── cli.rs         # e2e: spawns real binary (357 lines)
├── core.rs        # library-level generate_image_core (124 lines)
└── web.rs         # axum routes in-process (246 lines)
```

## Test Structure

**Suite Organization:**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    // optional: helper builders (fn request_with_extra(...), fn dalle_config())
    // optional: const fixtures (const PIXEL_PNG_B64: &str = "…")

    #[test]
    fn dalle3_body_has_quality_and_style() { … }   // src/provider.rs:477
}
```

Integration files open with a module doc comment, `mod common;`, then local builder helpers:

```rust
//! End-to-end tests that run the compiled `imagai` binary against a mock
//! OpenAI-compatible server.
mod common;                                   // tests/cli.rs:1-4

fn cmd(mock: &MockServer, out_dir: &Path) -> Command { … }   // tests/cli.rs:16
fn base_settings(out_dir: &Path) -> Settings { … }           // tests/core.rs:14
fn test_settings(mock: &MockServer, out_dir: &Path) -> Settings { … }  // tests/web.rs:17
```

**Patterns:**

- **Setup:** each async test starts its own `MockServer::start().await` + `TempDir::new().unwrap()` — no shared/global state between tests
- **Teardown:** automatic via `TempDir` Drop and mock server living in the test task
- **Grouping:** comment rules separate concern blocks in `tests/cli.rs` (`// ---- basics`, `// ---- engines`, `// ---- generate`, `// ---- errors` at lines 46, 84, 116, 316)
- **Async:** always `#[tokio::test(flavor = "multi_thread")]` for integration tests (needed because the mock server uses `tokio::spawn`); plain `#[test]` for unit tests
- **Ordering hazard:** tests mutating process env vars serialize on an explicit mutex — `static ENV_LOCK: Mutex<()> = Mutex::new(());` with `let _guard = ENV_LOCK.lock().unwrap();` at the top and `clear_imagai_env()` before/after (`src/config.rs:133-196`). Copy this pattern for any new env-dependent test.

## Mocking

**Framework:** none (no `mockall`/`wiremock`). Mocking is a **hand-rolled in-process axum server**: `tests/common/mod.rs`.

**Patterns:**

```rust
// tests/common/mod.rs:49-104
pub struct MockServer { pub addr: String, pub state: MockState }

// Behavior switched per test via enums:
mock.set_images_mode(ImagesMode::B64 { count: 2 });
mock.set_images_mode(ImagesMode::Error { status: 400, message: "bad words in prompt".into() });
mock.set_chat_mode(ChatMode::Text { content: "sunset_over_the_hills".to_string() });

// Request capture for assertions:
let bodies = mock.requests_for("/v1/images/generations");
assert_eq!(bodies[0]["prompt"], "a red cat");           // tests/cli.rs:140-142
```

- Binds `127.0.0.1:0` (ephemeral port); routes matched by **path substring** so tests can embed markers like `openrouter.ai` in the URL to trigger provider branches (`tests/cli.rs:301-307`)
- Captured `(path, json_body)` pairs in `Arc<Mutex<Vec<…>>>` (`tests/common/mod.rs:43`)
- Serves a 1×1 PNG from the shared `PIXEL_PNG_B64` fixture (`tests/common/mod.rs:16,193`)

**What to Mock:**

- External HTTP boundaries — always point the engine `base_url` at `mock.base_url()`, never the real OpenAI/OpenRouter/Stability endpoints
- `IMAGAI__ENGINES__MOCK__*` env vars configure the binary under test (`tests/cli.rs:16-24`)

**What NOT to Mock:**

- The `imagai` binary itself — e2e tests use `assert_cmd::Command::cargo_bin("imagai")` against the real compiled artifact
- Filesystem — use `TempDir` and assert real files exist (`tests/cli.rs:131`, `tests/core.rs:82-86`)
- The axum router — build it directly with `web::router(test_settings(...))` instead of stubbing handlers (`tests/web.rs:76`)

## Fixtures and Factories

**Test Data:**

```rust
// 1×1 transparent PNG, reused everywhere:
const PIXEL_PNG_B64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";
// src/utils.rs:280 and tests/common/mod.rs:16

// Inline JSON response fixtures parsed in-place:
let json = r#"{"data": [{"b64_json": "QUJD"}], "usage": {"total_tokens": 10}}"#;
let parsed: ImagesResponse = serde_json::from_str(json).unwrap();   // src/models.rs:130-135

// Factories for request/config construction:
fn request_with_extra(extra: HashMap<String, Value>) -> ImageGenerationRequest { … }  // src/provider.rs:457
fn dalle_config() -> EngineConfig { … }                                               // src/provider.rs:471
fn base_settings(out_dir: &Path) -> Settings { … }                                    // tests/core.rs:14
```

**Location:**

- Unit-test fixtures: consts/fns inside the `#[cfg(test)] mod tests` block
- Cross-file fixtures: `tests/common/mod.rs` (exported `PIXEL_PNG_B64`, `MockServer`, `ImagesMode`, `ChatMode`); the module carries `#![allow(dead_code)]` because not every test binary uses every helper

## Coverage

**Requirements:** None enforced — no `cargo-tarpaulin`/`llvm-cov` config, no coverage CI (`.github/` does not exist).

**View Coverage:**

```bash
cargo llvm-cov --all-targets    # not installed/configured; install if needed
```

De-facto coverage today: every module with logic has unit tests except `src/tui.rs` (0 tests) and `src/web.rs` (covered indirectly through `tests/web.rs`).

## Test Types

**Unit Tests:**

- Pure logic, no I/O: request-body construction (`src/provider.rs:477-550`), serde parsing (`src/models.rs:128-193`), filename sanitization/PNG chunk building (`src/utils.rs:288-416`), model-id heuristic (`src/cli.rs:420`)
- Env-dependent config loading serialized via `ENV_LOCK` (`src/config.rs:146-196`)

**Integration Tests:**

- `tests/cli.rs` — spawn the real binary with `assert_cmd`, assert exit codes (`.failure().code(1)`), stdout/stderr substrings, and files written to a `TempDir`; also inspects request bodies captured by the mock
- `tests/core.rs` — call `imagai::core::generate_image_core` directly with hand-built `Settings` (library API, no process spawn)
- `tests/web.rs` — in-process HTTP via `tower::ServiceExt::oneshot`, helpers `get(app, uri)` and `post_json(app, uri, payload)` returning `(StatusCode, Value)` (`tests/web.rs:32-70`)

**E2E Tests:**

- The whole CLI suite *is* the E2E layer: real binary + real HTTP (to mock) + real disk. No browser/E2E tooling for `web_interface.html`.

## Common Patterns

**Async Testing:**

```rust
#[tokio::test(flavor = "multi_thread")]
async fn generate_downloads_url_images() {
    let mock = MockServer::start().await;
    mock.set_images_mode(ImagesMode::Url { count: 1 });
    let out = TempDir::new().unwrap();
    cmd(&mock, out.path())
        .args(["generate", "-p", "url mode"])
        .assert()
        .success()
        .stdout(predicate::str::contains("generated successfully"));
}
```

(`tests/cli.rs:150-166`)

**Error Testing:**

```rust
// CLI: exit code + stderr message
cmd(&mock, out.path())
    .args(["generate", "-p", ""])
    .assert()
    .failure()
    .code(1)
    .stderr(predicate::str::contains("Prompt cannot be empty"));   // tests/cli.rs:350-356

// Library: error surfaced in the response DTO, not as a Result
let results = generate_image_core(&request, &settings).await;
assert!(results[0].error.as_deref().unwrap().contains("not configured"));  // tests/core.rs:35-37

// HTTP: status + JSON payload
assert_eq!(value["success"], false);
assert!(value["error"].as_str().unwrap().contains("not configured"));      // tests/web.rs:137-139
```

**Security assertion example** — keep this style for path handling:

```rust
let (status, _) = get(app, "/api/images/..%2F..%2F.env").await;
assert_eq!(status, StatusCode::NOT_FOUND);       // tests/web.rs:213-217
```

**Adding tests — quick rules:**

- Pure helper → unit test in the same `src/*.rs` file's `mod tests`
- New CLI behavior → `tests/cli.rs`, reuse `fn cmd(&mock, out_dir)`
- New REST endpoint → `tests/web.rs`, reuse `get`/`post_json` + `test_settings`
- New provider request shape → unit test in `src/provider.rs` via `build_images_body`, e2e confirmation in `tests/cli.rs` asserting the mock's captured body

---

*Testing analysis: 2026-10-03*
