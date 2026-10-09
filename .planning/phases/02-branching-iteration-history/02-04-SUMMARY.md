---
phase: 02-branching-iteration-history
plan: 04
type: execute
wave: 2
subsystem: web-surface
tags: [web, axum, rest, spa]
requires: [02-01]
provides:
  - GET /api/history and POST /api/fork
  - the SPA history view (node select + Fork selected node CTA + live refresh)
affects: [src/web.rs, web_interface.html]
tech-stack:
  added: []
  patterns: [one HistoryView serializer for CLI and web, resolver-shared error vocabulary, is_plain_filename on every path-shaped field]
key-files:
  created: []
  modified: [src/web.rs, web_interface.html, tests/web.rs]
decisions:
  - "The fork handler rejects an empty prompt with the CLI's `Prompt cannot be empty.` sentence — `/api/edit` has no such check and the plan's behaviour list requires the fork to reject it, so the check lives on fork only and `edit` stays byte-identical"
  - "`from` resolves through history::resolve_fork_source, whose three error sentences become the endpoint's entire error vocabulary — a browser user reads the same words as a CLI user"
metrics:
  duration: "~30m"
  completed: 2026-10-10T10:05:00Z
  tasks: 3
  commits: 1
  files: 3
plan_head_before: 3569561
plan_head_after: f688365
status: complete
actuals:
  tokens: 30000
  tasks: 3
  commits: 1
estimate:
  tokens: 34000
  tasks: 3
  confidence: low
---

# Phase 2 Plan 04: Web history + fork Summary

**One-liner:** `GET /api/history` returns the same nested tree the CLI's `--json` prints, `POST /api/fork` forks any past image behind the same `is_plain_filename` trust boundary as `/api/edit`, and the SPA renders both with no new CSS rule and no new dependency.

## What Was Built

**`src/web.rs`**

- `ForkPayload` (`prompt: String`, `#[serde(default)]` on `engine`, `from`, `refs`, `output`, `verbose`).
- `.route("/api/history", get(history))` and `.route("/api/fork", post(fork))`, both beside `/api/edit`.
- `async fn history` — serializes `view.output_dir`, `view.nodes`, `view.missing` and `view.roots` from the same `HistoryView` the CLI prints; an unreadable store returns `{ "success": false, "error": … }` at HTTP 200, matching every other handler in the file.
- `async fn fork` — trust boundary first (`from`, `refs` and `output` each pass `is_plain_filename`), then the 16-reference guard, the empty-prompt refusal, engine resolution, source resolution through `history::resolve_fork_source` (a node's filename reduced through `output_dir_file` before it is joined, T-02-10), reference resolution, the reduced output filename, the same `ImageGenerationRequest` `edit` builds, and `result_items_json` verbatim.

**`web_interface.html`**

- `#historyForm` — `Iteration history` section title, `#history_node` select (`size="8"`), help text, `#history_prompt`, `#history_engine`, `#history_refs`, `#history_output`, and the `Fork selected node` CTA. Composed only from `.form-grid`, `.form-group.full-width`, `.section-title`, `.help-text` and `.generate-btn`.
- `loadHistory()` — fetches `/api/history`, clears the select, prepends a disabled `{N} node(s) in history are missing from disk` option when `missing > 0`, then the `Pick a node to fork from` sentinel, then one option per node built by `historyNodeLabel` with the same grammar as the CLI tree (leading-space connectors, `✗ ` for missing nodes, the 32-character prompt field, `YYYY-MM-DD HH:MM`). Roots and children are walked in API order — no client-side sort. The CTA is disabled when there are no roots.
- `executeFork()` — posts explicit form values to `/api/fork`, shows `#loading` exactly as the generate/edit handlers do, writes per-result lines and `<img>` previews into the shared `#output-content` / `#image-preview` block, prints `❌ Fork failed: {error}` on a top-level failure, and calls `loadHistory()` on success.
- The submit listener, `loadHistory()` in `DOMContentLoaded`, and `#history_engine` fed by the same engine list `loadEngines()` already builds. `executeGeneration`, `executeEdit`, `buildCliCommand`, `loadSettings`, `saveSettings` and `initializeImageUpload` are untouched.

**`tests/web.rs`** — 11 new tests: `history_endpoint_returns_nested_tree`, `history_endpoint_orders_roots_newest_first` (both orderings), `history_endpoint_empty_dir`, `history_endpoint_reports_unreadable_store`, `fork_endpoint_returns_result_items` (including the exact result-item key set), `fork_endpoint_rejects_traversal_from`, `fork_endpoint_rejects_traversal_output`, `fork_endpoint_rejects_empty_prompt`, `fork_endpoint_rejects_more_than_sixteen_images`, `fork_endpoint_reports_unknown_from`, `history_form_is_served_with_fork_controls`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing critical functionality] `/api/fork` accepted an empty prompt**
- **Found during:** Task 2 (`fork_endpoint_rejects_empty_prompt`)
- **Issue:** the plan's behaviour list requires an empty prompt to be rejected, but `edit` has no such validation and the mock answered the request successfully, so the fork would have spent a provider call on an empty instruction.
- **Fix:** an explicit `payload.prompt.trim().is_empty()` guard returning `{ "success": false, "error": "Prompt cannot be empty." }` — the CLI's existing sentence for the same condition — placed after the reference-count guard and before engine resolution. `edit` itself is untouched and byte-identical.
- **Files modified:** src/web.rs

