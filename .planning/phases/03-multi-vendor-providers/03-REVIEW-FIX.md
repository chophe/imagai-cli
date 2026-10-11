---
phase: 03-multi-vendor-providers
fixed_at: 2026-10-11T04:45:39Z
review_path: .planning/phases/03-multi-vendor-providers/03-REVIEW.md
iteration: 1
findings_in_scope: 14
fixed: 13
skipped: 0
status: all_fixed
---

# Phase 3: Code Review Fix Report

**Fixed at:** 2026-10-11T04:45:39Z
**Source review:** `.planning/phases/03-multi-vendor-providers/03-REVIEW.md`
**Iteration:** 1

**Summary:**
- Findings in scope: 14 (2 BLOCKER, 6 WARNING, 6 INFO)
- Fixed: 13
- Dismissed with recorded reason: 1 (IN-05's guard — D-31 conflict; the scope correction half was applied)
- Skipped: 0

Verification ran in this agent's **isolated git worktree** (`.claude/worktrees/rf-03-45156-<epoch>`), which was then removed; the numbers below are reproducible from `main` at `d14437a`.

## Fixed Issues

### BLOCKER-1: `.env.example`'s uncommented placeholder makes the whole file unparseable

**Files modified:** `.env.example`, `src/config.rs`, `tests/cli.rs`
**Commit:** `22a5df9`
**Applied fix:** The `<NAME>` placeholder line is commented like its neighbours, and `Settings::load` no longer swallows `dotenvy`'s error — it emits the existing non-fatal `[warning]` convention naming the file and the parse error. Two regression tests, both verified failing-first: `the_shipped_env_example_loads_every_engine_it_documents` (copy `.env.example` into a temp cwd, run the real binary, assert both documented engines load and no parse warning) and `a_malformed_env_file_is_reported_not_swallowed` (a `.env` with a bad line yields both "No engines configured" and the named `[warning] could not parse …`). This test caught a `//` marker slip re-introduced in `.env.example` by the later WR-04 edit, restored in `d14437a`.

### BLOCKER-2: `list-engines` sends a configured fal key to `api.openai.com`

**Files modified:** `src/formats.rs`, `src/provider.rs`, `src/cli.rs`, `src/tui.rs`, `tests/core.rs`
**Commit:** `de54540`
**Applied fix:** Resolution **(b)** — discovery is gated per format. `fetch_models` now takes the engine name and its `EngineConfig`, resolves the declared format itself (so neither caller can forget it), and the new private `models_url` returns `None` for a format with no listing endpoint, so **no URL is constructed at all** — no request, and therefore no credential, can leave for a host the operator did not configure. The engine is named in the operator-visible reason, and the per-engine `Err` arm is unchanged, so one failed engine still costs one `[error]` line and the loop continues (03-03's T-03-11 contract). Preferred option (a) was rejected: fal has no `/models`, so a per-format default host only converts a leak into a guaranteed-failing request. Proof in two layers, both failing-first: `models_url(None|Some(…), Fal) == None` plus `the_openai_compatible_default_host_is_unchanged` (unit), and `fal_engine_discovery_is_refused_even_with_a_reachable_host` with an empty `mock.requests()` against a *live* host (integration). The CLI surface is covered by the WR-01 test, which asserts the real binary prints the refusal.

### WR-01: the base-URL column claims "Official OpenAI" for a fal engine

**Files modified:** `src/formats.rs`, `src/cli.rs`, `src/tui.rs`, `tests/cli.rs`
**Commit:** `cf36dc5`
**Applied fix:** `ProviderFormat::default_base_url_label` sits beside `format_label` and returns the format's own host (`N/A (fal.run)` / `N/A (Official OpenAI)`), so the CLI table and the TUI panel cannot drift. Pinned by a unit test, an extension of the existing `engines_tab_rows_name_the_format_column`, and `list_engines_names_the_base_url_each_format_defaults_to` (verified failing-first).

### WR-02: a fal engine with no MODEL silently POSTs to `…/dall-e-3`

**Files modified:** `src/provider.rs`, `tests/core.rs`
**Commit:** `ab79dbf`
**Applied fix:** The fal branch in `generate_images` no longer inherits the shared `dall-e-3` default; it bails naming the engine, the missing field and the variable to set, before any socket. `fal_engine_with_no_model_fails_before_any_request` points the engine at a *reachable* mock and asserts an empty request set (verified failing-first).

### WR-03: fal send/read errors propagate bare `reqwest::Error`s

**Files modified:** `src/provider.rs`, `tests/core.rs`
**Commit:** `2767d81`
**Applied fix:** Both sites now match the OpenAI-compatible path's wrapping — `Engine '{engine}' fal request failed: {e}` for send and `… fal response body could not be read: {e}` for read. `fal_send_failure_names_the_engine` uses a port that was listening a moment ago and is not now, so the failure is immediate. The read-arm wrapping is defensive parity: a body that fails mid-stream cannot be produced from the axum mock without a new dependency, and the adjacent non-UTF8 shape is covered end to end by the WR-05 tests.

### WR-04: silent 4:3 substitution for 2 of 3 accepted fal sizes

**Files modified:** `src/provider.rs`, `tests/core.rs`, `tests/web.rs`, `.env.example`
**Commit:** `58faa13`
**Applied fix:** Mapped **only** the geometrically exact size (`1024x1024` → `square_hd`), hard-failing everything else with a message that names the engine, the rejected size, the one size that maps and the `extra_params` escape hatch — consistent with the function's own stated principle and with 03-02-PLAN's "do not extend the table by guessing at aspect ratios". `.env.example` records the supported size. SPA alignment is behavioural: the web surface test `fal_engine_refuses_a_size_the_enum_has_no_entry_for` proves a non-square pick fails fast through `/api/generate` naming everything, with no request spent.

### WR-05: D-37's raw-body fallback proven only for JSON

**Files modified:** `tests/common/mod.rs`, `tests/core.rs`
**Commit:** `2ba6e2e`
**Applied fix:** `FalMode::RawError { status, body: Vec<u8> }` — bytes rather than the suggested `String`, so one variant covers HTML, empty and non-UTF8 bodies. Three tests: the 600-byte HTML 502 asserts the first 400 bytes plus the ellipsis and *not* the whole body; the empty 404 and the non-UTF8 500 both assert a named failure with the status and no phantom save.

### WR-06: `Settings::load` swallows a `.env` parse error

**Files modified:** `src/config.rs` (with BLOCKER-1)
**Commit:** `22a5df9`
**Applied fix:** One `if let Err(e)` around `dotenvy::from_path` emitting `[warning] could not parse {path}: {e}`, converting the silent loss into a named failure. Same commit as BLOCKER-1 because they are one failure mode; its regression test proves the warning reaches stderr through the real binary.

### IN-01: `format_label` and `resolve` duplicate the parsing rule

**Files modified:** `src/formats.rs`
**Commit:** `e7cfd56`
**Applied fix:** `ProviderFormat::declared(cfg) -> Option<&str>` is the single expression of the precedence rule; both functions read through it. `resolve_and_format_label_read_the_same_declaration` fails on any future divergence.

### IN-02: near-vacuous served-HTML needles

**Files modified:** `tests/web.rs`
**Commit:** `95e7735`
**Applied fix:** Replaced the `for needle in ["engine", …]` loop with an exact count of the label template (`${engine.format}` == 3, one per picker) plus each picker's `<select>` id.

### IN-03: the format column is unbounded

**Files modified:** `src/formats.rs`, `src/cli.rs`, `src/tui.rs`, `tests/cli.rs`
**Commit:** `d115698`
**Applied fix:** `ProviderFormat::format_label_cell(cfg, width)` clips with an ellipsis past the column width, used at 12 by the CLI table and 10 by the TUI panel. D-24's verbatim rule still holds: `format_label` carries the full value, `/api/engines` emits it, and the CLI test asserts the 40-character typo appears in full in the error line while the table cell stays clipped.

### IN-04: the mock's fal routing is an implicit string coupling

**Files modified:** `tests/common/mod.rs`
**Commit:** `b46d0f9`
**Applied fix:** Dispatch matches the `/fal-ai/` path segment the adapter builds rather than a bare substring, and the comment states what the remaining coupling is: an engine reaches the fal arm only by asking for that exact model path.

### IN-06: missing section separator in the new test block

**Files modified:** `tests/core.rs`
**Commit:** `eec628b`
**Applied fix:** Added `// ---- fal engine`, matching the rule CONVENTIONS.md records and `tests/cli.rs` follows.

## Dismissed (with reason)

### IN-05: `extra_params` can still inject OpenAI-shaped keys into a fal body

**File:** `src/provider.rs:547-562`
**Reason:** **Locked-decision conflict — the guard half is not applied.** D-31 keeps `extra_params` unchanged for both families, and 03-02-PLAN explicitly records the rejection of "per-format field allowlists (invented mechanism that constrains what platform features already do)". Stripping `model`/`n`/`size`/`response_format`/`quality`/`style` from the merged body would build exactly that mechanism, so the guard was dismissed rather than worked around.
**Applied instead** (commit `08e1ebf`): the review's second option — the overstated test now states its scope. `fal_body_carries_only_fal_keys` documents that it pins the *default* body with no extras, and a new `fal_body_merges_openai_shaped_keys_from_extra_params` pins the escape hatch's real contract: caller-supplied keys travel unchanged (D-31), while the adapter still pins `output_format` (D-27) and keeps the model id in the URL path (D-36). The finding's "library-API-only" caveat is now the documented scope — no CLI or web surface can reach these keys (the seven named flags only) — so the contract is stated rather than implied.

## Verification

Run in the isolated worktree before it was removed:

```
$ cargo fmt --check
(clean, exit 0)

$ cargo clippy --all-targets
    Checking imagai v0.1.0 (…/rf-03-45156-<epoch>)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.37s

$ cargo test
     Running unittests src/lib.rs   → 108 passed; 0 failed
     Running unittests src/main.rs  →   0 passed; 0 failed
     Running tests/cli.rs           →  52 passed; 0 failed
     Running tests/core.rs          →  26 passed; 0 failed
     Running tests/web.rs           →  40 passed; 0 failed
     Doc-tests imagai               →   0 passed; 0 failed
```

Baseline was 99 lib / 48 cli / 18 core / 39 web = 204; the fix round adds 22 tests (9 lib, 4 cli, 8 core, 1 web) and removes or weakens none. No regression in Phase 1/2 behaviour: the pre-existing no-regression key-set tests (`openai_default_engine_body_is_unchanged`, `stability_default_engine_body_is_unchanged`, `declared_openai_format_keeps_the_sniffed_transports`, `declared_openai_edit_engine_still_saves_with_lineage`) all pass unchanged. No new dependency.

Failing-first evidence collected during the round:

| Finding | With the fix reverted |
|---------|----------------------|
| BLOCKER-1 | both `.env` regression tests FAILED |
| BLOCKER-2 | `fal_engine_discovery_is_refused_even_with_a_reachable_host` + 2 `models_url` unit tests FAILED |
| WR-01 | `list_engines_names_the_base_url_each_format_defaults_to` FAILED |
| WR-02 | `fal_engine_with_no_model_fails_before_any_request` FAILED |

 ROB-01 (accepted residual, Phase 4 scope) remains accepted and untouched — recorded in `.planning/STATE.md:108`.

---

_Fixed: 2026-10-11T04:45:39Z_
_Fixer: the agent (gsd-code-fixer)_
_Iteration: 1_
