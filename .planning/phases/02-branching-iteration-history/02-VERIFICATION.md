---
phase: 02-branching-iteration-history
verified: 2026-10-10T03:52:20Z
status: gaps_found
score: 21/22 truths verified
covered_files:
  - .planning/phases/02-branching-iteration-history/02-01-PLAN.md
  - .planning/phases/02-branching-iteration-history/02-01-SUMMARY.md
  - .planning/phases/02-branching-iteration-history/02-02-PLAN.md
  - .planning/phases/02-branching-iteration-history/02-02-SUMMARY.md
  - .planning/phases/02-branching-iteration-history/02-03-PLAN.md
  - .planning/phases/02-branching-iteration-history/02-03-SUMMARY.md
  - .planning/phases/02-branching-iteration-history/02-04-PLAN.md
  - .planning/phases/02-branching-iteration-history/02-04-SUMMARY.md
  - .planning/phases/02-branching-iteration-history/02-ui-spec.md
  - .planning/phases/02-branching-iteration-history/02-REVIEW.md
  - .planning/phases/02-branching-iteration-history/02-REVIEW-FIX.md
  - .planning/phases/02-branching-iteration-history/02-UAT.md
  - src/history.rs
  - src/core.rs
  - src/cli.rs
  - src/tui.rs
  - src/web.rs
  - src/utils.rs
  - src/lib.rs
  - web_interface.html
  - tests/cli.rs
  - tests/core.rs
  - tests/web.rs
  - .gitignore
behavior_unverified: 0
overrides_applied: 0
head: 7f0d8204c5f2e675dc3ce1b2b4950e516fe3d327
---

# Phase 02: Branching & Iteration History — Verification Report

**Phase Goal:** Users can view an image's iteration lineage and fork any past
result into a new edit without losing the original, so they can branch instead
of starting over.
**Verified:** 2026-10-10
**Status:** gaps_found
**Re-verification:** No — initial verification (the verify step had never run;
`verification.status` returned `missing` before this report)

Config used: `workflow.verifier = true`, `workflow.human_verify_mode =
"end-of-phase"`, `workflow.nyquist_validation = false`. Verification ran against
the LIVE code on `main` at `7f0d820`, not against plan text.

## Goal Achievement

### Observable Truths

