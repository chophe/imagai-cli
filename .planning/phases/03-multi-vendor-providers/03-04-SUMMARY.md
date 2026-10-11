---
phase: 03-multi-vendor-providers
plan: 04
subsystem: web
tags: [web-api, spa, engines-endpoint, textcontent, xss-safety]

requires:
  - phase: 03-multi-vendor-providers
    provides: "03-01's format_label helper — the same one the CLI table and TUI table call — and 03-02's fal adapter reachable through the shared generate_image_core"

provides:
  - The format field on every GET /api/engines entry
  - Three SPA engine pickers (generate, edit, history/fork) rendering the format in their option labels
  - engines_endpoint_reports_a_broken_format_declaration_verbatim and history_form_is_served_with_engine_format_labels

affects: [03-05 regression and parity net]

actuals:
  tokens: 1783
  tasks: 2
  commits: 1

tech-stack:
  added: []
  patterns:
    - "textContent for every configuration-derived label: option.textContent builds the label from an engine name and format value reaching the SPA from operator config, so no innerHTML sink exists on that path (D-34, T-03-12)"
    - "One loadEngines feeding three selects, so a new field appears in all three pickers by construction rather than by three edits kept in sync manually"
    - "A served-HTML assertion pinned to a label token, which proves the SPA reads the field the API emits rather than a hardcoded string"

key-files:
  created: []
  modified:
    - src/web.rs
    - web_interface.html
    - tests/web.rs

key-decisions:
  - "D-33/D-34: no picker logic was added. The option `value` stays the engine name, so selection is unchanged and criterion 4 is true by construction — only the adapter behind the selected name differs"
  - "The response keeps success, engines and default_engine exactly as before, with no resolved/declared pair and no second format representation (T-03-14)"
  - "The edit picker's existing help text ('Must be an engine that accepts image input') is left alone — it is already the correct guidance now that the capability gate is format-driven"

patterns-established: []

requirements-completed: [PRVD-01]

coverage:
  - id: D1
    description: "GET /api/engines carries a format field per engine, from the shared helper, with a broken declaration reported verbatim"
    requirement: PRVD-01
    verification:
      - kind: integration
        ref: "cargo test --test web engines_endpoint, engines_endpoint_reports_a_broken_format_declaration_verbatim"
        status: pass
  - id: D2
    description: "All three SPA engine pickers render the format, display-only, with no new controls and no markup-injection sink"
    requirement: PRVD-01
    verification:
      - kind: integration
        ref: "cargo test --test web history_form_is_served_with_engine_format_labels"
        status: pass

---

# Phase 3 Plan 4: The web surface names the format Summary

**One-liner:** `/api/engines` gains a `format` field per engine and the SPA's three engine pickers append it to their labels with `textContent`, adding no control, filter or route.

## What Was Built

One JSON key on the `/api/engines` handler, sourced from the same `format_label` helper the CLI and TUI call. One label template applied where `loadEngines` builds each of the three option lists, changing `${engine.name} (${engine.model})` to `${engine.name} (${engine.model}) · ${engine.format}`.

## Tasks Completed

| Task | Name | Status | Commit |
|------|------|--------|--------|
| 1 | The web API names each engine's format (tracer) | completed | 7c057ab |
| 2 | The SPA's three engine pickers render the format | completed | 7c057ab |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Test] an `edit` call orphaned a neighbouring test's body**
- **Found during:** Task 2
- **Issue:** the `edit` used `history_form_is_served_with_fork_controls`'s signature line as its `oldString`, so the pre-existing test lost its signature and its body was left dangling after the new test.
- **Fix:** restored `history_form_is_served_with_fork_controls` intact and inserted `history_form_is_served_with_engine_format_labels` before it, so both exist and both pass.
- **Files modified:** tests/web.rs
- **Commit:** 7c057ab

## Verification Evidence

| Gate | Result |
|------|--------|
| `cargo fmt --check` | clean |
| `cargo clippy --all-targets` | clean (0 warnings) |
| `cargo test --test web engines_endpoint` | 2 passed |
| `cargo test --test web history_form_is_served_with_engine_format_labels -- --exact` | 1 passed |
| `cargo test --test web` | 37 passed (36 pre-existing + 1 new) |
| `cargo test` | 99 lib / 44 cli / 16 core / 37 web = 196, all ok |
| `innerHTML` gate | the only matches in web_interface.html are `''` clears on preview elements; none sits on the engine option label path |

All three `option.textContent` templates carry `${engine.format}` (grep count = 3).

Acceptance criteria: all three option labels include the format and use `textContent`; the served-HTML test asserts the label token plus all three select ids; `/api/engines`'s shape is unchanged by Task 2; no route, handler, payload struct or middleware changed beyond the one added JSON key.

## ROADMAP Success Criteria Touched

- Criterion 3 (second half, complete): a fal engine is discoverable through the web API.

## Files Changed

- `src/web.rs`, `web_interface.html`, `tests/web.rs`

## Self-Check: PASSED
