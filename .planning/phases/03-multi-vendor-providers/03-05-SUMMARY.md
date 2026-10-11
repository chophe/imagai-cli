---
phase: 03-multi-vendor-providers
plan: 05
subsystem: testing
tags: [regression-net, parity, key-set-equality, surface-coverage]

requires:
  - phase: 03-multi-vendor-providers
    provides: "03-02's fal adapter and mock modes, 03-03's format column and TUI rendering test, 03-04's web format field"

provides:
  - openai_default_engine_body_is_unchanged and stability_default_engine_body_is_unchanged: full key-set equality proofs for engines that declared nothing
  - declared_openai_format_keeps_the_sniffed_transports and declared_openai_edit_engine_still_saves_with_lineage: an explicit declaration changes no within-family transport
  - fal_engine_generates_from_the_cli, fal_engine_generates_through_the_web_api and fal_engine_edit_fails_from_the_web_api: one real fal generation per surface, each asserting the mock's request count
  - The phase's per-suite test counts and the TUI surface-parity statement

affects: [Phase 4 non-zero exit contract, verify-work]

actuals:
  tokens: 3673
  tasks: 2
  commits: 1

tech-stack:
  added: []
  patterns:
    - "Absence claims stated positively: criteria 3 and 4 are claims about *absence* (nothing broke, nothing is unreachable), so each is pinned as a test rather than asserted in prose"
    - "Full key-set equality rather than subset checks: collecting the captured body's keys into a sorted Vec and comparing to the recorded pre-phase list is what makes a regression test a regression test (T-03-15, T-03-18)"
    - "Per-surface request-count assertion: 'one fal request, zero OpenAI-compatible requests' is observed at the mock for each surface rather than inferred from a shared code path"

key-files:
  created: []
  modified:
    - tests/cli.rs
    - tests/core.rs
    - tests/web.rs

key-decisions:
  - "The TUI half of criterion 4 is parity by construction, not a driving test: the TUI's engine selection is the unchanged App.engine string field (ROW_ENGINE) handed to the same ImageGenerationRequest the other surfaces build, and 03-03's engines_tab_rows_name_the_format_column proves the format is visible there (D-33, T-03-17). A TUI driving test would assert a code path this phase deliberately did not create."
  - "No src/ file was modified by this plan, per its own acceptance criteria — when a test exposed a real gap it was recorded rather than patched around in a test"

patterns-established: []

requirements-completed: [PRVD-01]

coverage:
  - id: D1
    description: "An undeclared dall-e-3 engine sends byte-identical requests — full key-set equality on the captured body"
    requirement: PRVD-01
    verification:
      - kind: integration
        ref: "cargo test --test cli openai_default_engine_body_is_unchanged"
        status: pass
  - id: D2
    description: "An undeclared Stability engine keeps its exact key set, including the dropped n and response_format"
    requirement: PRVD-01
    verification:
      - kind: integration
        ref: "cargo test --test cli stability_default_engine_body_is_unchanged"
        status: pass
  - id: D3
    description: "A declared openai FORMAT leaves every sniffed transport intact, including the OpenRouter chat path and the gpt-image multipart path"
    requirement: PRVD-01
    verification:
      - kind: integration
        ref: "cargo test --test core declared_openai"
        status: pass
  - id: D4
    description: "The fal engine generates from the CLI --engine flag with exactly one fal request and zero OpenAI-compatible requests"
    requirement: PRVD-01
    verification:
      - kind: integration
        ref: "cargo test --test cli fal_engine_generates_from_the_cli"
        status: pass
  - id: D5
    description: "The fal engine generates through the web engine field and is refused for an edit before any socket opens"
    requirement: PRVD-01
    verification:
      - kind: integration
        ref: "cargo test --test web fal_engine"
        status: pass

---

# Phase 3 Plan 5: The regression and parity net Summary

**One-liner:** Full key-set equality proves every OpenAI-compatible engine still sends byte-identical requests, and one real fal generation runs from each of the CLI and web surfaces with its request count observed at the mock.

## What Was Built

Six new tests, no source change. The two CLI no-regression tests collect a captured request body's keys into a sorted `Vec<&str>` and compare it to the pre-phase list, so an accidental fal key leaking into an OpenAI-compatible body fails the suite instead of reaching a provider. The two core `declared_openai` tests re-run the OpenRouter chat and gpt-image multipart paths with an explicit `openai` declaration added. The three surface tests each assert the mock recorded exactly one request to the fal path and zero to `/v1/images/generations` (or none at all, for the refused edit).

## Tasks Completed

| Task | Name | Status | Commit |
|------|------|--------|--------|
| 1 | An undeclared engine sends byte-identical requests (tracer) | completed | edf74b2 |
| 2 | The fal engine is reachable from all three surfaces | completed | edf74b2 |

## Deviations from Plan

### Recorded Gaps (not auto-fixable within this plan's rules)

