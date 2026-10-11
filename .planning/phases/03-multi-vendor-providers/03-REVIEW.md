---
phase: 03-multi-vendor-providers
reviewed: 2026-10-11T03:20:00Z
fixed: 2026-10-11T04:45:39Z
depth: standard
files_reviewed: 14
files_reviewed_list:
  - src/formats.rs
  - src/config.rs
  - src/provider.rs
  - src/models.rs
  - src/lib.rs
  - src/cli.rs
  - src/tui.rs
  - src/web.rs
  - tests/common/mod.rs
  - tests/core.rs
  - tests/cli.rs
  - tests/web.rs
  - web_interface.html
  - .env.example
findings:
  critical: 2
  warning: 6
  info: 6
  total: 14
fixed: 14
skipped: 0
status: clean
fix_report: 03-REVIEW-FIX.md
---

# Phase 3: Code Review Report

**Reviewed:** 2026-10-11T03:20:00Z
**Depth:** standard
**Files Reviewed:** 14
**Status:** clean — all 14 findings fixed or dispositioned in `/gsd-code-review --fix` (see § Fix Dispositions and 03-REVIEW-FIX.md)

## Summary

Reviewed the Phase 3 delta (`36ad9b9..edf74b2`, files above) against 03-CONTEXT.md (D-24..D-39), the five plans' `must_haves`, ROADMAP § Phase 3's four success criteria, and `.planning/codebase/CONVENTIONS.md`.

Baseline verification: `cargo fmt --check` clean, `cargo clippy --all-targets` clean (0 warnings), `cargo test` = 99 lib / 48 cli / 18 core / 39 web = 204, all passing. No source file was modified by this review; the two BLOCKERs below were each reproduced empirically in a throwaway copy of the tree under `$TMPDIR/opencode/` (deleted afterwards).

The architecture decisions landed correctly: the fal branch in `generate_images` is entered only on a resolved `ProviderFormat::Fal` (`src/provider.rs:228`), the within-family sniffing is untouched, `edit_transport`'s gate now resolves the format before the capability question and still names engine+model, and the three surfaces really do share one label helper. The key-set assertions for "no OpenAI body leaking" (`tests/core.rs` `fal_engine_saves_image_through_the_shared_pipeline`, `tests/cli.rs` `openai_default_engine_body_is_unchanged`) are real key-set assertions, not substrings — those parts of D-28/D-36 are honestly proven.

What is not sound is the configuration surface the phase ships. Two defects reach the user through the phase's own happy path and both are silent: `.env.example` now contains a line that makes the whole file unparseable, so every engine disappears; and the phase's headline discovery command (`list-engines`, success criterion 3) ships a configured fal API key to `api.openai.com` as a bearer token whenever `BASE_URL` is left unset — which is exactly the minimal fal configuration `.env.example` documents. `cargo test` cannot see either one.

## Narrative Findings (AI reviewer)

Each finding carries its disposition after the fix round. `fixed — <sha>` links the atomic commit that closed it.

### CR-01: `.env.example`'s uncommented placeholder line makes the whole file unparseable — every engine is silently dropped

> **Disposition:** fixed — `22a5df9` (placeholder commented, parse error warned, both regression tests added). The `//` marker slip introduced by the WR-04 edit and caught by that regression test was restored in `d14437a`.

**File:** `.env.example:18` (block at `.env.example:15-18`)
**Severity:** BLOCKER
**Issue:** The new line is uncommented, unlike every other commented entry in the file:

```
# ── Wire format ──────────────────────────────────────────────────
# Declares the format an engine speaks. Closed set: openai (the default), fal.
IMAGAI__ENGINES__<NAME>__FORMAT=openai
```

`dotenvy` cannot parse `IMAGAI__ENGINES__<NAME>__FORMAT` (the `<`/`>` are not key characters), and it aborts the **whole file** on that error. `Settings::load()` discards that error (`src/config.rs:56`: `let _ = dotenvy::from_path(dir.join(".env"));`), so the failure is completely silent and the process continues with **zero engines**. `.env.example`'s own line 2 says "Copy this file to `.env` and fill in your keys" — a user who follows the shipped instruction loses every engine in the file, including the working `OPENAI_DALLE3` and `OPENAI_GPT` entries, and `imagai list-engines` answers "No engines configured. Check your .env file or environment variables." pointing them at the file they just made correct.

