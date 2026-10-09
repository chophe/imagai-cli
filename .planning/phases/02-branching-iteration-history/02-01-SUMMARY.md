---
phase: 02-branching-iteration-history
plan: 01
type: execute
wave: 1
subsystem: history-store
tags: [persistence, lineage, sidecar, core-hook]
requires: []
provides:
  - src/history.rs (record/node/view model, sidecar read/write, fork-source resolver)
  - history::record_result hook on generate_image_core
  - utils::datetime_iso as the crate's single timestamp source
affects: [src/core.rs, src/utils.rs, src/web.rs, src/cli.rs, src/tui.rs]
tech-stack:
  added: []
  patterns: [JSON sidecar store, read-modify-write, lazy adoption, single recorder on the shared pipeline]
key-files:
  created: [src/history.rs]
  modified: [src/lib.rs, src/core.rs, src/utils.rs, src/web.rs, tests/core.rs, tests/web.rs, .gitignore]
decisions:
  - "D-08/D-09/D-11 as written: one .imagai-history.json in output_dir, plain generations are roots, whole-sidecar read-modify-write per save, no locking/journal/compaction"
  - "record_result returns Ok(\"\") instead of writing when the saved path's parent is not output_dir — the D-22 guard compares Path values directly, no canonicalisation"
  - "Tree derivation indexes records by parent and skips already-seen ids; records stranded in a cyclic parent chain are listed as their own roots so no record ever disappears (D-20)"
  - "resolve_fork_source treats an id-shaped input (≤8 lowercase-hex chars) as an id prefix, so an ordinary filename is never mistaken for a node id"
  - "Auto-adopted files record empty prompt/model; renderers substitute (unknown prompt)/(unknown model) rather than inventing lineage"
metrics:
  duration: "~25m"
  completed: 2026-10-09T18:40:00Z
  tasks: 1
  commits: 1
  files: 8
plan_head_before: c16062d
plan_head_after: 9a8f297
status: complete
actuals:
  tokens: 34000
  tasks: 1
  commits: 1
estimate:
  tokens: 33000
  tasks: 3
  confidence: low
---

# Phase 2 Plan 01: Sidecar store + recording hook Summary

**One-liner:** `.imagai-history.json` sidecar in `output_dir` with a single recorder on `generate_image_core`, so every surface writes the same lineage and lazy adoption recovers anything missed.

## What Was Built

**`src/history.rs` (new)** — the project's first persistence layer:

- `HISTORY_FILENAME`, `history_path`, `HistoryRecord` (`#[serde(default)]` on every field except `id`/`filename`), `HistoryNode` (read-time `missing` + nested `children`), `HistoryView` (both `Serialize` — one serializer feeds CLI `--json` and the web API, D-19).
- `read_records` — absent sidecar reads empty; a malformed one bails with the contract sentence `Could not read history at {path}: {why}`, never a silent empty tree.
- `append_records` — D-11 read-modify-write, `to_string_pretty`, no locking/journal/compaction.
- `new_node_id` — 8 lowercase hex from `uuid::Uuid::new_v4()`, regenerated on collision.
- `load_view` — parent→children index in a single pass (T-02-02): a record whose parent is absent/unknown/self becomes a root; children oldest-first, roots newest-first by `(created, id)`; `missing` computed per read via `node_path`, never persisted (D-20).
- `record_result` — D-22 guard first (`saved_path.parent() == dir`), filename from `file_name()` only, `created` from the shared `datetime_iso`, parent matched from the source's own basename (D-09).
- `resolve_fork_source` / `ForkSource` / `node_path` — D-12 order: unique id prefix → exact filename → existing file (auto-adopted as a root when it sits in `dir`, untracked when it does not) → bail. The three Copywriting Contract sentences are produced here and nowhere else. `node_path` is the T-02-01 reduction every consumer uses before joining under `output_dir`.

**`src/core.rs`** — one `record_result` call inside the success arm, after `saved_path` is set, with the non-fatal `let _ =` and its comment. Single recording point for CLI, TUI and web.