| # | Truth (source plan) | Status | Evidence |
|---|---------------------|--------|----------|
| 1 | After any generate or edit saving into `output_dir`, a sidecar `.imagai-history.json` exists there holding a node for the saved file (02-01) | ✓ VERIFIED | `src/core.rs:168-176` — the single `history::record_result` call in the success arm, non-fatal by design; `tests/core.rs#generate_writes_history_sidecar` passes (asserts `records.len() == 1`, prompt, model `dall-e-3`, 8-hex id, `parent` null, zero `/v1/images/edits` requests); live run: `LS sidecar written at .imagai-history.json` |
| 2 | A plain generation becomes a session root and an edit becomes a child of the node whose filename matches its source (D-09) (02-01) | ✓ VERIFIED | `src/history.rs:329-373` `record_result` derives `parent` via `record_for_filename` (newest match, CR-05); `tests/core.rs#edit_writes_child_node_under_its_source` passes; live run B/D: `edit --from` produced a child under the source root |
| 3 | A history node whose file was deleted externally is kept and marked `missing`, never silently dropped (D-20) (02-01) | ✓ VERIFIED | `src/history.rs:204+` `missing` is `!dir.join(filename).is_file()` computed at read time, never persisted; `history::tests::load_view_marks_absent_files_missing_and_keeps_them` passes; live run E: deleted edit file → `✗ 10b1fe48` row kept + `[warning] 2 nodes point at files that are no longer in the output directory.` |
| 4 | Recorded filenames are relative to `output_dir` only, so moving the folder keeps history valid (D-23) (02-01) | ✓ VERIFIED | Records store `file_name()` only (`src/history.rs:366-368`); live run M: `cp -R` the whole dir to `/tmp/imagai_moved` → `imagai history` printed both roots with exit 0 |
| 5 | Only results that land in the configured `output_dir` are recorded; an explicit out-of-dir output path stays untracked (D-22) (02-01) | ✓ VERIFIED | `src/history.rs:329-345` guard: record only when `saved_path.parent() == dir`; `tests/core.rs#out_of_dir_result_is_not_recorded` passes; live run N: `-o ../imagai_escape.png` succeeded on disk but the sidecar stayed at 2 records |
| 6 | `imagai history` prints one indented ASCII tree per root with id8, a 32-char prompt column, filename and timestamp, roots newest-first, children oldest-first (02-02) | ✓ VERIFIED | `src/history.rs:375-414` `prompt_field`/`timestamp_field`, `src/history.rs:421+` `history_row_text`; `tests/cli.rs#history_tree_prints_connectors` passes; live run (two children under one root) emitted `   +- …` / `   `- …` with three-space depth-1 indent |
| 7 | `imagai history --json` emits output_dir, nodes, missing, nested roots with children (02-02) | ✓ VERIFIED | `src/cli.rs:628-629` `history::load_view` serialized; live run 4 emitted the exact envelope with `"nodes": 1, "missing": 0, "roots":[{…}]` |
| 8 | `imagai fork --from <image\|node-id> -p "<change>"` starts an edit whose source is the named past image, references the same file rather than copying it, saves under `<stem>-edit.<ext>` (02-02) | ✓ VERIFIED | `src/cli.rs:533-612` `cmd_fork` → `resolve_from` → `history::resolve_fork_source` → shared `generate_image_core`; `tests/cli.rs#fork_from_node_id_records_child` and `#fork_from_filename_of_adopted_file` pass; live run 5 printed `🔀 Forking from …` and saved `…-edit.png` |
| 9 | Forking never deletes or detaches anything: the original branch remains intact and both branches are visible afterwards (D-14, D-15) (02-02) | ✗ GAP | The named original stays intact (`tests/cli.rs#fork_keeps_both_branches_visible` passes; live run D showed both), but a SECOND fork from the same source collides on filename — see G-1 below. The truth's literal clause holds; the guarantee it encodes does not. |
| 10 | Stepping back is choosing an ancestor node id as `--from`, which is just a fork from that node — forward-only, nothing rewound (02-02) | ✓ VERIFIED | `resolve_fork_source` step (1) `src/history.rs:492-506` accepts any id prefix; no rewind/delete path exists in `src/history.rs` or `src/cli.rs`; live run 5 forked from an ancestor and both generations survived |
| 11 | Unknown `from`, a deleted recorded file and an unreadable store each print the exact Copywriting Contract sentence on stderr and exit 1 (02-02) | ✓ VERIFIED | `src/history.rs:505-563` — the three verbatim sentences produced in exactly one place (plus `Could not read history at` at `:140`/`:144`); live run G `Error: Could not read history at …` exit 1; live run H `History node 4424ac29 points at '…', which is no longer in ….` exit 1; live run I `Ambiguous node id prefix 'aaa' — candidates: aaa11111, aaa22222` exit 1 |
| 12 | The TUI gains a History tab at TABS index 2 between Edit and Engines, listing the same nodes in the same order as the CLI tree (D-18, BRCH-04) (02-03) | ✓ VERIFIED | `src/tui.rs:43` `const TABS: [&str; 5] = ["Generate","Edit","History","Engines","About"]`; `tui::tests::tabs_has_history_at_index_two` passes; both renderers walk the same `history::HistoryView` |
| 13 | The tab shows a bordered tree pane and a Node detail pane in the 55/45 split the Edit tab uses, with the selected row marked `▶ ` in Cyan+BOLD and unselected rows Gray (02-03) | ✓ VERIFIED | `src/tui.rs:1126` `Layout::horizontal([Percentage(55), Percentage(45)])`; `src/tui.rs:1136-1175` `▶ ` / `Color::Cyan` + `BOLD`, unselected `Color::Gray` |
| 14 | Enter on the History tab forks the selected node: sets `edit_source`, switches to the Edit tab, pushes the same `Source image: {name}` info log (D-14, D-18) (02-03) | ✓ VERIFIED | `src/tui.rs:1054-1088` `history_select`; `tui::tests::history_enter_forks_selected_node_into_edit` and `#history_enter_completes_pending_pick` pass |
| 15 | The Edit tab source picker gains a first row ` Pick from history… `; selecting it moves to the History tab with a pending pick and Enter writes the chosen filename into `edit_source` (02-03) | ✓ VERIFIED | `src/tui.rs:1090-1094` `open_history_pick`, `PickerOrigin::History`; `tui::tests::picker_history_entry_moves_to_history_tab` passes |
| 16 | Escaping the picker or the History tab leaves `edit_source` untouched (02-03) | ✓ VERIFIED | `tui::tests::picker_and_history_escape_leave_source_unchanged` passes; `src/tui.rs:1096-1119` `handle_history_key` Esc branch returns to tab 1 without assigning |
| 17 | Re-indexing TABS from 4 to 5 re-points every `match self.tab` arm and replaces the `_ =>` catch-all with an explicit index-4 arm (02-03) | ✓ VERIFIED | `rg -n '_ =>' src/tui.rs | rg 'tab|about'` returns nothing — the dispatch is exhaustive over 0..=4 |
| 18 | `GET /api/history` returns `{success, output_dir, nodes, missing, roots:[…]}` with the same nested node shape as the CLI `--json` (D-19, BRCH-04) (02-04) | ✓ VERIFIED | `src/web.rs:469-504` handler serializes `view.roots` directly (no second node struct); live probe returned `nodes`, `missing`, `output_dir`, `roots`, `success` with node keys `['children','created','filename','id','missing','model','parent','prompt']`; child `parent` equals root `id` |
| 19 | `POST /api/fork` accepts `from`, `prompt`, `engine`, `output`, `refs` and its response is `result_items_json` verbatim (02-04) | ✓ VERIFIED | `src/web.rs:506+` handler; live probe: `/api/fork` result key set `['error','image_b64_json','image_data','index','saved_path','success']` — byte-identical to `/api/edit`'s key set |
| 20 | Every path-shaped field on `/api/fork` passes `is_plain_filename`; a separator-carrying value is refused before any read or network call (02-04) | ✓ VERIFIED | `src/web.rs:271` `is_plain_filename` applied to `from`, `refs`, `output` first; live probes: `../../.env`, `..\..\env`, `../../escaped.png`, `../outside.png` all `success:false` with zero mock requests and no escaped file on disk |
| 21 | The SPA renders a history view composed only from existing CSS classes, with a `Fork selected node` CTA, a `Pick a node to fork from` sentinel, and a disabled option reporting the missing count when `missing > 0` (02-04) | ✓ VERIFIED | `web_interface.html:563-600` markup; `loadHistory` emits the disabled `{missing} node(s) in history are missing from disk` option (singular at 1) and `button.disabled = result.roots.length === 0`; live probe found every token; CSS rule count `rg -c '^\s{8}\.[a-z-]+ \{'` = 22 at every commit from `f9ec7bd` through HEAD, so no new rule |
| 22 | Forking from the browser calls `loadHistory()` on success so the new node appears without a reload (02-04) | ✓ VERIFIED | `web_interface.html:1189` `fetch('/api/fork'` → `:1237` `loadHistory()`; live probe: `/api/history` nodes 2 → 4 after a fork |

