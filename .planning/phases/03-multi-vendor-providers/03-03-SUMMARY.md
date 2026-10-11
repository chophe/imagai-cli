---
phase: 03-multi-vendor-providers
plan: 03
subsystem: cli
tags: [discovery, list-engines, tui, env-example, format-column]

requires:
  - phase: 03-multi-vendor-providers
    provides: "03-01's crate::formats::format_label helper — the single source of the format cell every surface renders"

provides:
  - The five-column list-engines table with a Format column (Engine Name, Format, Default Model, Base URL, API Key Set)
  - The TUI Engines tab's extracted engine_rows helper carrying the same column from the same helper
  - engines_tab_rows_name_the_format_column, the TUI's first engines-tab test
  - .env.example's FORMAT variable, its closed set and a commented fal engine block

affects: [03-04 web surface, 03-05 regression and parity net]

actuals:
  tokens: 3203
  tasks: 3
  commits: 1

tech-stack:
  added: []
  patterns:
    - "One display helper, three surfaces: cmd_list_engines, tui::engine_rows and web::list_engines all call crate::formats::ProviderFormat::format_label, so the column cannot drift"
    - "Pure row builder called by a draw function: `render_models` was the precedent, `engine_rows` follows it, which is what makes the TUI's table assertable without a frame"
    - "A broken declaration is displayed verbatim rather than resolved, so a typo is visible in discovery (D-24, T-03-09)"

key-files:
  created: []
  modified:
    - src/cli.rs
    - src/tui.rs
    - tests/cli.rs
    - .env.example

key-decisions:
  - "D-32: the format column goes in the one existing discovery command, with the existing API Key Set column kept last rather than dropped — losing the key-status signal would be a regression in information, not a simplification"
  - "D-34: the TUI work is capped at rendering. No filter, no format selector, no new key handling, no change to the Engine form row"
  - "list-engines --all keeps its existing behaviour for a provider with no GET /models: the per-engine Err arm prints one [error] line and the loop continues, so no fal-specific discovery path was added (T-03-11)"
  - ".env.example documents only the variable and its allowed values (D-38); the full multi-engine walkthrough stays Phase 4's DIST-01"

patterns-established:
  - "Extracting a pure row builder out of a ratatui draw function so its output can be asserted through the file's existing row_text-style span flattening"

requirements-completed: [PRVD-01]

coverage:
  - id: D1
    description: "list-engines shows a Format column naming openai or fal per engine, and shows a broken declaration verbatim"
    requirement: PRVD-01
    verification:
      - kind: integration
        ref: "cargo test --test cli list_engines"
        status: pass
  - id: D2
    description: "The TUI Engines tab renders the same column from the same helper, with no new controls"
    requirement: PRVD-01
    verification:
      - kind: unit
        ref: "cargo test --lib tui::tests::engines_tab_rows_name_the_format_column"
        status: pass

---

# Phase 3 Plan 3: Discovery shows the format Summary

**One-liner:** `imagai list-engines` and the TUI Engines tab name each engine's wire format from one shared helper, and `.env.example` records the `FORMAT` variable and its closed set.

## What Was Built

`cmd_list_engines` renders five columns with the format cell from `format_label`, coloured magenta beside the existing cyan/green/yellow. `draw_engines_tab` lost its inline table construction to a new `engine_rows` helper — deleting the discarded `row` string in the process — and now renders exactly what that helper returns. `.env.example` gained a wire-format section and a commented `FAL_FLUX` block.

## Tasks Completed

| Task | Name | Status | Commit |
|------|------|--------|--------|
| 1 | list-engines names the format of every engine (tracer) | completed | 80c9d86 |
| 2 | The TUI Engines tab renders the same format column | completed | 80c9d86 |
| 3 | .env.example records the FORMAT variable and its values | completed | 80c9d86 |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Test] `engines_tab_rows_name_the_format_column` indexed a sorted list by position**
- **Found during:** Task 2
- **Issue:** the test asserted `rows[1]` contains `openai` and `rows[2]` contains `fal`, but `engine_names()` sorts, so `fal` is row 1 and `mock` is row 2 — the assertion failed on the first run while the rendering was correct.
- **Fix:** match rows by engine name (`starts_with("mock")`, `starts_with("fal")`) rather than by a fixed index, with a comment noting the sort.
- **Files modified:** src/tui.rs
- **Commit:** 80c9d86

**2. [Rule 3 - Blocking] `.copied()` on a `Line` slice iterator**
- **Found during:** Task 2
- **Issue:** `Line` is not `Copy`, so `iter().copied()` failed to compile.
- **Fix:** `iter().map(line_text)`, taking a reference.
- **Files modified:** src/tui.rs
- **Commit:** 80c9d86

## Verification Evidence

| Gate | Result |
|------|--------|
| `cargo fmt --check` | clean |
| `cargo clippy --all-targets` | clean (0 warnings) |
| `cargo test --test cli list_engines` | 3 passed (2 pre-existing + 1 new) |
| `cargo test --lib tui::tests::engines_tab_rows_name_the_format_column -- --exact` | 1 passed |
| `cargo test --lib tui::tests` | 19 passed |
| `cargo test` | 99 lib / 44 cli / 16 core / 35 web = 196, all ok |

Rendered table confirmed by eye with two configured engines:
```
Engine Name              Format       Default Model                      Base URL                     API Key Set
a                        openai       Not specified                      N/A (Official OpenAI)        ✅ Set
b                        fal          fal-ai/flux/schnell                N/A (Official OpenAI)        ✅ Set
```
`git diff --numstat -- .env.example` reports 14 insertions / 0 deletions — additions only.

Acceptance criteria: five columns headed `Engine Name`, `Format`, `Default Model`, `Base URL`, `API Key Set`; every row's format cell from `crate::formats::format_label`; both new list_engines tests plus the untouched `list_engines_all_fetches_models` pass; the `--all` block, sort order and empty-engines warning untouched; `.env.example` documents the closed set and the commented fal block with no real credential.

## ROADMAP Success Criteria Touched

- Criterion 3 (first half, complete): `imagai list-engines` shows the new provider — declared and typo'd alike.

## Files Changed

- `src/cli.rs`, `src/tui.rs`, `tests/cli.rs`, `.env.example`

## Self-Check: PASSED
