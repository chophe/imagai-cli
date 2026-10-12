---
phase: 03-multi-vendor-providers
verified: 2026-10-12T00:00:00.000Z
status: passed
score: 34/34 must-haves verified
covered_files:
  - ".env.example"
  - "src/cli.rs"
  - "src/config.rs"
  - "src/formats.rs"
  - "src/models.rs"
  - "src/provider.rs"
  - "src/tui.rs"
  - "src/web.rs"
  - "web_interface.html"
  - "tests/cli.rs"
  - "tests/common/mod.rs"
  - "tests/core.rs"
  - "tests/web.rs"
covered_digest: "v3:sha256:f39ddf5f978828bb3eaf1e4b76e9828e904229585301d2476c27a2230403b323"
behavior_unverified: 0
overrides_applied: 0
human_verification: []
---

# Phase 3: Multi-Vendor Providers Verification Report

**Phase Goal:** Users can configure named non-OpenAI-compatible providers alongside
existing engines and select them per request
**Verified:** 2026-10-11T06:21:21Z (re-verification — no prior VERIFICATION.md existed)
**Status:** passed
**Head verified:** `ffddab86716cddb401268b05c5521b1e4646c0dd`

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | SC-1 — a user can configure a named non-OpenAI-compatible provider adapter (own request/response shape) next to existing OpenAI-compatible engines | ✓ VERIFIED | `src/formats.rs:17-24` (`ProviderFormat { OpenAiCompatible, Fal }`), `src/config.rs:13-15` (`EngineConfig.format`), `src/provider.rs:639-737` (`fal_generate`), `src/models.rs` `FalImagesResponse`/`FalImage` with no OpenAI key |
| 2 | SC-2 — user selects the provider per request, request built with that provider's wire format, no OpenAI body leaking in | ✓ VERIFIED | `src/provider.rs:251-283` (resolve → fal branch only for `ProviderFormat::Fal`); runtime capture: fal body keys `["image_size","num_images","output_format","prompt"]`, OpenAI body keys `["model","n","prompt","quality","response_format","size","style"]`, intersection `{prompt}` |
| 3 | SC-3 — `imagai list-engines` shows the new provider and existing OpenAI-compatible engines keep working unchanged | ✓ VERIFIED | Runtime table (ANSI-stripped): `Engine Name / Format / Default Model / Base URL / API Key Set` with rows `dalle … openai … N/A (Official OpenAI)` and `fal_flux … fal … https://fal.run`; key-set assertion `openai_default_engine_body_is_unchanged` (tests/cli.rs:247) and `stability_default_engine_body_is_unchanged` (tests/cli.rs:295) both pass |
| 4 | SC-4 — new provider selectable from all three surfaces (CLI flag, TUI, web engine picker) | ✓ VERIFIED | CLI: `tests/cli.rs:355 fal_engine_generates_from_the_cli` + runtime run. Web: `tests/web.rs:954 fal_engine_generates_through_the_web_api` + runtime `POST /api/generate`. TUI: pty-drive — `Generating 1 image(s) with engine 'fal'…`, one PNG saved, capture server logged `POST /fal-ai/flux/schnell` with `Authorization: Key fal-secret-key-123` |
| 5 | 03-01 — `IMAGAI__ENGINES__<NAME>__FORMAT` declared value is one of a closed set (`openai`, `fal`) (D-24) | ✓ VERIFIED | `src/formats.rs:29` (`FORMAT_NAMES`), `src/formats.rs:46-52` (`from_declared`), `src/config.rs:96-104` (parse arm) |
| 6 | 03-01 — an engine with no declared FORMAT behaves exactly as today (D-28) | ✓ VERIFIED | `src/formats.rs:122` (`Ok(Self::OpenAiCompatible)` default); runtime: undeclared engine's body keys unchanged (check 7 below); `tests/cli.rs:247` passes |
| 7 | 03-01 — a declared FORMAT beats a sniffed inference (D-29) | ✓ VERIFIED | `src/formats.rs:112-123` (`resolve` returns the declaration first); `src/formats.rs:245-251` unit test pins an OpenAI base_url + `dall-e-3` model still resolving `Fal` |
| 8 | 03-01 — edit/fork against a format that cannot take image input fails before any socket, naming engine and model (D-26, D-30, D-05) | ✓ VERIFIED | `src/provider.rs:172` (resolve) → `:178-185` (`supports_image_input` gate) before `http_client()` at `:38`; runtime: `imagai edit --engine fal` → `Engine 'fal' (model 'fal-ai/flux/schnell') cannot accept image input for edits: the 'fal' format is generation-only…`, rc=1; `tests/core.rs:540` asserts zero mock requests |
| 9 | 03-01 — `edit_transport` still routes gpt-image to multipart edits and OpenRouter to chat vision (D-28, D-30) | ✓ VERIFIED | `src/provider.rs:191-202`; `tests/core.rs:447 declared_openai_format_keeps_the_sniffed_transports` and `declared_openai_edit_engine_still_saves_with_lineage` both pass |
| 10 | 03-01 — an unrecognized FORMAT fails fast naming the engine and the supported names (D-24, D-05) | ✓ VERIFIED | `src/formats.rs:116-120`; runtime `[warning] Engine 'typo' declares an unknown FORMAT 'falai'; supported formats: openai, fal. This engine fails on its first request.`; `tests/cli.rs:385 unknown_format_fails_the_cli_run` asserts zero requests |
| 11 | 03-02 — fal request POSTed to `{base_url}/{model}` (model is a URL path), `Authorization: Key <key>`, only shared OpenAI key is `prompt` (D-36) | ✓ VERIFIED | `src/provider.rs:646` (`resolve_url(&fal_base_url(config), model)`), `:663-666` (explicit `Key` header); capture: `POST /fal-ai/flux/schnell`, `Host: 127.0.0.1:18711`, `Authorization: Key fal-secret-key-123`, body keys `image_size/num_images/output_format/prompt`; `tests/core.rs:656-657` asserts `vec!["Key test"]` |
| 12 | 03-02 — the fal response maps onto the existing `ImageGenerationResponse` (D-27) | ✓ VERIFIED | `src/provider.rs:701-736` → `image_url`/`image_b64_json`; PNG saved through unchanged pipeline (`tests/core.rs:588 fal_engine_saves_image_through_the_shared_pipeline`) |
| 13 | 03-02 — an unparseable fal response fails naming engine, status and body, never panics (D-35) | ✓ VERIFIED | `src/provider.rs:701-710`; `tests/core.rs:802 fal_unparseable_success_response_bails_naming_engine_status_and_body` passes |
| 14 | 03-02 — a fal error body (flat `{"detail": …}` or array) maps into the standard envelope, with a best-effort raw-body fallback (D-37) | ✓ VERIFIED | `src/provider.rs:953-985` (`fal_error_message`), `:690-697` (`unwrap_or_else(truncate)`); runtime: flat → `Request timed out (type: request_timeout)`; array → `value is not a valid enumeration member (type: type_error.enum)`; HTML → full raw body; empty → `…(500 Internal Server Error): `; non-UTF8 → `…(503 Service Unavailable): <lossy bytes>` |
| 15 | 03-02 — `extra_params` still reaches the provider for a fal engine (D-31) | ✓ VERIFIED | `src/provider.rs:597-599` (merge first), `:600-607` (adapter keys after, so they win); `src/provider.rs:1140 fal_body_merges_openai_shaped_keys_from_extra_params` and `:1209 fal_body_keeps_png_output_format_over_an_override` pass |
| 16 | 03-02 — every existing OpenAI-compatible engine still sends byte-identical requests (D-28, D-29) | ✓ VERIFIED | `src/provider.rs:260` (`matches!(format, ProviderFormat::Fal)` gate); runtime key-set equality for dall-e-3 and Stability; `tests/cli.rs:247`, `:295` pass |
| 17 | 03-03 — `imagai list-engines` shows a format column beside the engine name (D-32) | ✓ VERIFIED | `src/cli.rs:761-769` (header `Engine Name / Format / Default Model / Base URL / API Key Set`); runtime output captured; `tests/cli.rs:111` passes |
| 18 | 03-03 — the column shows the resolved name and a broken declaration verbatim (D-24) | ✓ VERIFIED | `src/formats.rs:132-140`; runtime: `FORMAT=falai` renders `falai` in the Format column; `tests/cli.rs:136` passes |
| 19 | 03-03 — the TUI Engines tab shows the same format column, drawn from the same helper, no new filter/selector/picker (D-34) | ✓ VERIFIED | `src/tui.rs:1652-1715` (`engine_rows`), `:1701-1707` (same `format_label_cell`); `tui::tests::engines_tab_rows_name_the_format_column` passes (src/tui.rs:2123) |
| 20 | 03-03 — `--engine`, the TUI engine row and the web `engine` field are unchanged; only the adapter behind the name changes (D-33) | ✓ VERIFIED | `src/cli.rs:239-243` (unchanged resolution), `src/tui.rs:414-431` (`App.engine` → `build_request`), `src/web.rs` (`engine` field → `generate_image_core`); no picker code added |
| 21 | 03-03 — a fal engine with no `GET /models` support still lists (the per-engine fetch failure does not abort the listing) | ✓ VERIFIED | `src/provider.rs:32-37` (bail before client), `src/cli.rs:810-817` (one `[error]` line, loop continues); runtime `list-engines --all`: exactly one `[error] Failed to fetch models for 'fal': …` line followed by `📚 Models for 'oai' (all)`; `tests/cli.rs:420` passes |
| 22 | 03-03 — `.env.example` records FORMAT and its allowed values (D-38) | ✓ VERIFIED | `.env.example:15-18` (`Closed set: openai (the default), fal`) and `:58-68` (fal block incl. `…__FORMAT=fal`) |
| 23 | 03-04 — `GET /api/engines` carries a `format` field per engine (D-32, D-33) | ✓ VERIFIED | `src/web.rs:174`; runtime: `[{"base_url":"http://127.0.0.1:18711","format":"fal",…},{"base_url":"http://127.0.0.1:18711/v1","format":"openai",…}]`; `tests/web.rs:89 engines_endpoint` passes |
| 24 | 03-04 — the web format value is produced by the same helper the CLI and TUI use | ✓ VERIFIED | `src/web.rs:174` calls `crate::formats::ProviderFormat::format_label(cfg)`; single call site, no `format_declared` / resolved pair |
| 25 | 03-04 — each of the three SPA engine pickers renders the format in its option label, no new filter/selector (D-34) | ✓ VERIFIED | `web_interface.html:853`, `:873`, `:891` — three `option.textContent = \`${engine.name} (${engine.model}) · ${engine.format}\`` sites; `tests/web.rs:922-926` asserts `html.matches("${engine.format}").count() == 3`. **Rendered DOM proven 2026-10-12 in a real browser** (headless Chrome via CDP against the live `imagai web` server with fal + openai engines configured): all three `<select>`s render `Use Default Engine`, `fal (fal-ai/flux/schnell) · fal`, `oai (dall-e-3) · openai`. See "Browser DOM Closure" below. |
| 26 | 03-04 — the SPA builds option labels with `textContent`, never `innerHTML` | ✓ VERIFIED | `web_interface.html:853/873/891` use `option.textContent` + `appendChild`; the only `innerHTML` uses in the file are `= ''` container clears (773, 812, 913, 914, 1007, 1137, 1234, 1277, 1302), none on the label path |
| 27 | 03-04 — `POST /api/generate` with `{"engine": "<fal engine>"}` produces the same result the CLI produces (D-33) | ✓ VERIFIED | Runtime: `POST /api/generate {"prompt":"web fal cat","engine":"fal"}` → 200, `results[0].success=true`, saved `.png`; capture server saw the identical fal wire shape as the CLI run; `tests/web.rs:954` passes |
| 28 | 03-04 — an engine whose declaration is broken reports its raw declared value through the API | ✓ VERIFIED | `src/web.rs:174` → `format_label` verbatim; `tests/web.rs:125 engines_endpoint_reports_a_broken_format_declaration_verbatim` passes |
| 29 | 03-05 — an engine with no declared FORMAT produces the exact pre-phase request body (criterion 3, D-28) | ✓ VERIFIED | Runtime capture: dall-e-3 body keys exactly `["model","n","prompt","quality","response_format","size","style"]`; Stability body keys exactly `["aspect_ratio","mode","model","negative_prompt","prompt","seed"]` with `n`/`response_format`/`size` absent and `mode: text-to-image` injected |
| 30 | 03-05 — a declared `openai` FORMAT leaves every sniffed transport intact, including the OpenRouter chat path (D-29, D-28) | ✓ VERIFIED | `tests/core.rs:447 declared_openai_format_keeps_the_sniffed_transports` (one `/v1/chat/completions`, zero `/v1/images/generations`) and `declared_openai_edit_engine_still_saves_with_lineage` both pass |
| 31 | 03-05 — the existing edit path still works for a gpt-image engine end to end, with PNG lineage metadata and a non-zero exit on failure (criterion 3, D-26) | ✓ VERIFIED | `tests/cli.rs:635 edit_saves_source_derived_image_with_lineage_metadata` passes; runtime: a declared-`openai` gpt-image engine's edit reaches the request (not refused by the gate) |
| 32 | 03-05 — a fal engine is selectable and generates from all three surfaces (criterion 4, D-33) | ✓ VERIFIED | CLI: runtime + `tests/cli.rs:355`. Web: runtime + `tests/web.rs:954`. TUI: pty-drive (below) |
| 33 | 03-05 — the TUI's selection path is unchanged code, so parity is established by construction and documented (D-33) | ✓ VERIFIED | `src/tui.rs:414-431` (engine field) → `:386-404` (`start_generation`) → `src/tui.rs:402` `generate_image_core(&request, &settings)`. Independently evidenced by pty-drive, not by construction alone |
| 34 | 03-05 — the full suite is green with every pre-existing test still passing | ✓ VERIFIED | lib 108 / cli 52 / core 26 / web 40 = 226 passed, 0 failed, 0 ignored (baseline 166). `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean. No test deleted or narrowed — the only removals in `tests/*` are *strengthenings* (`tests/web.rs`: a loose `for needle in ["engine", …]` loop replaced by an exact 3-count + named `<select>` ids; `tests/common/mod.rs`: mock routing tightened to the `/fal-ai/` path segment) |

**Score:** 34/34 truths verified (0 present-but-behavior-unverified)

### Deferred Items

None. No identified gap is covered by a later milestone phase.

### Advisory (New Scope, Unevidenced)

Not applicable — this is an initial verification, not a re-verification round.

### Required Artifacts

| Artifact | Expected | Status | Details |
|---------|----------|--------|---------|
| `src/formats.rs` | The format registry: closed set of declared names, precedence resolution, per-format capability, display label | ✓ VERIFIED | `ProviderFormat` (src/formats.rs:17), `FORMAT_NAMES` (:29), `as_str` (:35), `from_declared` (:46), `supports_image_input` (:63), `supports_model_list` (:75), `default_host` (:84), `resolve` (:112), `format_label` (:132), `default_base_url_label` (:153), `format_label_cell` (:171); 12 unit tests |
| `src/config.rs` | Fourth per-engine env field FORMAT parsed alongside API_KEY/BASE_URL/MODEL | ✓ VERIFIED | `EngineConfig.format: Option<String>` (:15); `"format"` arm with load-time `[warning]` (:96-104); malformed-`.env` `[warning]` (:62-67) |
| `src/provider.rs` | The fal adapter: format dispatch, fal body builder, response/error mapping, format-driven `edit_transport` | ✓ VERIFIED | `edit_transport` (:167-208), `fetch_models` (:30-86), `models_url` (:106-114), `generate_images` dispatch (:251-283), `fal_base_url` (:579), `build_fal_images_body` (:594), `fal_image_size` (:625), `fal_generate` (:639), `fal_error_message` (:953) |
| `src/models.rs` | The fal response DTOs, deliberately without any OpenAI key | ✓ VERIFIED | `FalImagesResponse` / `FalImage` — no `data`, `b64_json`, top-level `url` |
| `src/lib.rs` | Declaration of the new flat sibling module | ✓ VERIFIED | `pub mod formats;` in flat alphabetical position |
| `src/cli.rs` | The format column in `cmd_list_engines`, rendered through the shared label helper | ✓ VERIFIED | `src/cli.rs:761` `format_label_cell(cfg, 12).magenta()`; header `{:<24} {:<12} {:<34} {:<28} {:<22}` (:763) |
| `src/tui.rs` | A pure `engine_rows` helper whose lines carry the format column, plus its unit test | ✓ VERIFIED | `engine_rows` (:1652-1715); `engines_tab_rows_name_the_format_column` (:2123) |
| `.env.example` | The FORMAT variable, its closed set of values, and a commented fal engine block | ✓ VERIFIED | `:15-18` wire-format block, `:58-68` fal block ending `IMAGAI__ENGINES__FAL_FLUX__FORMAT=fal` |
| `src/web.rs` | The format field on every entry of `/api/engines` | ✓ VERIFIED | `src/web.rs:174` `"format": crate::formats::ProviderFormat::format_label(cfg),` |
| `web_interface.html` | The three engine option labels carrying the format, built with textContent | ✓ VERIFIED | `:853`, `:873`, `:891` |
| `tests/cli.rs` | The no-regression proofs and the CLI-surface fal generation | ✓ VERIFIED | `openai_default_engine_body_is_unchanged` (:247), `stability_default_engine_body_is_unchanged` (:295), `fal_engine_generates_from_the_cli` (:355) |
| `tests/core.rs` | The shared-pipeline parity proof and the declared-openai transport proofs | ✓ VERIFIED | `fal_engine_saves_image_through_the_shared_pipeline` (:588), `declared_openai_format_keeps_the_sniffed_transports` (:447) |
| `tests/web.rs` | The web-surface fal generation through POST /api/generate | ✓ VERIFIED | `fal_engine_generates_through_the_web_api` (:954) |
| `tests/common/mod.rs` | A fal mock mode and captured Authorization headers | ✓ VERIFIED | `FalMode` enum, `set_fal_mode`, `auth_for` (:135), AUTHORIZATION capture (:193) |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `src/config.rs` | `src/formats.rs` | `Settings::load` parses FORMAT and validates against the closed set | ✓ WIRED | `src/config.rs:97-101` — the `[warning]` fires before any request |
| `src/provider.rs` | `src/formats.rs` | `edit_transport` asks the registry before deciding transport | ✓ WIRED | `src/provider.rs:172`, `:178` — capability check precedes `http_client()` |
| `src/formats.rs` | `src/provider.rs` | `resolve` returns the OpenAI-compatible family when nothing is declared | ✓ WIRED | `src/formats.rs:122`; the within-family sniffing (`dall-e-3`, `stability`, `openrouter.ai`+`gemini`) stays untouched in `src/provider.rs` |
| `src/provider.rs` | `src/core.rs` | `fal_generate` returns `Vec<ImageGenerationResponse>` carrying `image_url`/`image_b64_json` | ✓ WIRED | `src/provider.rs:715-724`; a PNG is saved through the unchanged pipeline (runtime + tests) |
| `src/cli.rs` | `src/formats.rs` | `cmd_list_engines` calls the one shared `format_label_cell` helper | ✓ WIRED | `src/cli.rs:761`; `src/tui.rs:1704` and `src/web.rs:174` call the same registry |
| `src/tui.rs` | `src/formats.rs` | The TUI's engine rows call the same helper as the CLI | ✓ WIRED | `src/tui.rs:1704` |
| `tests/common/mod.rs` | `src/provider.rs` | The mock serves the fal `images` shape at a path containing the model id | ✓ WIRED | `tests/common/mod.rs` routing arm; capture server observed `POST /fal-ai/flux/schnell` |
| `src/web.rs` | `src/formats.rs` | `list_engines` calls the same helper the CLI and TUI call | ✓ WIRED | `src/web.rs:174` |
| `web_interface.html` | `src/web.rs` | The SPA reads the format field `/api/engines` emits | ✓ WIRED | `web_interface.html:853/873/891` read `${engine.format}` from the field `src/web.rs:174` emits |
| `src/tui.rs` | `src/core.rs` | TUI engine row → `generate_image_core` | ✓ WIRED | `src/tui.rs:414-431` → `:386-404` → `:402`; pty-drive observed the engine name arriving there |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|--------------|--------|--------------------|--------|
| `imagai list-engines` table | Format cell | `EngineConfig.format` → `ProviderFormat::format_label` / `format_label_cell` | Yes — operator config | ✓ FLOWING |
| `imagai list-engines` table | Base URL cell | `cfg.base_url` or `default_base_url_label(cfg)` (`fal.run` vs `api.openai.com`) | Yes | ✓ FLOWING |
| `GET /api/engines` | `engines[].format` | `settings.engines` → shared label helper | Yes | ✓ FLOWING |
| fal generation | `image_url` / `image_b64_json` | Live fal HTTP response (`FalImagesResponse.images[]`) | Yes — PNG bytes were saved to disk in every runtime run | ✓ FLOWING |
| fal generation | `Authorization` header | `config.api_key` (`Key {api_key}`) | Yes | ✓ FLOWING |

### Behavioral Spot-Checks

All checks ran the **live debug binary** at HEAD `ffddab8` with two purpose-built local servers: a plain-HTTP fal endpoint that logs method/path/Host/Authorization/body, and a refusing HTTP proxy that logs every `CONNECT` authority before rejecting it (exploiting the fact that `reqwest::Client::builder()` in `src/provider.rs:14-19` never calls `.no_proxy()`, so `HTTP_PROXY`/`HTTPS_PROXY` are honoured).

| Behavior | Command | Result | Status |
|---------|---------|--------|--------|
| `FORMAT=fal` accepted | `imagai generate -p x --engine fal` (fal engine pointed at capture server) | `✅ Image 1 generated successfully!` | ✓ PASS |
| `FORMAT=openai` accepted | `imagai generate -p x --engine oai` | rc=0, no `unknown FORMAT` error | ✓ PASS |
| undeclared → openai default | `imagai list-engines` | format cell `openai` | ✓ PASS |
| bogus FORMAT warns at load | `IMAGAI__ENGINES__TYPO__FORMAT=falai imagai list-engines` | `[warning] Engine 'typo' declares an unknown FORMAT 'falai'; supported formats: openai, fal. This engine fails on its first request.` + row renders `falai` verbatim | ✓ PASS |
| `.env.example` copied verbatim parses | `cp .env.example $tmp/.env && imagai list-engines` | rc=0, no `could not parse`, engines `openai_dalle3` + `openai_gpt` | ✓ PASS |
| malformed `.env` is non-fatal, not silent | `.env` = `IMAGAI__ENGINES__<NAME>__FORMAT=openai` + good lines | rc=0, `[warning] could not parse <path>: Error parsing line: 'IMAGAI__ENGINES__<NAME>__FORMAT=openai'`, stdout `No engines configured` | ✓ PASS |
| BLOCKER-2: fal key never leaves (BASE_URL unset) | `HTTP_PROXY=HTTPS_PROXY=refusing-proxy imagai list-engines --all` (fal engine, BASE_URL unset) | **0 outbound attempts in the proxy log; the fal key string appears nowhere in it** | ✓ PASS |
| BLOCKER-2: positive control | same proxy, OpenAI-compatible engine, BASE_URL unset | proxy log records `CONNECT api.openai.com:443` — the negative above is not vacuous | ✓ PASS |
| T-03-11: one `[error]` line, loop continues | `imagai list-engines --all` with fal + openai engines | exactly one `[error] Failed to fetch models for 'fal': … no model list to fetch …` then `📚 Models for 'oai' (all)` | ✓ PASS |
| WR-02: fal with no MODEL fails fast | `IMAGAI__ENGINES__FAL__FORMAT=fal imagai generate -p x --engine fal` | `Engine 'fal' has no MODEL set: the 'fal' format derives its endpoint from the model id, so set IMAGAI__ENGINES__<NAME>__MODEL (for example 'fal-ai/flux/schnell').` | ✓ PASS |
| T-03-11 edit refusal on fal | `imagai edit -p sunset --engine fal --image source.png` | rc=1, `Engine 'fal' (model 'fal-ai/flux/schnell') cannot accept image input for edits: the 'fal' format is generation-only in this version. …` | ✓ PASS |
| edit on a capable engine proceeds | `imagai edit -p sunset --engine oai --image source.png` (gpt-image-1, declared openai) | gate passes; the run reaches the provider (a real 401 from `api.openai.com` for the placeholder key) | ✓ PASS |
| WR-04: `1024x1024` → `square_hd` | `imagai generate -p sq --engine fal --size 1024x1024` | `✅ … generated successfully`; capture body `"image_size":"square_hd"` | ✓ PASS |
| WR-04: every other size hard-fails | `--size` ∈ {`1024x1792`, `1792x1024`, `512x512`, `1024x1024 `, `square_hd`} | each: `Engine 'fal' cannot request the size '<s>' from the 'fal' format; the only size that maps exactly is 1024x1024 (fal's 'square_hd'). Pass provider-specific fields through extra_params.` | ✓ PASS |
| WR-05: HTML error body | fal engine → capture returns 502 `text/html` | `[Error] Error generating image 1: Engine 'fal' fal request failed (502 Bad Gateway): <!DOCTYPE html>…` — no save | ✓ PASS |
| WR-05: empty error body | 500, empty body | `… (500 Internal Server Error): ` — no save | ✓ PASS |
| WR-05: non-UTF8 error body | 503 `application/octet-stream`, raw bytes | `… (503 Service Unavailable): <lossy bytes>` — no panic, no save | ✓ PASS |
| WR-05: documented fal error shapes | 504 flat `{"detail":…}`, 422 `{"detail":[…]}` | `Request timed out (type: request_timeout)` / `value is not a valid enumeration member (type: type_error.enum)` | ✓ PASS |
| Criterion 4, TUI surface (pty-drive) | `pty.fork()` → `imagai tui`; type prompt, Enter; focus lands on Engine row, type `fal`, Enter; `j`×8 to ROW_SUBMIT; Enter | screen shows `🖼️ Generating 1 image(s) with engine 'fal'…` and `Generating with engine 'fal'…`; `✅ Image 1 saved to …/a_tui_fal_cat_….png`; capture logged one `POST /fal-ai/flux/schnell` with `Key fal-secret-key-123` and body `["image_size","num_images","output_format","prompt"]` | ✓ PASS |
| Web surface `/api/engines` | `imagai web` + `GET /api/engines` | `[{…,"format":"fal","model":"fal-ai/flux/schnell",…},{…,"format":"openai","model":"dall-e-3",…}]` | ✓ PASS |
| Web surface generate | `POST /api/generate {"prompt":"web fal cat","engine":"fal"}` | 200, `results[0].success=true`, saved `.png` | ✓ PASS |
| Web surface generate (no regression) | `POST /api/generate {"engine":"oai"}` | 200, `results[0].success=true` | ✓ PASS |
| Two distinct wire formats, one server | capture-server log for the two runs above | fal: `POST /fal-ai/flux/schnell`, `Key fal-secret-key-123`, keys `[image_size,num_images,output_format,prompt]`; OpenAI: `POST /v1/images/generations`, `Bearer sk-oai-secret-key-456`, keys `[model,n,prompt,quality,response_format,size,style]`; **intersection = `{prompt}`** | ✓ PASS |
| SPA renders the format in all three pickers | `GET /` from the live server | `engine.format` label templates = 3; `select` ids `engine`, `edit_engine`, `history_engine` all present | ✓ PASS |

**Full-suite runs (each executed once, output read in full):**

```
test result: ok. 108 passed; 0 failed; 0 ignored   (lib, 0.05s)
test result: ok.  52 passed; 0 failed; 0 ignored   (cli, 2.72s)
test result: ok.  26 passed; 0 failed; 0 ignored   (core, 0.03s)
test result: ok.  40 passed; 0 failed; 0 ignored   (web, 0.06s)
test result: ok.   0 passed; 0 failed; 0 ignored   (doc-tests)
```

Single-named-test confirmation of the phase-critical behaviours (each run in isolation, not filtered out of a full run):

```
cargo test --test cli list_engines                                   -> ok. 5 passed; 47 filtered out
cargo test --test cli openai_default_engine_body_is_unchanged --exact -> ok. 1 passed; 51 filtered out
cargo test --test cli stability_default_engine_body_is_unchanged --exact -> ok. 1 passed; 51 filtered out
cargo test --test cli fal_engine_generates_from_the_cli --exact      -> ok. 1 passed; 51 filtered out
cargo test --test cli unknown_format_fails_the_cli_run --exact        -> ok. 1 passed; 51 filtered out
cargo test --test cli the_shipped_env_example_loads_every_engine_it_documents --exact -> ok. 1 passed
cargo test --test cli a_malformed_env_file_is_reported_not_swallowed --exact -> ok. 1 passed
cargo test --lib formats                                             -> ok. 12 passed; 96 filtered out
cargo test --lib tui::tests::engines_tab_rows_name_the_format_column --exact -> ok. 1 passed; 107 filtered out
cargo test --test core fal                                          -> ok. 15 passed; 11 filtered out
cargo test --test core edit_against_fal_format_fails_before_any_request --exact -> ok. 1 passed
cargo test --test core declared_openai_format_keeps_the_sniffed_transports --exact -> ok. 1 passed
cargo test --test core declared_openai_edit_engine_still_saves_with_lineage --exact -> ok. 1 passed
cargo test --test web engines_endpoint                                -> ok. 2 passed; 38 filtered out
cargo test --test web fal_engine_generates_through_the_web_api --exact -> ok. 1 passed
cargo test --test web fal_engine_edit_fails_from_the_web_api --exact  -> ok. 1 passed
cargo test --test web history_form_is_served_with_engine_format_labels --exact -> ok. 1 passed
cargo test --test web fal_engine_refuses_a_size_the_enum_has_no_entry_for --exact -> ok. 1 passed
```

### Probe Execution

Not applicable — Phase 3 declares no `scripts/*/tests/probe-*.sh` probes; no PLAN, SUMMARY or success criterion mentions probe scripts, runnable PASS markers or stage markers. The phase's runnable checks are the Rust test suites and the live-binary spot-checks above.

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| PRVD-01 | 03-01, 03-02, 03-03, 03-04, 03-05 | Named non-OpenAI-compatible provider adapters alongside existing engines, selected per request | ✓ SATISFIED | `ProviderFormat::Fal` registry + adapter; selectable from `--engine`, the TUI Engine row and the web `engine` field; distinct wire format proven by capture |

No orphaned requirements: `.planning/REQUIREMENTS.md` maps exactly `PRVD-01` to Phase 3, and all five plans declare `requirements: [PRVD-01]`.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| — | — | None found | — | Scanned every phase-modified file (`.env.example`, `src/{cli,config,formats,models,provider,tui,web}.rs`, `web_interface.html`, `tests/{cli,common/mod,core,web}.rs`) for `TBD`/`FIXME`/`XXX`, `TODO`/`HACK`/`PLACEHOLDER`, `placeholder`/`coming soon`/`not yet implemented`, `return null`/`return {}`/`return []`, `console.log`, `#[ignore]`. Zero matches. No stub, no hollow data flow, no debt marker. `web_interface.html`'s nine `innerHTML` occurrences are all container clears, none on the engine-label path. |