**Score:** 21/22 truths verified, 1 gap (G-1).

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/history.rs` | `HISTORY_FILENAME`, `HistoryRecord`, `HistoryNode`, `HistoryView`, `history_path`, `read_records`, `append_records`, `new_node_id`, `load_view`, `record_result`, `resolve_fork_source`, `ForkSource` | ✓ VERIFIED | All present: lines 23, 47, 62, 79, 121, 134, 157, 168, 204, 329, 487, 111; 24 `history::tests` pass |
| `src/core.rs` | Recording hook on the shared pipeline, exactly once | ✓ VERIFIED | `src/core.rs:168-176` — one call, inside the success arm after `saved_path` is set; non-fatal with a `[warning] Lineage was not recorded for …` line (WR-01) |
| `src/utils.rs` | `pub fn datetime_iso` + `civil_from_days` relocated from `web.rs` | ✓ VERIFIED | `src/utils.rs:345`, `:367`; `rg 'fn datetime_iso\|fn civil_from_days' src/web.rs` returns nothing; `src/web.rs` still calls `crate::utils::datetime_iso` at 2 sites; `src/history.rs` uses it at 366 and 468 |
| `src/lib.rs` | `pub mod history;` in flat alphabetical position | ✓ VERIFIED | `src/lib.rs:6`, between `core` and `models` |
| `tests/core.rs` | `generate_writes_history_sidecar`, `edit_writes_child_node_under_its_source`, `out_of_dir_result_is_not_recorded` | ✓ VERIFIED | `tests/core.rs:168`, `:231`, `:294` — all pass |
| `src/cli.rs` | `HistoryArgs`, `ForkArgs`, `cmd_history`, `cmd_fork` | ✓ VERIFIED | `src/cli.rs:166`, `:196`, `:628`, `:533`; `history --json` and `fork --from` flags present in `--help` |
| `tests/cli.rs` | `history_tree_prints_connectors` + fork e2e | ✓ VERIFIED | 12 history/fork tests at `tests/cli.rs:690..1142`, all pass |
| `src/tui.rs` | `TABS[5]`, `HistoryState`, `HistoryRow`, `history_rows`, `flatten_nodes`, `load_history`, `draw_history_tab`, `handle_history_key`, `history_move`, `history_select`, `open_history_pick`, `scroll_selection`, `PickerOrigin` | ✓ VERIFIED | `src/tui.rs:43`, 1004, 1032, 1054, 1090, 1096, 1120, 1132, 1190, 1265, 925; 9 `tui::tests` history tests pass |
| `src/web.rs` | `/api/history` route + handler, `/api/fork` route + handler, `ForkPayload` | ✓ VERIFIED | `src/web.rs:141-142` routes, `:469` handler, `:506` handler, `struct ForkPayload` beside `EditPayload` |
| `web_interface.html` | `historyForm`, `history_node`, `historyBtn`, `history_prompt`, `history_engine`, `history_refs`, `history_output`, `loadHistory`, `executeFork` | ✓ VERIFIED | Lines 563, 568, 598, 574, 580, 588, 594, 1122, 1166; all present in the served HTML |
| `tests/web.rs` | 10 history/fork endpoint + form tests | ✓ VERIFIED | `tests/web.rs:483..851`, all pass; suite reports 35 passed / 0 failed |
| `.gitignore` | `.imagai-history.json` entry, pre-existing entries unchanged | ✓ VERIFIED | `.gitignore:8` with its comment; `/target`, `/generated_images`, `.env`, `*.env`, `.DS_Store` intact |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `src/core.rs` | `src/history.rs` | every successful save into `output_dir` calls `history::record_result` | ✓ WIRED | `src/core.rs:168` — the single recording point, so CLI/TUI/web cannot drift (BRCH-04) |
| `src/history.rs` | `src/utils.rs` | node `created` from the one shared timestamp helper | ✓ WIRED | `src/history.rs:366`, `:468` call `crate::utils::datetime_iso`; `web.rs` defines no second source |
| `src/history.rs` | `.imagai-history.json` | read-modify-write of the whole sidecar per save (D-11) | ✓ WIRED | `src/history.rs:157-165` `append_records` — read, extend, pretty-print, temp-file + `rename` (WR-01) |
| `src/cli.rs` | `src/history.rs` | `cmd_history` → `load_view`, serializing the same `HistoryView` | ✓ WIRED | `src/cli.rs:629`; `HistoryView` derives `Serialize` (`src/history.rs:79`) |
| `src/cli.rs` | `src/history.rs` | `cmd_fork` → `resolve_fork_source`, the single resolver | ✓ WIRED | `src/cli.rs:505`; TUI and web route through the same function |
| `src/cli.rs` | `src/core.rs` | fork builds the same request and calls `generate_image_core` | ✓ WIRED | `src/cli.rs:612`; no parallel pipeline (D-13) |
| `src/tui.rs` | `src/history.rs` | the History tab renders a `HistoryView` from `load_view` | ✓ WIRED | `src/tui.rs:1004`; `load_view_treats_unknown_parent_as_a_root_and_survives_a_cycle` protects the walker |
| `src/tui.rs` | `src/tui.rs` | fork reuses Edit-tab state rather than a second request path | ✓ WIRED | `history_select` assigns `self.edit_source` and `self.tab = 1` (`src/tui.rs:1054-1088`) |
| `src/web.rs` | `src/history.rs` | history and fork handlers call `load_view` / `resolve_fork_source` | ✓ WIRED | `src/web.rs:469`, `:506+` — same store the CLI reads |
| `src/web.rs` | `src/web.rs` | fork response is `result_items_json` | ✓ WIRED | `src/web.rs:281` definition, called by both `edit` and `fork`; live probe confirmed identical key sets |
| `web_interface.html` | `src/web.rs` | `loadHistory` fetches `/api/history`, `executeFork` posts `/api/fork`, then `loadHistory()` again | ✓ WIRED | `web_interface.html:1126`, `:1189`, `:1237` |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `src/core.rs` save arm | `record_result` args | real `saved_path` + real prompt + real model name | ✓ Yes — live run wrote `.imagai-history.json` with a real record per saved image | ✓ FLOWING |
| `src/history.rs` store | `records` | `fs::read_to_string` of the sidecar | ✓ Yes — live `history --json` printed the persisted envelope | ✓ FLOWING |
| `src/history.rs` view | `roots` | the sidecar, reduced + ordered | ✓ Yes — live tree rendered 4 nodes with correct parent links | ✓ FLOWING |
| `src/web.rs` `/api/history` | `view.roots` | `load_view` | ✓ Yes — live HTTP 200 with nested children and `parent` == root `id` | ✓ FLOWING |
| `src/web.rs` `/api/fork` | `source_image` | `resolve_fork_source` → `is_plain_filename`-guarded | ✓ Yes — live POST saved `…-edit.png` and returned `image_data` | ✓ FLOWING |
| `web_interface.html` picker | `#history_node` options | `GET /api/history` roots, no client re-sort | ✓ Yes — live GET populated the select; `sort` appears only in a comment saying not to | ✓ FLOWING |
| `src/tui.rs` History rows | `state.rows` | `history_rows(&view)` | ✓ Yes — unit-asserted connector/`✗`/`… n more` rendering | ✓ FLOWING |

