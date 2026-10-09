---
phase: 02-branching-iteration-history
plan: 02
type: execute
wave: 2
subsystem: cli-surface
tags: [cli, history, fork, tree-grammar]
requires: [02-01]
provides:
  - imagai history (tree + --json) and imagai fork --from
  - imagai edit --from
  - the reference ASCII tree grammar the TUI and SPA match
affects: [src/cli.rs, src/history.rs]
tech-stack:
  added: []
  patterns: [clap subcommand + cmd_ handler, pure render_history_text over the shared HistoryView, single resolver for --from]
key-files:
  created: []
  modified: [src/cli.rs, src/history.rs, tests/cli.rs]
decisions:
  - "Indent is three spaces per ancestor level (\u0022   \u0022.repeat(depth)) then the connector: the UI-SPEC grammar table and both plans specify this; the doc's example block omits the depth-1 indent and is the only contradiction"
  - "cmd_fork prints the contract sentence with a colored [Error] tag and exits 1, matching cmd_edit's usage-error convention, so resolve_fork_source stays the only producer of the wording"
  - "history::node_id_for looks up the node id by filename so the fork line can name the adopted node's id without widening the ForkSource enum"
metrics:
  duration: "~20m"
  completed: 2026-10-09T19:10:00Z
  tasks: 2
  commits: 1
  files: 3
plan_head_before: 9a8f297
plan_head_after: 592c440
status: complete
actuals:
  tokens: 28000
  tasks: 2
  commits: 1
estimate:
  tokens: 33000
  tasks: 3
  confidence: low
---

# Phase 2 Plan 02: CLI history + fork Summary

**One-liner:** `imagai history` renders one indented ASCII tree per root session (`--json` for scripts) and `imagai fork --from` / `imagai edit --from` start an edit from any past image without losing the original branch.

## What Was Built

**`src/cli.rs`**
- `HistoryArgs { json }`, `ForkArgs { from, prompt, refs, engine, output, verbose }`, `Commands::History` / `Commands::Fork`, dispatch arms in `run`.
- `cmd_history` — `history::load_view(&settings.output_dir)?` (the unreadable-store sentence propagates verbatim), then `println!("{}", serde_json::to_string_pretty(&view)?)` for `--json` or `render_history_text` for the tree.
- `render_history_text` — pure function of `HistoryView`: header `🌳 Iteration history — {dir} ({N} node{s}[, {S} session{s}])` with the session clause dropped at exactly one session, one blank line after the header and between sessions, three spaces of indent per ancestor level, `+- ` / `` `- `` connectors, `✗ ` for missing files, the fixed 32-character prompt column (29 chars + `...` when longer, else padded; `(unknown prompt)` when a lazily adopted file has no lineage), unclipped filename, `YYYY-MM-DD HH:MM`, and the singular/plural `[warning]` line omitted entirely at zero.
- `cmd_fork` — resolves `--from` (printing `[Error] {sentence}` + exit 1 on failure), prints `🔀 Forking from {filename} (node {id8})`, then the existing edit path unchanged: the `🖼️ Editing image with engine:` / `📜 Change:` prints, the D-05 capability gate, the same `ImageGenerationRequest` build with `source_image: Some(...)`, `print_edit_results`, and the edit-path exit guard.
- `resolve_from` — the one resolution step shared by `fork` and `edit`; `print_fork_line` names the branch point, looking the node id up through the new `history::node_id_for`.
- `EditArgs::from` — `--from` on `edit`, used only when `--image` is absent (D-03's "explicit path still wins" is unchanged).

**`src/history.rs`** — added `pub fn node_id_for(dir, filename) -> Option<String>`.

**`tests/cli.rs`** — 10 new e2e tests: `history_tree_prints_connectors` (header, 32-wide field, clipped `...` case, padded case, both connectors, timestamp format), `history_tree_marks_missing_nodes` (marker position, singular and plural sentences), `history_empty_state` (exact stdout, no header, no warning), `fork_from_node_id_records_child`, `fork_from_filename_of_adopted_file` (adoption + source bytes untouched), `fork_keeps_both_branches_visible` (BRCH-02), `fork_rejects_unknown_source`, `fork_rejects_deleted_file_of_recorded_node`, `fork_rejects_more_than_sixteen_images`, `edit_from_resolves_a_past_node_without_an_image_flag`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `fork_from_filename_of_adopted_file` compared a `usize` length with a `u64` metadata length**
- **Found during:** Task 2 (first compile of the RED tests)
- **Issue:** `assert_eq!(read.len(), metadata.len())` does not compile; the intent (bytes unchanged) needed a before/after comparison.
- **Fix:** capture the source bytes before the fork and compare afterwards.
- **Files modified:** tests/cli.rs

### Documented decision (not a deviation in behaviour)

The UI-SPEC's *example block* shows a depth-1 row with no leading indent, while its own grammar table ("3 spaces per ancestor level, then the connector") and both plan files specify `"   ".repeat(depth)`. The example block is self-inconsistent with the table; I followed the table and the plans, which agree with each other, and pinned the choice with `history_tree_prints_connectors`.

### Observation (not a deviation)

One `cargo test` run reported a single lib-test failure with no failing test named in the captured output, immediately after a `cargo fmt` rewrite of the same files. Twenty subsequent runs (`cargo test`, `cargo test --lib`, `cargo test --lib history::`, 12× repeat sweeps) are clean, so the failure did not reproduce and no test was found to be flaky.

## Verification Evidence

```
cargo fmt --check                            -> exit 0
cargo clippy --all-targets                   -> 0 warnings, 0 errors
cargo test --test cli                        -> ok. 36 passed; 0 failed
cargo test                                   -> lib 58 ok, cli 36 ok, core 7 ok, web 19 ok
Binary smoke test (IMAGAI__OUTPUT_DIR=/tmp/hist-smoke):
  imagai history    -> header, blank line, `✗ 1a2b3c4d … cat.png … 11:40`,
                       `   +- ✗ 2b3c4d5e …`, `   \`- ✗ 3c4d5e6f …`,
                       blank line, `[warning] 3 nodes point at files …`
  imagai history --json -> the documented object with output_dir/nodes/missing/roots
                       and nested children
  imagai history (empty dir) -> exactly the empty-state line
  rg "No history node or image named|which is no longer in" src/cli.rs -> no match
                       (the sentences live only in history::resolve_fork_source)
```

Manual confirmation with a real engine was not possible in this environment (no API key); the CLI e2e suite covers the same paths against the mock.

## Known Stubs

None.

## Threat Flags

None — T-02-05/T-02-06 are mitigated by the resolver's guards (`--from <id8>` joins only through `node_path`'s reduction; the fork path appends records and deletes nothing) and are asserted by `fork_from_filename_of_adopted_file` and `fork_keeps_both_branches_visible`.

## Self-Check: PASSED

- `.planning/phases/02-branching-iteration-history/02-02-SUMMARY.md` — written
- `src/cli.rs` contains `HistoryArgs`, `ForkArgs`, `Commands::History`, `Commands::Fork`, `cmd_history`, `cmd_fork`, `resolve_from`, `render_history_text`, `from: Option<String>` on `EditArgs`
- Commit `592c440` is an ancestor of HEAD; no tracked deletions in the commit