### Human Verification Required

None — the browser-DOM item that was previously routed here has been closed (see below).

### Browser DOM Closure (2026-10-12)

The one item this report used to defer to a human — the rendered text of the three
SPA engine pickers — was closed with live browser evidence instead:

- **Product code under test:** the served `GET /` from the live `imagai web`
  server (binary at HEAD), with two engines configured by environment variables
  only (`IMAGAI__ENGINES__FAL__FORMAT=fal`, one undeclared → `openai`).
- **Driver:** headless Google Chrome (installed at `/Applications/Google Chrome.app`),
  driven over the Chrome DevTools Protocol; the page's own `DOMContentLoaded` →
  `loadEngines()` ran against the live server. (The original verifier reported
  "no browser binary" because Playwright's bundled chromium is not installed —
  the system Chrome is, so the check was automatable after all.)
- **Rendered `<option>` text, all three pickers (`#engine`, `#edit_engine`,
  `#history_engine`):**

  ```
  ["Use Default Engine", "fal (fal-ai/flux/schnell) · fal", "oai (dall-e-3) · openai"]
  ```

  Each option reads `<name> (<model>) · <format>` exactly as the human check
  specified, and the fal and openai formats render side by side — SC-4's web
  surface is now proven by rendered DOM, not by template inspection alone.