### Requirements Coverage

| Requirement | Source Plans | Description | Status | Evidence |
|-------------|--------------|-------------|--------|----------|
| BRCH-01 | 02-02, 02-03, 02-04 | View iteration history | ✓ SATISFIED | CLI tree + `--json`, TUI History tab, `GET /api/history`; 3 surfaces all consume the same `HistoryView` |
| BRCH-02 | 02-02, 02-03, 04 | Fork a past image into a new edit | ✓ SATISFIED | `fork --from`, TUI Enter, `POST /api/fork` — all through `resolve_fork_source` + `generate_image_core` |
| BRCH-03 | 02-02, 02-03, 04 | Keep the original when forking | ✓ SATISFIED (with G-1) | The named original is never modified; `-edit.<ext>` naming + `fork_keeps_both_branches_visible` prove it. G-1 is a *sibling* collision, not damage to the original |
| BRCH-04 | 02-01, 02-03, 02-04 | Same history across all surfaces | ✓ SATISFIED | One recorder (`src/core.rs:168`), one store, one serializer — CLI/web key sets confirmed identical live |

No orphaned requirement IDs. None of the four is claimed by only one plan.

### UI Considerations (from 02-ui-spec.md `## UI Considerations`)

Every category below was checked as a live or test-backed assertion, not taken
on the SPEC's word. Surfaces: E1 CLI tree, E2 TUI History tab, E3 web SPA
history view, E4 TUI node detail prompt row, E5 history-read / fork failure.