**Evidence (reproduced):** in a throwaway copy of `HEAD`, running the real binary with a `.env` copied from `.env.example`:

```
$ imagai list-engines
[warning] No engines configured. Check your .env file or environment variables.
```

with `Settings::load()`'s `dotenvy::from_path` error surfaced (one-line probe patch, throwaway copy):

```
PROBE-DOTENVY-ERROR: Error parsing line: 'IMAGAI__ENGINES__<NAME>__FORMAT=openai', error at line index: 17
```

A control file with the same content but that line removed loads `openai_dalle3` normally, isolating line 18 as the cause. Because the loader also walks up to the repo root (`src/config.rs:51-59`), a broken root `.env` disables every engine for every invocation from any subdirectory.

**Fix:** comment the line like its neighbours (`# IMAGAI__ENGINES__<NAME>__FORMAT=openai`), or use a real engine name. Add a regression test that parses `.env.example` through `dotenvy` and asserts `Ok(())` — the shipped example is currently untested by the suite.

---

### CR-02: `list-engines` (and the TUI model fetch) send a configured fal API key to `api.openai.com` as a bearer token

> **Disposition:** fixed — `de54540`. Resolution **(b)**: discovery is gated per format, with an operator-visible reason. Preferred option (a) was rejected because fal has no `/models` endpoint, so defaulting the host per format only converts a credential leak into a guaranteed-failing request to a host the user did not choose. Proven at both layers: `models_url` constructs no URL at all for a non-OpenAI-compatible format, and a fal engine pointed at a *reachable* listing-shaped endpoint still spends no request. 03-03-PLAN's T-03-11 contract is preserved — one `[error]` line per failed engine, loop continues.

**File:** `src/provider.rs:24-28` with callers `src/cli.rs:770-780` and `src/tui.rs:1791-1793`; triggered by `.env.example:62-65`
**Severity:** BLOCKER
**Issue:** `fetch_models` is format-blind and falls back to the OpenAI host when `base_url` is `None`:

```rust
let url = match base_url {
    Some(b) => format!("{}/models", b.trim_end_matches('/')),
    None => "https://api.openai.com/v1/models".to_string(),
};
let resp = client.get(&url).bearer_auth(api_key).send().await?;
```

The fal family is the first family in this crate whose default host is *not* OpenAI — `fal_base_url` (`src/provider.rs:532-537`) defaults to `https://fal.run` — and `.env.example`'s fal block deliberately leaves `BASE_URL` commented out (line 63), i.e. the documented minimal fal engine is `API_KEY` + `MODEL` + `FORMAT`. `cmd_list_engines` runs the fetch loop for **every** engine with `key_set()`, unconditionally (no `--all` gate), and the TUI's `fetch_models` does the same. So with the shipped minimal fal configuration, the phase's own discovery command (success criterion 3) transmits the fal API key, in a `Bearer` header, to a host the user never configured. The correct host is not reached, and the credential is disclosed to a third party over a credentialed request.

**Evidence (reproduced):** real binary from a throwaway `HEAD` copy, with `fetch_models`' `None` arm redirected to a local capture server, and `IMAGAI__ENGINES__FAL__API_KEY=fal-secret-key-123`, `MODEL=fal-ai/flux/schnell`, `FORMAT=fal`, no `BASE_URL`:

```
$ imagai list-engines
Engine Name              Format       Default Model                      Base URL                     API Key Set
fal                      fal          fal-ai/flux/schnell                N/A (Official OpenAI)        ✅ Set
[error] Failed to fetch models for 'fal': models fetch failed (401 Unauthorized): {}

# captured request:
GET /v1/models HTTP/1.1
authorization: Bearer fal-secret-key-123
```

Unpatched, that URL is `https://api.openai.com/v1/models`.

**Fix:** make discovery format-aware — `fetch_models` should take the resolved `ProviderFormat` (or the caller should gate on it) and skip non-OpenAI families with the existing per-engine failure line ("`the 'fal' format has no GET /models endpoint`"), rather than substituting the OpenAI host. Do not "fix" this by defaulting to `fal_base_url`: fal has no `/models`, and the reverse leak (an OpenAI key bound for `fal.run`) is equally wrong. A test should assert `mock.requests()` is empty after a fal engine's `list-engines`, mirroring the edit gate's proof.

---

### WR-01: `list-engines` tells the user a fal engine with no `BASE_URL` is on "N/A (Official OpenAI)"