**1. [Rule 4 - Architectural] `unknown_format_fails_the_cli_run` cannot assert a non-zero exit today**
- **Found during:** Task 2
- **Issue:** the plan's behaviour list requires "an unknown FORMAT value fails the CLI run with a non-zero exit". `cmd_generate` returns `Ok(())` for per-item errors — `print_edit_results` writes the error to stderr and the process exits 0. Confirmed by running the real binary: `generate -p x --engine bad` printed the `[warning]` and the error, then exited 0.
- **Why not fixed here:** making the per-item error path exit non-zero is Phase 4's explicit success criterion ("Non-zero exit on failure", ROADMAP § Phase 4), it is a shared-code-path change also affecting `cmd_edit`, and this plan's own acceptance criteria forbid modifying any `src/` file ("if a test exposes a real defect, stop and report it rather than patching around it in a test").
- **What the test asserts instead:** stderr names the engine, the rejected value and both supported names, and the mock records zero requests — the operator-visible and no-credit-spent halves this phase owns.
- **Residual risk:** an automated caller still sees exit 0 on a failed generate. Owned by Phase 4.
- **Recorded in:** tests/cli.rs's test comment and this summary.
- **Commit:** edf74b2

### Auto-fixed Issues

**2. [Rule 1 - Test] the edit-failure envelope is nested, not top-level**
- **Found during:** Task 2
- **Issue:** the plan described `POST /api/edit` returning `success: false` with an `error` string. The real handler returns `success: true` with `results[].error`, because an edit can partially succeed.
- **Fix:** the test mirrors the existing `edit_endpoint_*` tests' two-shape check (`success == false` OR any `results[].success == false`), which is correct for both envelope shapes.
- **Files modified:** tests/web.rs
- **Commit:** edf74b2

## Verification Evidence

**Per-suite test counts, before and after this plan:**

| Suite | Baseline at phase start | After 03-01 | After 03-02 | After 03-03 | After 03-04 | After 03-05 |
|-------|------------------------|-------------|-------------|-------------|-------------|-------------|
| lib | 80 | 92 | 98 | 99 | 99 | 99 |
| cli | 43 | 43 | 43 | 44 | 44 | 48 |
| core | 8 | 9 | 16 | 16 | 16 | 18 |
| web | 35 | 35 | 35 | 35 | 37 | 39 |
| **total** | **166** | **179** | **192** | **196** | **196** | **204** |

Every suite reports strictly more tests than it inherited; no pre-existing test was deleted, weakened or `#[ignore]`d.

| Gate | Result |
|------|--------|
| `cargo fmt --check` | clean |
| `cargo clippy --all-targets` | clean (0 warnings) |
| `cargo test --test cli openai_default_engine_body_is_unchanged -- --exact` | 1 passed |
| `cargo test --test cli stability_default_engine_body_is_unchanged -- --exact` | 1 passed |
| `cargo test --test core declared_openai` | 2 passed |
| `cargo test --test cli fal_engine_generates_from_the_cli -- --exact` | 1 passed |
| `cargo test --test cli unknown_format_fails_the_cli_run -- --exact` | 1 passed |
| `cargo test --test web fal_engine` | 2 passed |
| `cargo test` | 99 lib / 48 cli / 18 core / 39 web = 204, all ok |
| `git diff --stat` vs 03-04's HEAD | tests/cli.rs, tests/core.rs, tests/web.rs only — no src/ |

**TUI surface-parity statement (criterion 4's third half):** the TUI's engine selection is the unchanged `App.engine` string field (`src/tui.rs`, `ROW_ENGINE`), handed to the same `ImageGenerationRequest` the CLI and web surfaces build, routed through the same `generate_image_core`. Plan 03-03's `engines_tab_rows_name_the_format_column` proves the format is visible in the TUI's engine list, and this plan's core-level `declared_openai` tests plus the CLI and web fal tests prove every request built from that string routes identically. D-33 makes the parity true by construction precisely because no new picker logic exists. A reviewer can verify this by reading `ROW_ENGINE`'s flow in `src/tui.rs` — no dedicated TUI driving test was written for it.

## ROADMAP Success Criteria — All Four

1. **A user can configure a named non-OpenAI-compatible provider adapter** — `IMAGAI__ENGINES__FAL__FORMAT=fal` with an API key and a model id, dispatched by 03-02's `fal_generate`.
2. **The request is built with that provider's wire format, no OpenAI body leaking in** — `fal_engine_saves_image_through_the_shared_pipeline` asserts the captured body carries `prompt`, `num_images`, `image_size`, `output_format`, `Authorization: Key test`, and none of `model`/`n`/`size`/`response_format`/`quality`/`style`; `openai_default_engine_body_is_unchanged` asserts the converse.
3. **list-engines shows the new provider and existing engines keep working** — the `Format` column (03-03) plus `openai_default_engine_body_is_unchanged`, `stability_default_engine_body_is_unchanged`, `declared_openai_format_keeps_the_sniffed_transports` and `declared_openai_edit_engine_still_saves_with_lineage`.
4. **Selectable from all three surfaces** — `fal_engine_generates_from_the_cli` (`--engine fal`), `fal_engine_generates_through_the_web_api` (web `engine` field), and the TUI by construction (D-33, statement above).

## Files Changed

- `tests/cli.rs`, `tests/core.rs`, `tests/web.rs`

## Self-Check: PASSED