**`src/utils.rs`** — `pub fn datetime_iso` + `civil_from_days` relocated from `web.rs` (arithmetic and format untouched); `src/web.rs::list_images` now calls `crate::utils::datetime_iso` at both sites and no longer defines either function.

**`.gitignore`** — `.imagai-history.json` added beside `/generated_images`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] `edit_writes_child_node_under_its_source` used `to_string_lossy` on a `PathBuf`**
- **Found during:** Task 1 (first compile of the RED tests)
- **Issue:** `saved.to_string_lossy()` picked up rustix's `Arg::to_string_lossy` through the `ImageGenerationRequest` inference, failing compilation.
- **Fix:** `Some(saved.to_string())`.
- **Files modified:** tests/core.rs

**2. [Rule 2 - Missing critical functionality] Records stranded in a cyclic `parent` chain vanished from the view**
- **Found during:** Task 1 (`load_view_treats_unknown_parent_as_a_root_and_survives_a_cycle`)
- **Issue:** The first derivation passed only over roots, so two records whose `parent` fields pointed at each other were reachable from no root and disappeared from the tree — a truthfulness violation (D-20) even though derivation terminated.
- **Fix:** After building from the roots, any record still unseen is listed as its own root (newest-first) and built too; the builder also skips children already reached, so a cycle emits each record exactly once.
- **Files modified:** src/history.rs (`load_view`, `build_roots`, `build_node`)

**3. [Rule 1 - Bug] A bare filename of an untracked file in `output_dir` could not be auto-adopted**
- **Found during:** Task 1 (`resolve_fork_source_adopts_an_untracked_file_as_a_root`)
- **Issue:** Step (3) as literally written tested `Path::new(from).is_file()`, which resolves against the process CWD — a bare filename inside `output_dir` is not a file from there, so `fork --from <filename>` fell through to the unknown-name error instead of adopting (D-12).
- **Fix:** Step (3) tries the literal path first; when `from` carries no separator and `dir.join(from)` is a file, that file is adopted as a root and returned. Separator-carrying values are never joined onto `dir` — `is_plain_filename` refuses them on the web surface, and the resolver does not silently reduce them.
- **Files modified:** src/history.rs (`resolve_fork_source`)

### Test-fixture adaptations (recorded, not deviations in behaviour)

`openrouter_chat_image_model_saves_png` and `generate_endpoint_returns_image_preview` asserted "exactly one file in output_dir". The sidecar is a new resident of that directory by design, so both fixtures now filter `.imagai-history.json` out of the listing. No assertion semantics were weakened.

## Verification Evidence

```
cargo fmt --check                       -> exit 0
cargo clippy --all-targets              -> 0 warnings, 0 errors
cargo test --test core generate_writes_history_sidecar -- --exact       -> ok (1 passed)
cargo test --test core edit_writes_child_node_under_its_source -- --exact -> ok (1 passed)
cargo test --test core out_of_dir_result_is_not_recorded -- --exact     -> ok (1 passed)
cargo test --lib                         -> ok. 58 passed; 0 failed
cargo test --test web                    -> ok. 19 passed; 0 failed
cargo test                               -> lib 58 ok, cli 26 ok, core 7 ok, web 19 ok
rg -n 'unwrap\(|expect\(' src/history.rs | awk '$1<420'  -> (none outside #[cfg(test)])
```

The manual confirmation with a real engine was not possible in this environment (no API key); the library-level e2e tests cover the same paths against the mock.

## Known Stubs

None.

## Threat Flags

None — the plan's threat register (T-02-01…T-02-04) was mitigated in code: `node_path` reduces every record filename to its final component before any join, derivation is a single indexed pass with a visited set, and the store-error sentence names only the operator's own path.

## Self-Check: PASSED

- `.planning/phases/02-branching-iteration-history/02-01-SUMMARY.md` — written
- `src/history.rs`, `pub mod history;` in `src/lib.rs`, `datetime_iso` in `src/utils.rs` — present
- Commit `9a8f297` is an ancestor of HEAD; no tracked deletions in the commit