> **Disposition:** fixed — `cf36dc5` (`ProviderFormat::default_base_url_label`, one helper beside `format_label`; CLI table, TUI panel and unit test).

**File:** `src/cli.rs:747-750` (and `src/tui.rs:1681-1684`)
**Severity:** WARNING
**Issue:** The default-host string is still the OpenAI one, so a fal engine — whose default host is `fal.run` (`src/provider.rs:532-537`) — is displayed as if it were the official OpenAI endpoint, in the very table D-32 added to make the new provider discoverable. Reproduced in CR-02's output above.

**Fix:** make the fallback format-aware (`N/A (fal.run)` / `N/A (Official OpenAI)`), the same way `fal_base_url` already is.

---

### WR-02: a fal engine with no `MODEL` silently POSTs to `https://fal.run/dall-e-3`

> **Disposition:** fixed — `ab79dbf`. The shared `dall-e-3` default is now bypassed for the fal branch, which bails naming the engine, the missing field and the variable to set, before any socket — proven against a *reachable* mock with an empty request set.

**File:** `src/provider.rs:209-212` (the shared `unwrap_or_else(|| "dall-e-3")`), reached via `src/provider.rs:593`
**Severity:** WARNING
**Issue:** `dall-e-3` is the OpenAI family's default id; for the fal family the model id *is* the URL path, so the default silently produces a request to a nonsense endpoint. The phase's own rationale ("a config mistake must not spend a request", D-05, applied to an unknown `FORMAT`) is not applied to a missing `MODEL`, and the resulting operator-visible message names no cause.

**Evidence (reproduced):** `IMAGAI__ENGINES__NOMODEL__FORMAT=fal`, no `MODEL`, `imagai generate -p hi --engine nomodel` → `[Error] Error generating image 1: Engine 'nomodel' fal request failed (404 Not Found): ` — a bald 404 with an empty body (fal returns `content-length: 0` for an unknown model path), with no hint that the engine defaulted to `dall-e-3`.

**Fix:** in the `Fal` branch, bail naming the engine when `cfg.model` is `None`, before any socket opens.

---

### WR-03: `fal_generate`'s raw `?` on send/read errors loses the engine and status naming every other adapter path provides

> **Disposition:** fixed — `2767d81`. Both sites now name the engine. The send failure is proven by a connection-refused test; the read failure mirrors the OpenAI path's `failed reading response:` and is defensive, because a body that fails *mid-stream* cannot be produced from the axum mock without a new dependency — the adjacent non-UTF8 shape is covered end to end by the WR-05 tests.

**File:** `src/provider.rs:615-618`
**Severity:** WARNING
**Issue:** The OpenAI-compatible generation path wraps both failure points (`src/provider.rs:284-289` `"request error: {e}"`, `src/provider.rs:293-299` `"failed reading response: {e}"`). `fal_generate` propagates the `reqwest::Error` unchanged, so a connection failure or a non-UTF8 body produces a bare `error sending request for url (…)` with no engine name and no HTTP status. That is the one case in the review's focus list ("non-UTF8", "no panics on odd shapes") where D-35/D-37's envelope parity is not met — no panic occurs, but the envelope loses the engine identifier the OpenAI path always carries.

**Fix:** mirror the OpenAI-compatible wrapping at both sites (`format!("Engine '{}' fal request failed: {e}", request.engine)`), keeping the body-read failure distinct from the send failure.

---

### WR-04: the fal size mapping silently substitutes a different aspect ratio for two of the three accepted sizes

> **Disposition:** fixed — `58faa13`. Resolution: map **only** the geometrically exact size (`1024x1024` → `square_hd`) and hard-fail the rest, which is consistent with the function's own stated principle and with 03-02-PLAN's "do not extend the table by guessing at aspect ratios". The substitution is documented nowhere new because there is nothing to document; `.env.example` records the one supported size. SPA alignment is behavioural, not a dropdown change: the web surface now proves a non-square pick fails fast through `/api/generate` naming the engine, the rejected size and the one size that maps.

**File:** `src/provider.rs:570-581` (`fal_image_size`), used at `src/provider.rs:557-559`
**Severity:** WARNING
**Issue:** `1024x1792` (ratio 0.571) maps to `portrait_4_3` (0.75) and `1792x1024` (1.75) maps to `landscape_4_3` (1.33). Orientation is preserved, the requested geometry is not, and nothing tells the user. The code's own stated principle two lines above — "silently dropping an unmapped size would hand the provider its own default … and give the user a landscape image they never asked for" — is exactly what happens here, for 2 of 3 accepted sizes. Nothing in the plans, `.env.example`, or `--size`'s help records the substitution, so the mapping was invented and left undocumented.