| Category | Element(s) | Status | Evidence |
|----------|------------|--------|----------|
| empty | E1, E2, E3, E4, E5 | ✓ VERIFIED | Live run L: `🌳 No history yet — generate or edit an image first.`; SPA renders the sentinel option `Pick a node to fork from` (`web_interface.html:1143`) and `button.disabled = result.roots.length === 0` (`web_interface.html:1158`); `history_empty_state` test passes |
| populated | E1–E5 | ✓ VERIFIED | Live run D rendered 4 nodes with id8, 32-char prompt field, filename and `YYYY-MM-DD HH:MM`; `populated` row of the SPEC grammar matches `prompt_field`/`timestamp_field`/`history_row_text` |
| zero-one-many | E1–E5 | ✓ VERIFIED | Live run K: 1 node → no session clause; live run D: 4 nodes → `(4 nodes, 2 sessions)`; singular/plural `1 node` vs `{N} nodes` driven by one branch each (`src/cli.rs` `IN-05` fix) |
| partial | E1–E5 | ✓ VERIFIED | Live run O: a record with no `prompt` rendered `(unknown prompt)` (`src/history.rs:377`); live run E: a deleted file kept its `✗ ` marker and stayed listed; live run J2: an adopted untracked file rendered `(unknown prompt)` too |
| overflow | E1–E5 | ✓ VERIFIED | `prompt_field` clips at 32 chars + `…` (`src/history.rs:379-381`); filename and id8 never clipped; TUI `… {n} more` in `Color::DarkGray` (`src/tui.rs:1607`); `history_scroll_marks_remaining_rows` passes |
| long-text | E2, E3, E5 | ✓ VERIFIED | `Paragraph::wrap(Wrap { trim: true })` on the detail pane prompt row (`src/tui.rs:1261`); web option labels and error sentences are unbounded single `<option>`/`<div>` |
| error | E1–E5 | ✓ VERIFIED | Three verbatim Copywriting Contract sentences from one function (`src/history.rs:505-563`) on stderr with exit 1 (live runs G/H/I); TUI routes through `push_log` (`history_enter_refuses_missing_node` asserts `no longer in`); web returns `{success:false,error}` (live probe) |
| loading | E1–E5 | ✓ VERIFIED (explicit negative) | No new affordance exists: `loadHistory` populates after the awaited fetch following the `loadOutputImages()` pattern (`web_interface.html:1122-1137`); the fork request reuses the existing edit spinner. Confirmed by source audit — zero new CSS rules and zero new DOM for a spinner |

Dismissed considerations: none — as the SPEC records.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| — | — | `rg 'TODO\|FIXME\|XXX\|unimplemented!\|todo!'` over `src/`, `tests/`, `web_interface.html` | — | Zero hits |
| — | — | `unwrap()`/`expect()` in `src/history.rs` outside `mod tests` | — | Zero hits — `mod tests` opens at `src/history.rs:571` and every hit is above or inside it |
| — | — | `unwrap()`/`expect()` added to `src/tui.rs` by Phase 2 | — | 22 added, all inside `#[cfg(test)] mod tests` (`src/tui.rs:2057`); 48 total hits are pre-existing |
| — | — | `_ =>` catch-all in the tab dispatch | — | Absent — `rg -n '_ =>' src/tui.rs | rg 'tab|about'` returns nothing, so `draw`/`draw_footer` are exhaustive over 0..=4 |
| — | — | New CSS rule in the SPA | — | None. Rule count 22 at `f9ec7bd` (pre-implementation) through HEAD, so 02-04 added zero |
| — | — | New dependency | — | None. `uuid` was already in `Cargo.toml`; `.gitignore` and `Cargo.toml` unchanged by this phase's code commits |