### Documentation discrepancy

The plan's Task 3 acceptance pins the pre-existing CSS rule count at 21 (`rg -c '^\s{8}\.[a-z-]+ \{' web_interface.html`); the actual pre-existing count is 22 (there are two `.image-preview {` rules). The count is unchanged at 22 after this plan's edit, so the "no new CSS rule" rule holds; the number in the plan is stale.

## Verification Evidence

```
cargo fmt --check   -> exit 0
cargo clippy --all-targets -> 0 warnings, 0 errors
cargo test --test web -> ok. 30 passed; 0 failed   (11 new, all listed above)
cargo test           -> lib 67 ok, cli 36 ok, core 7 ok, web 30 ok
rg -c 'fn result_items_json' src/web.rs        -> 1
rg -c 'result_items_json\(' src/web.rs         -> 4
rg -c 'roots' src/web.rs                      -> 1 (the envelope key only)
rg -c "fetch\('/api/fork'"   web_interface.html -> 1
rg -c "fetch\('/api/history'" web_interface.html -> 1
rg -c 'historyBtn|history_node|historyForm' web_interface.html -> 12
rg -c 'executeGeneration|loadSettings|initializeImageUpload' web_interface.html -> 6
rg -c '^\s{8}\.[a-z-]+ \{' web_interface.html -> 22 (unchanged; no new CSS rule)
rg -n 'sort' web_interface.html -> only the two "do not re-sort" comments
```

Manual confirmation in a browser was not possible in this environment (no browser tooling is configured in this repo); the serve test proves the edited HTML is embedded and reaches the client, and the endpoint tests prove the two routes.

## Known Stubs

None.

## Threat Flags

None — T-02-09 (traversal through `from`/`refs`/`output`), T-02-10 (a record filename joined under `output_dir`), T-02-11 (reference count / payload size) and T-02-12 (failure wording) are all covered by the guards above and asserted by the traversal, 16-reference and unknown-from tests.

## Self-Check: PASSED

- `.planning/phases/02-branching-iteration-history/02-04-SUMMARY.md` — written
- `src/web.rs` contains `struct ForkPayload`, `.route("/api/fork", post(fork))`, `.route("/api/history", get(history))`, `async fn history`, `async fn fork`
- Commit `f688365` is an ancestor of HEAD; no tracked deletions in the commit