Secondary, same code path: the web SPA's size dropdown offers `512x512` and `768x768` (`web_interface.html:396-401`), which hard-fail for a fal engine (`fal_image_size` bails). Correct behavior, but the resulting message is only unit-tested at the builder level, never through a surface.

**Fix:** either map only sizes that are geometrically equivalent and bail on the rest (consistent with the comment), or keep the nearest-enum mapping and document it in `.env.example`/`--size` help as "fal: 1024x1024, otherwise the nearest 4:3 portrait/landscape". One integration test pinning whichever choice is made.

---

### WR-05: D-37's best-effort raw-body fallback is only proven for a JSON error body

> **Disposition:** fixed — `2ba6e2e`. `FalMode::RawError { status, body: Vec<u8> }` added (bytes rather than the suggested `String` so one variant covers HTML, empty and non-UTF8 bodies); three tests assert the truncated HTML body with status, the empty-body named failure and the non-UTF8 named failure.

**File:** `tests/common/mod.rs:280-296` (`FalMode::Error`) with `tests/core.rs` `fal_error_body_without_detail_falls_back_to_the_raw_body`
**Severity:** WARNING
**Issue:** The three fal error tests cover a string `detail`, an array `detail`, and a null `detail` — all JSON. The decision D-37 ("unmapped shapes fall back to best-effort raw body with status", including gateway HTML and empty bodies) is never exercised end to end. The code is correct for those shapes by inspection (`fal_error_message` returns `None` → `truncate(&text, 400)`, `src/provider.rs:883-914`), but an HTML 502 from a proxy — the most likely real-world unmapped shape — has no test asserting either the fallback or the 400-char truncation. The empty-body case observed in WR-02 shows the operator-facing message can degenerate to `"…(404 Not Found): "`.

**Fix:** add `FalMode::RawError { status, body: String }` and one test asserting the truncated raw body and the status travel through the envelope.

---

### WR-06: `Settings::load` swallows a `.env` parse error, which is what turned CR-01 into silent total config loss