One plan-text inaccuracy, recorded for accuracy rather than as code debt: the
02-04 acceptance criterion expected `rg -c '^\s{8}\.[a-z-]+ \{'` to return "the
pre-existing count of 21". The real pre-existing count is 22 (measured at
`f9ec7bd`, before any 02-04 code). Count unchanged across the phase, so the
criterion's *intent* — no new CSS rule — holds.

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Full test suite | `cargo test` | `test result: ok. 76 passed` (lib) / `39 passed` (tests/cli.rs) / `8 passed` (tests/core.rs) / `35 passed` (tests/web.rs) / 0 doc-tests — 158 passed, 0 failed | ✓ PASS |
| Format | `cargo fmt --check` | exit 0, no diff | ✓ PASS |
| Lints | `cargo clippy --all-targets` | `Finished dev profile … in 1.45s`, zero `warning:`/`error:` lines | ✓ PASS |
| Tab dispatch exhaustive | `rg -n '_ =>' src/tui.rs \| rg 'tab\|about'` | no output | ✓ PASS |
| No production unwrap in the store | `rg -n 'unwrap\(\|expect\(' src/history.rs \| rg -v 'mod tests'` | only in-test hits (all at line ≥ 570) | ✓ PASS |
| CLI generate → sidecar | `imagai generate -p "a red cat on a sofa" --engine mock_gen` | `✅ Image 1 generated successfully! Saved to: …/a_red_cat_on_a_sofa_….png` + `.imagai-history.json` created | ✓ PASS |
| CLI edit → child node | `imagai edit --from <file> -p "make it blue"` | saved `…-edit.png`; tree shows a child under the source root | ✓ PASS |
| CLI fork by node id | `imagai fork --from <id8> -p "make it a sunset"` | `✅ …Saved to: …-edit.png`, exit 0 | ✓ PASS |
| Web `/api/history` | `GET http://127.0.0.1:8734/api/history` | 200, `success:true`, `output_dir:"imagai_uat_web3"` (bare name, no absolute path), `nodes:2` | ✓ PASS |
| Web `/api/fork` | `POST /api/fork {"from":"<id8>","prompt":"make it a sunset"}` | 200, `success:true`, `saved_path` ends `-edit.png`, `image_data` prefix `data:image/png;base64,` | ✓ PASS |
| Web traversal probes | `POST /api/fork` with `../../.env`, `..\..\env`, `../../escaped.png`, `../outside.png` | all `success:false`; mock counters unchanged; no escaped file on disk | ✓ PASS |
| Sidecar not served | `GET /api/images/.imagai-history.json` and `.imagai-history.json.tmp` | 404 `{"error":"Endpoint not found","success":false}` — `serve_image` refuses both (`src/web.rs:837`) | ✓ PASS |

### Prohibitions Review

| Prohibition | Status | Evidence |
|-------------|--------|----------|
| No second recorder per frontend | ✓ HELD | `rg -c 'record_result' src/cli.rs src/tui.rs src/web.rs` is 0 outside `core.rs` |
| No second result serializer | ✓ HELD | `rg -c 'fn result_items_json' src/web.rs` = 1, called by both `edit` and `fork` |
| No client-side re-sort of history | ✓ HELD | `sort` appears in `web_interface.html` only inside `loadHistory`'s "do not re-sort" comment |
| No new CSS rule / JS toolchain / dependency | ✓ HELD | CSS rule count unchanged at 22; no `package.json`; `Cargo.toml` untouched by 02-* code commits |
| No `unwrap`/`expect` in production `src/history.rs` or new `src/tui.rs` code | ✓ HELD | measured zero outside `mod tests` |
| No silent empty tree on an unreadable store | ✓ HELD | `history_reports_a_non_utf8_sidecar_instead_of_an_empty_tree`, `history_endpoint_reports_unreadable_store` and live run G (exit 1, named path) |
| No CWD-relative fork resolution (CR-01/CR-03) | ✓ HELD | `fork_from_a_file_that_lives_only_in_the_cwd_is_refused` and `fork_from_a_bare_file_when_cwd_is_output_dir_adopts_it` both pass |
| No path traversal from `filename` (T-02-01/T-02-09/T-02-10) | ✓ HELD | every consumer reduces through `history::node_path` (`src/web.rs:264`, `src/history.rs:184`); `load_view_reduces_a_filename_that_carries_a_parent_component` and `fork_endpoint_reduces_a_traversal_shaped_record_filename` pass |