### Gaps Summary

No gaps. All 34 must-have truths (4 ROADMAP success criteria + 30 PLAN must_haves across 03-01…03-05) are verified against live code at HEAD, both by reading the implementation and by driving the real binary. The two code-review BLOCKERs are fixed and independently re-proven:

- **BLOCKER-1** (`22a5df9`) — `.env.example:18` is now commented out, so a verbatim copy parses cleanly and yields the engines it documents (`openai_dalle3`, `openai_gpt`); and `src/config.rs:62-67` turns dotenvy's parse failure into a non-fatal `[warning] could not parse <path>: …` instead of a silent zero-engine load. Both halves confirmed at runtime.
- **BLOCKER-2** (`de54540`) — `src/provider.rs:106-114` (`models_url`) returns `None` for a format that `supports_model_list()` rejects, so `fetch_models` at `:32-37` bails *before* `http_client()` is built. Proven by pointing `HTTP_PROXY`/`HTTPS_PROXY` at a local refusing proxy: a fal engine with BASE_URL unset produced **zero outbound attempts** and the fal key appears nowhere in the log, while the positive control (an OpenAI-compatible engine) did record `CONNECT api.openai.com:443`. The real fal request was observed separately against a local capture server: `POST /fal-ai/flux/schnell`, `Host: 127.0.0.1:18711`, `Authorization: Key fal-secret-key-123`, body keys `[image_size, num_images, output_format, prompt]`.

One deviation from the phase plan is recorded and accepted by the plan's own authors, not a verifier finding: `tests/cli.rs:385-396` does not assert a non-zero exit for `unknown_format_fails_the_cli_run`, because `cmd_generate` returns `Ok(())` for per-item errors today. This is Phase 4's success criterion (ROB-01) and 03-05's acceptance criteria forbid touching `src/`; the deviation is documented in 03-05-SUMMARY.md.

---

_Verified: 2026-10-11T06:21:21Z_
_Verifier: the agent (gsd-verifier)_