> **Disposition:** fixed — `22a5df9` (the same commit as CR-01's first half, since they are one failure mode). `src/config.rs` now emits the existing non-fatal `[warning]` convention naming the file and the parse error; `a_malformed_env_file_is_reported_not_swallowed` proves it through the real binary.

**File:** `src/config.rs:51-59` (`let _ = dotenvy::from_path(dir.join(".env"));` on line 56)
**Severity:** WARNING
**Issue:** Pre-existing, but this phase is what makes it user-visible: a `.env` that fails to parse now yields *zero engines* with no diagnostic, indistinguishable from "the user configured nothing". The existing non-fatal-`[warning]` convention (`src/config.rs:107`) is applied to the output dir but not to `.env`.

**Fix:** `if let Err(e) = dotenvy::from_path(dir.join(".env")) { eprintln!("[warning] could not parse {path:?}: {e}"); }` — one line, and it converts CR-01's silent loss into a named failure.

---

### IN-01: `format_label` and `resolve` duplicate the declaration-parsing rule

> **Disposition:** fixed — `e7cfd56`. `ProviderFormat::declared(cfg)` is now the single expression; `resolve_and_format_label_read_the_same_declaration` guards against drift.

**File:** `src/formats.rs:75-113`
**Issue:** Both functions independently implement "trim → filter empty → match closed set → else raw/error". Adding a third format means editing both, and a divergence between "what resolves" and "what is displayed" would be invisible to the current tests (they only test each in isolation).

**Fix:** have `resolve` return the raw declaration on failure (or have `format_label` call a shared `fn declared(cfg) -> Option<&str>`) so the precedence rule has one expression.

---

### IN-02: the served-HTML assertion's needles are near-vacuous

> **Disposition:** fixed — `95e7735`. The loop is replaced by an exact count of the label template (3, one per picker) plus each picker's `<select>` id.

**File:** `tests/web.rs` `history_form_is_served_with_engine_format_labels`
**Issue:** `for needle in ["engine", "edit_engine", "history_engine"] { assert!(html.contains(needle)) }` — `"engine"` is satisfied by any script text in a 1300-line SPA, so it asserts almost nothing. The meaningful assertion in this test is `html.contains("engine.format")`.

**Fix:** assert the three label templates directly (e.g. that each picker's option-building block reads `engine.format`), or drop the loop.

---

### IN-03: the format column is unbounded, so a long raw declared value misaligns the table

> **Disposition:** fixed — `d115698` (`ProviderFormat::format_label_cell`, used by the CLI table at width 12 and the TUI panel at width 10; the full value still reaches `/api/engines` and the first request's error, so D-24's verbatim rule holds).

**File:** `src/cli.rs:733-766`, `src/tui.rs:1655-1658`
**Issue:** D-24 requires a broken declaration to render verbatim, which the code does — but with `{:<12}` / `{:<10}` padding and no truncation, a 40-character typo pushes Base URL, Default Model and API Key Set off the fixed columns. Display-only.

**Fix:** truncate the cell with an ellipsis past the column width (keeping the full value in the JSON API and in the error message on first request), or accept and note it.

---

### IN-04: the mock's fal routing is an implicit string coupling

> **Disposition:** fixed — `b46d0f9`. The dispatch now matches the `/fal-ai/` path segment the adapter builds, and the comment states what the coupling is.

**File:** `tests/common/mod.rs:185-187` (`if path.contains("fal-ai")`)
**Issue:** Any mock engine whose model id or path contains `fal-ai` is served the fal mode, whatever its declared format. It works for the current tests but would silently misroute a future OpenAI-compatible mock engine.

**Fix:** route on the request path prefix that the adapter builds, or key the mode off an explicit route.

---

### IN-05: `extra_params` can still inject OpenAI-shaped keys into a fal body from the library API

> **Disposition:** **guard dismissed — recorded conflict with D-31**, plus the test-scope correction. D-31 keeps `extra_params` unchanged for both families and 03-02-PLAN explicitly rejects per-format field allowlists; stripping OpenAI images-API keys from the merged body would invent exactly that mechanism, so the guard is not applied. What *is* fixed is the overstatement: `fal_body_carries_only_fal_keys` now states its scope (the default body, no extras), and `fal_body_merges_openai_shaped_keys_from_extra_params` pins the escape hatch's real contract — the caller's keys travel, while the adapter still pins `output_format` (D-27) and keeps the model id in the URL path (D-36). No CLI/web surface can reach these keys (the seven named flags only), so "library-API-only" is now the documented scope rather than an implied one. Commit `08e1ebf`.

**File:** `src/provider.rs:547-562`
**Issue:** The adapter defends its own `output_format` against an override, but `model`, `n`, `size`, `response_format`, `quality` and `style` arriving through `extra_params` are merged straight through, so `fal_body_carries_only_fal_keys` proves the *default* shape, not an invariant. D-31 keeps `extra_params` unchanged for both families and no CLI/web surface can reach these keys (the seven named flags only), so this is library-API-only — but the test name overstates what is proven.

**Fix:** strip the OpenAI images-API keys from the merged body the way `output_format` is pinned, or rename the test to state its scope.

---

### IN-06: test-file section separators drift in the new tests

> **Disposition:** fixed — `eec628b` (`// ---- fal engine`, matching CONVENTIONS.md's recorded rule).

**File:** `tests/core.rs:538-850`
**Issue:** `CONVENTIONS.md` records `// ---- name` rules as the test-file convention and `tests/cli.rs` follows it (`// ---- fal engine`); the new `tests/core.rs` block is appended after the helper functions without one.

**Fix:** add a `// ---- fal engine` separator for consistency.

---

## Contract notes (verified, not findings)

- **D-28 / D-29 no-regression:** verified structurally (fal branch gated on `matches!(format, ProviderFormat::Fal)`, `src/provider.rs:228`) and by `openai_default_engine_body_is_unchanged`, `stability_default_engine_body_is_unchanged`, `declared_openai_format_keeps_the_sniffed_transports`, `declared_openai_edit_engine_still_saves_with_lineage` — those key-set assertions are exhaustive, not substring greps. No existing engine path changed behavior.
- **D-30 / D-05 edit gate:** `edit_transport` resolves the format *before* the capability question (`src/provider.rs:140`), refuses fal naming engine+model, and `edit_against_fal_format_fails_before_any_request` proves it with an empty `mock.requests()`.
- **D-26:** the web `/api/edit` gate is proven by `fal_engine_edit_fails_from_the_web_api` with the same empty-request proof.
- **D-27:** the fal response maps onto `ImageGenerationResponse`; `fal_engine_saves_image_through_the_shared_pipeline` asserts the unchanged save path wrote the PNG `tEXt` lineage.
- **D-32 / D-33 / D-34 / D-38:** one label helper across CLI/TUI/web (`format_label`), display-only in all three surfaces, `textContent` (not `innerHTML`) in the SPA, and the `FORMAT` variable recorded in `.env.example`.
- **ROB-01 (accepted residual):** confirmed recorded and correctly *not* treated as new. `unknown_format_fails_the_cli_run` documents the deviation in-test and 03-05-SUMMARY.md § Deviations #1 records it, with `.planning/STATE.md:108` carrying it forward as Phase 4's success criterion. `cmd_generate` returning `Ok(())` for per-item errors is genuinely Phase 4's to fix.

---

## Fix Dispositions

Fix round run by `the agent (gsd-code-fixer)` via `/gsd-code-review --fix`; full report in 03-REVIEW-FIX.md.

| Finding | Disposition | Commit |
|---------|-------------|--------|
| BLOCKER-1 (CR-01) | fixed — placeholder commented; `.env` parse error now warned; two regression tests (shipped example loads / malformed file reported) | `22a5df9` |
| BLOCKER-2 (CR-02) | fixed — discovery gated per format (resolution b), no URL constructed for a non-listing format; CLI + TUI call sites take engine+config | `de54540` |
| WR-01 | fixed — `default_base_url_label` names each format's own host in CLI and TUI | `cf36dc5` |
| WR-02 | fixed — fal branch requires MODEL, bails naming engine + variable before any socket | `ab79dbf` |
| WR-03 | fixed — send/read failures name the engine | `2767d81` |
| WR-04 | fixed — only the exact size maps; others hard-fail; documented in `.env.example`; proven through core and web surfaces | `58faa13` |
| WR-05 | fixed — `FalMode::RawError`; HTML/empty/non-UTF8 fallback tests | `2ba6e2e` |
| WR-06 | fixed — same commit as BLOCKER-1's second half | `22a5df9` |
| IN-01 | fixed — one `declared()` expression, guarded by a drift test | `e7cfd56` |
| IN-02 | fixed — exact template count + picker ids | `95e7735` |
| IN-03 | fixed — `format_label_cell` clips to the column in both surfaces | `d115698` |
| IN-04 | fixed — mock routes on the `/fal-ai/` path segment | `b46d0f9` |
| IN-05 | guard dismissed (D-31 conflict), test scope stated + escape hatch pinned | `08e1ebf` |
| IN-06 | fixed — `// ---- fal engine` separator | `eec628b` |

Supporting fix: `d14437a` restored the fal block's comment marker in `.env.example`, a `//` slip introduced by the WR-04 edit and caught by BLOCKER-1's regression test.

**Post-fix verification:** `cargo fmt --check` clean; `cargo clippy --all-targets` clean (0 warnings); `cargo test` = 108 lib / 52 cli / 26 core / 40 web = 226 passing (baseline 204 + 22 new; none removed or weakened). Both BLOCKERs were re-verified as failing-first: with the discovery gate disabled, `fal_engine_discovery_is_refused_even_with_a_reachable_host` and both `models_url` unit tests fail; with the `MODEL` guard removed, `fal_engine_with_no_model_fails_before_any_request` fails; with the CR-01 fixes stashed, both `.env` regression tests fail.

**No conflicts with locked decisions:** no fix contradicts D-24..D-39 or a ROADMAP success criterion. Two notes: (1) BLOCKER-2 changes 03-03-PLAN's "no code change" position on fal discovery (T-03-11) — the observable contract it protected (one `[error]` line per engine, loop continues) is preserved, and BLOCKER-2 supersedes it because the leak reaches the user through the phase's own happy path; (2) WR-04 removes two entries from a table 03-02-PLAN prescribed, in the direction that plan's own stated principle requires, and no `must_have` pins the substituted geometry.

---

_Reviewed: 2026-10-11T03:20:00Z_
_Reviewer: the agent (gsd-code-reviewer)_
_Depth: standard_
_Fixed: 2026-10-11T04:45:39Z · status clean (14/14 dispositioned)_