## Gaps Summary

### G-1 — A second fork from the same source overwrites the first fork's file, and the store then holds two nodes naming one file (BLOCKER)

**Affected truth:** #9 — *"Forking never deletes or detaches anything: the original
branch remains intact and both branches are visible in the tree afterwards
(D-14, D-15)."* The original branch IS intact and both nodes ARE listed, so the
truth's literal clause passes and its test
(`tests/cli.rs#fork_keeps_both_branches_visible`) passes. What fails is the
guarantee the sentence encodes: a second fork from the same source destroys the
first fork's image.

**Root cause — `src/core.rs:85`.**

```rust
} else if let Some(source) = request.source_image.as_deref() {
    let source_path = Path::new(source);
    output_ext = get_image_extension(source_path);
    let stem = source_path.file_stem().and_then(|s| s.to_str()).unwrap_or("image");
    let base = format!("{stem}-edit.{output_ext}");          // <-- line 85
    if request.n > 1 {
        suffix_numbered(&base, i, request.n)                // <-- line 87, only when n > 1
    } else {
        base                                                // <-- lines 88-89, always the same path
    }
}
```

Every edit and fork request the three surfaces build sets `n = 1`, so the
`else { base }` arm at `src/core.rs:88-89` is the only reachable one for them and it
is deterministic: `{stem}-edit.png`. `imagai fork` and `imagai edit` never pass
`-o`, so `output_filename` is `None` and this branch always runs.

The CR-06 fix (`src/core.rs:104-118`) closed exactly this class of bug in the
adjacent `else` branch and even states the consequence — *"the sidecar then held
two records claiming one filename (which then feeds CR-05)"*. The `source_image`
branch at line 85 still carries it, so the hazard CR-06 removed from one sibling
remains live in the other.

`suffix_numbered` cannot help: `src/core.rs:191-194` returns its input unchanged
when `total <= 1`, and this is a `n == 1` case.

**Live reproduction** (mock provider, `IMAGAI__OUTPUT_DIR=/tmp/imagai_g1`, two
`fork` commands — not two `edit` commands — against the same root node id):

```
$ imagai generate -p "a red cat on a sofa" --engine mock_gen
  root node id: a4d0c62c
$ imagai fork --from a4d0c62c -p "make it a sunset"
  → /tmp/imagai_g1/a_red_cat_on_a_sofa_20261010_074855-edit.png
$ imagai fork --from a4d0c62c -p "make it a forest"
  → /tmp/imagai_g1/a_red_cat_on_a_sofa_20261010_074855-edit.png

files on disk:
  a_red_cat_on_a_sofa_20261010_074855.png
  a_red_cat_on_a_sofa_20261010_074855-edit.png     # ONE file for two forks

sidecar records:
  a4d0c62c 'a_red_cat_on_a_sofa_20261010_074855.png'      | a red cat on a sofa
  d4c38c4d 'a_red_cat_on_a_sofa_20261010_074855-edit.png' | make it a sunset  <- overwritten
  52e536d2 'a_red_cat_on_a_sofa_20261010_074855-edit.png' | make it a forest <- the survivor

imagai history:
  🌳 Iteration history — /tmp/imagai_g1 (3 nodes)

  a4d0c62c  a red cat on a sofa               a_red_cat_on_a_sofa_20261010_074855.png  04:18
     +- d4c38c4d  make it a sunset                  a_red_cat_on_a_sofa_20261010_074855-edit.png  04:18
     `- 52e536d2  make it a forest                  a_red_cat_on_a_sofa_20261010_074855-edit.png  04:18
```

Note what the last block shows: `imagai history` reports `3 nodes`, marks
NEITHER sibling with `✗ `, and prints NO `[warning] … nodes point at files that
are no longer …` line — because a file with that name exists, so D-20's
read-time `missing` check cannot see the loss. Two nodes claim one file and the
tree looks perfectly healthy. The lineage is recorded; two of its images are
gone. That is the same failure mode CR-06 named and fixed in the adjacent
branch, still live here.

The only workaround today is an explicit `-o` (verified: `fork --from a4d0c62c
-p "make it a desert" -o desert_try.png` lands distinct files). Nothing in the
CLI help, the SPA or the TUI nudges the user toward it, and `imagai fork`'s own
`--output` help says only *"If omitted, derived from the source"* — which is
exactly the non-unique derivation.

**Test that lets it through — `tests/cli.rs:1053-1054`:**

```rust
assert!(
    stdout.contains("night") || stdout.contains("cat-edit-1.png"),
    "the fork's child is listed: {stdout}"
);
```

The `||` accepts either filename, so the test passes whether or not the fork
collided. Note the alternative `cat-edit-1.png` — a name this code path never
produces — which suggests the assertion was written for a collision-free
outcome that the implementation does not deliver.

**Severity:** high. Data loss on the primary user action, silent, and it poisons
the history store the whole phase is built on.

**Not fixed here.** Per the dispatch constraints I do not modify source. Route:
a code-review-fix dispatch scoped to `src/core.rs:85-91` (make the
`source_image` branch collision-safe on `n == 1`, mirroring CR-06), plus
tightening `tests/cli.rs#fork_keeps_both_branches_visible` to drop the `||`
escape so the regression cannot pass silently again.

### Everything else

No gaps in the other 21 truths. All other must-haves verified at source,
behaviour, test and CLI-output level; all 13 required artifacts present and
substantive; all 11 key links wired; all 4 requirement IDs satisfied; 8/8
UI-consideration categories verified; full suite 158/158; `cargo fmt --check`
clean; `cargo clippy --all-targets` clean; UAT 23/24 with the single failure
being G-1; zero anti-patterns apart from G-1.

The two 02-security-relevant audit rows the threat register named for
`/gsd-secure-phase` — T-02-01/T-02-02 (read side) and T-02-09/T-02-10 (web fork
side) — are mitigated and evidenced above; that re-audit is a separate command.

## Self-Check: PASSED

Verified at write time, not asserted:

| Claim in this report | Check | Result |
|----------------------|-------|--------|
| `02-VERIFICATION.md` and `02-UAT.md` exist at the phase dir | `[ -f … ]` on both paths | FOUND — 327 lines and 198 lines |
| No source file was modified by this verification | `git status --short` | Only the two new report files are untracked (`??`); zero `M` entries; `HEAD` still `7f0d820` |
| Test counts 76 / 39 / 8 / 35 / 158 | `cargo test \| rg 'test result:'` | matched verbatim, in that order, all `ok.` |
| `cargo fmt --check` exits 0 | run directly | exit 0, no diff |
| `cargo clippy --all-targets` clean | run directly | `Finished dev profile … in 1.45s`, zero `warning:`/`error:` lines |
| `mod tests` opens at `src/history.rs:571` and no `unwrap`/`expect` precedes it | `rg 'unwrap\(\|expect\(' src/history.rs` filtered by line < 573 | 0 hits |
| `#[cfg(test)]` opens at `src/tui.rs:2057`; the 22 Phase-2-added unwrap/expect lines are all inside it | `git diff 35cd140 HEAD -- src/tui.rs` + range check | all 22 within the test module |
| No `_ =>` tab catch-all | `rg -n '_ =>' src/tui.rs \| rg 'tab\|about'` | no output |
| CSS rule count 22 at every phase commit | `git show <c>:web_interface.html \| rg -c '^\s{8}\.[a-z-]+ \{'` at `f9ec7bd`, `35cd140`, `592c440`, `f688365`, `7f0d820`, HEAD | 22 at all six |
| G-1 root-cause lines | `sed -n '85p;87p;88,89p' src/core.rs` | line 85 `let base = format!("{stem}-edit.{output_ext}");`, line 87 `suffix_numbered(&base, i, request.n)`, lines 88-89 `} else { base }` |
| Every `file:line` citation in this report | 48 spot-checked `sed -n <line>p` reads across `src/`, `web_interface.html`, `.planning/` | 46 exact; 3 corrected during this check (core.rs 89→87, 91→88-89; history.rs 536-568→505-563); 1 corrected (`web_interface.html:1137`→1158) |
| Live evidence commands were actually run | mock provider on `127.0.0.1:8731` (request counters `{"/v1/images/generations": 14, "/v1/images/edits": 8}` at the end of the web probes), web server on `127.0.0.1:8734` / `8735` | all quoted transcript lines in this report come from those runs |
| No leftover scratch state in the repo | `git status --short`; `ls generated_images` | repo clean; the one accidental default-dir write from a mis-set env var was removed, leaving the tree exactly as found |

One reporting correction is recorded in the body: the 02-04 acceptance
criterion's "pre-existing count of 21" CSS rules is wrong — the count is and was
22. The criterion's intent (no new rule) holds.

---

_Verified: 2026-10-10T03:52:20Z_
_Verifier: gsd-executor (post-execution verification gates; HEAD 7f0d820)_
_Evidence: cargo test 158 passed; fmt/clippy clean; live CLI + web runs against a local mock provider on 127.0.0.1:8731 (web on 127.0.0.1:8734/8735)_
