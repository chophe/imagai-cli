---
phase: 02-branching-iteration-history
verified: 2026-10-10T09:46:55Z
status: passed
score: 24/24 must-haves verified
covered_files:
  - .gitignore
  - Cargo.toml
  - src/cli.rs
  - src/core.rs
  - src/history.rs
  - src/lib.rs
  - src/tui.rs
  - src/utils.rs
  - src/web.rs
  - tests/cli.rs
  - tests/core.rs
  - tests/web.rs
  - web_interface.html
covered_digest: "v3:sha256:20a084a947784f5ff7575e8b11b224e3a5b027b05394a92cb2ac3081f7ff5aa6"
behavior_unverified: 0
overrides_applied: 0
head: d92cd83c1d983a25145df7f14bb723eadc03ceba
re_verification:
  previous_status: gaps_found
  previous_score: 21/22
  gaps_closed:
    - "G-1 — A second fork from the same source overwrites the first fork's file, and the store then holds two nodes naming one file (truth #9, BRCH-02/BRCH-03)"
  gaps_remaining: []
  regressions: []
---

# Phase 02: Branching & Iteration History — Verification Report

**Phase Goal:** Users can view an image's iteration lineage and fork any past
result into a new edit without losing the original, so they can branch instead
of starting over.
**Verified:** 2026-10-10
**Status:** passed
**Re-verification:** Yes — after the G-1 gap closure (`5264a55` + `f5cf9c2`)

Config used: `workflow.verifier = true`, `workflow.human_verify_mode =
"end-of-phase"`, `workflow.nyquist_validation = false`. Verification ran against
the LIVE code on `main` at `d92cd83`, not against plan text. The previous report
(`7f0d820`, status `gaps_found`, 21/22) is superseded by this one.

## Goal Achievement

### Observable Truths

Must-haves are the merged set: 02-01..02-04 plan frontmatter (22) plus the
02-05 plan frontmatter (3, of which one restates truth #9 and is deduped into it)
= **24**.

| # | Truth (source plan) | Status | Evidence |
|---|---------------------|--------|----------|
| 1 | After any generate or edit saving into `output_dir`, a sidecar `.imagai-history.json` exists there holding a node for the saved file (02-01) | ✓ VERIFIED | `src/core.rs:186` — the single `history::record_result` call in the success arm, non-fatal by design (WR-01); `tests/core.rs#generate_writes_history_sidecar` passes |
| 2 | A plain generation becomes a session root and an edit becomes a child of the node whose filename matches its source (D-09) (02-01) | ✓ VERIFIED | `src/history.rs:383-385` `record_result` derives `parent` via `record_for_filename` (newest match, CR-05); `tests/core.rs#edit_writes_child_node_under_its_source` passes; live chain run confirmed `root → child → grandchild` |
| 3 | A history node whose file was deleted externally is kept and marked `missing`, never silently dropped (D-20) (02-01) | ✓ VERIFIED | `src/history.rs:332-335` — `missing` computed at read time from `node_path`, never persisted; `history::tests::load_view_marks_absent_files_missing_and_keeps_them` passes; live: deleted `-edit_1.png` → `✗ ee5295cb` row kept + `[warning] 1 node points at a file that is no longer in the output directory.` |
| 4 | Recorded filenames are relative to `output_dir` only, so moving the folder keeps history valid (D-23) (02-01) | ✓ VERIFIED | Records store `file_name()` only (`src/history.rs:366-368`); `record_result` guards `saved_path.parent() != Some(dir)` (`:357`) |
| 5 | Only results that land in the configured `output_dir` are recorded; an explicit out-of-dir output path stays untracked (D-22) (02-01) | ✓ VERIFIED | `src/history.rs:357` guard; `tests/core.rs#out_of_dir_result_is_not_recorded` passes |
| 6 | `imagai history` prints one indented ASCII tree per root with id8, a 32-char prompt column, filename and timestamp, roots newest-first, children oldest-first (02-02) | ✓ VERIFIED | `src/history.rs:401`/`:435`/`:447` (`prompt_field`/`timestamp_field`/`history_row_text`); `tests/cli.rs#history_tree_prints_connectors` passes; live tree above |
| 7 | `imagai history --json` emits output_dir, nodes, missing, nested roots with children (02-02) | ✓ VERIFIED | `src/cli.rs:629` serializes `history::load_view`; live: `{"output_dir":"/tmp/imagai_empty","nodes":0,"missing":0,"roots":[]}` and a populated 4-node variant |
| 8 | `imagai fork --from <image\|node-id> -p "<change>"` starts an edit whose source is the named past image, references the same file rather than copying it, saves under `<stem>-edit.<ext>` (02-02) | ✓ VERIFIED | `src/cli.rs:533` `cmd_fork` → `resolve_from` → `src/history.rs:513` `resolve_fork_source` → shared `generate_image_core` (`src/cli.rs:612`); `fork_from_node_id_records_child` and `fork_from_filename_of_adopted_file` pass; live printed `🔀 Forking from …` and saved `…-edit.png` |
| 9 | Forking never deletes or detaches anything: the original branch remains intact and both branches are visible in the tree afterwards (D-14, D-15) (02-02) — **was ✗ GAP (G-1), now closed** | ✓ VERIFIED | Root stays intact; **the sibling-collision guarantee now holds**: live `three forks from one root` → `…-edit.png`, `…-edit_1.png`, `…-edit_2.png`, three files, three distinct sidecar filenames, `(4 nodes)`, no `✗`, no warning. `tests/cli.rs#two_forks_from_one_source_keep_both_images` (the G-1 regression) and `#fork_keeps_both_branches_visible` (now asserting exactly `cat-edit_1.png`, the `\|\|` escape removed at `tests/cli.rs:1053`) both pass |
| 10 | Stepping back is choosing an ancestor node id as `--from`, which is just a fork from that node — forward-only, nothing rewound (02-02) | ✓ VERIFIED | `resolve_fork_source` step (1) accepts any id prefix; no rewind/delete path in `src/history.rs` or `src/cli.rs`; live fork from an ancestor left both generations on disk |
| 11 | Unknown `from`, a deleted recorded file and an unreadable store each print the exact Copywriting Contract sentence on stderr and exit 1 (02-02) | ✓ VERIFIED | `src/history.rs:513-594`; live run against a corrupt sidecar: `[Error] Could not read history at /tmp/…/.imagai-history.json: expected ident at line 1 column 2`, exit 1 |
| 12 | The TUI gains a History tab at TABS index 2 between Edit and Engines, listing the same nodes in the same order as the CLI tree (D-18, BRCH-04) (02-03) | ✓ VERIFIED | `src/tui.rs:43`; `tui::tests::tabs_has_history_at_index_two` passes; both renderers walk the same `history::HistoryView` |
| 13 | The tab shows a bordered tree pane and a Node detail pane in the 55/45 split the Edit tab uses, with the selected row marked `▶ ` in Cyan+BOLD and unselected rows Gray (02-03) | ✓ VERIFIED | `src/tui.rs:1126` `Layout::horizontal([Percentage(55), Percentage(45)])`; `src/tui.rs:1136-1175` |
| 14 | Enter on the History tab forks the selected node: sets `edit_source`, switches to the Edit tab, pushes the same `Source image: {name}` info log (D-14, D-18) (02-03) | ✓ VERIFIED | `src/tui.rs:1054-1088` `history_select`; `history_enter_forks_selected_node_into_edit`, `history_enter_refuses_missing_node`, `history_enter_completes_pending_pick` pass |
| 15 | The Edit tab source picker gains a first row ` Pick from history… `; selecting it moves to the History tab with a pending pick and Enter writes the chosen filename into `edit_source` (02-03) | ✓ VERIFIED | `src/tui.rs:1090-1094`; `picker_history_entry_moves_to_history_tab` passes |
| 16 | Escaping the picker or the History tab leaves `edit_source` untouched (02-03) | ✓ VERIFIED | `src/tui.rs:1096-1119`; `picker_and_history_escape_leave_source_unchanged` passes |
| 17 | Re-indexing TABS from 4 to 5 re-points every `match self.tab` arm and replaces the `_ =>` catch-all with an explicit index-4 arm (02-03) | ✓ VERIFIED | `rg -n '_ =>' src/tui.rs | rg 'tab|about'` returns nothing — dispatch exhaustive over 0..=4 |
| 18 | `GET /api/history` returns `{success, output_dir, nodes, missing, roots:[…]}` with the same nested node shape as the CLI `--json` (D-19, BRCH-04) (02-04) | ✓ VERIFIED | `src/web.rs:469` handler serializes `view.roots` directly; live probe returned `{"success":true,"nodes":3,"missing":0,...}` with children carrying `parent` == root `id` |
| 19 | `POST /api/fork` accepts `from`, `prompt`, `engine`, `output`, `refs` and its response is `result_items_json` verbatim (02-04) | ✓ VERIFIED | `src/web.rs:506` handler, `:692` `result_items_json`; live probe key set byte-identical to `/api/edit`'s |
| 20 | Every path-shaped field on `/api/fork` passes `is_plain_filename`; a separator-carrying value is refused before any read or network call (02-04) | ✓ VERIFIED | `src/web.rs:271` `is_plain_filename` on `from`, `refs`, `output` first; live: 6 traversal probes (`../../.env`, `..\..\env`, `../../escaped.png`, `../outside.png`, `/etc/passwd`, `..\..\win`) → all `{"error":"'…' is not a plain filename inside output_dir","success":false}`, **mock request counter delta = 0**, no escaped file on disk |
| 21 | The SPA renders a history view composed only from existing CSS classes, with a `Fork selected node` CTA, a `Pick a node to fork from` sentinel, and a disabled option reporting the missing count when `missing > 0` (02-04) | ✓ VERIFIED | `web_interface.html:563-600` markup — byte-identical to `7f0d820`; `loadHistory` emits the disabled `{missing} node(s) in history are missing from disk` option and `button.disabled = result.roots.length === 0` (`:1158`) |
| 22 | Forking from the browser calls `loadHistory()` on success so the new node appears without a reload (02-04) | ✓ VERIFIED | `web_interface.html:1189` `fetch('/api/fork'` → `:1237` `loadHistory()`; live probe: `/api/history` nodes 0 → 2 after two web forks |
| 23 | The first output for a source with no collision keeps the canonical `{stem}-edit.<ext>` name, so single-edit naming is unchanged (02-05) | ✓ VERIFIED | `src/core.rs:97-98` starts `candidate` at the canonical `base` (`:85`); live: fresh dir, one fork → `…-edit.png` exactly, `ls` shows only the root and `…-edit.png`. The G-1 fix is collision-only — no Phase 1 naming change when nothing claims the name |
| 24 | No two history records name the same file after successive edits/forks from one source, so `imagai history` never prints two healthy nodes over one image (02-05) | ✓ VERIFIED | Live chain: root → fork → edit-of-the-edit → 3 records, 3 distinct filenames, correct parent chain, `unique | length` = 3. Live with a deleted file kept in the store: 4 records / 4 distinct filenames, the deleted node marked `✗` and warned about, fork 3 landing on `…-edit_2.png` rather than reusing the deleted `…-edit_1.png` — the **record half** of `filename_taken`, which a disk-only probe would have missed |

**Score:** 24/24 truths verified (0 behavior-unverified).

### Requirement re-verification (G-1 route)

The 02-05 plan's `<verification>` section directs the verifier to re-run truth
#9 and the BRCH-02/BRCH-03 rows. Both hold — see rows 9, 23, 24 above and the
Requirements Coverage table below.

### Deferred Items

None. G-1 was the single open gap and it is closed on HEAD.

### Advisory (New Scope, Unevidenced)

| # | Finding | Category | Why Advisory |
|---|---------|----------|--------------|
| — | none | — | No new-scope finding from Step 7 in this re-verification round. Every anti-pattern hit below pre-dates the gap-closure round or is inside `mod tests`. |

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/history.rs` | `HISTORY_FILENAME`, `HistoryRecord`, `HistoryNode`, `HistoryView`, `history_path`, `read_records`, `append_records`, `new_node_id`, `load_view`, `record_result`, `resolve_fork_source`, `ForkSource` | ✓ VERIFIED | All present: lines 23, 47, 62, 79, 147, 160, 183, 194, 230, 355, 513, 137; **plus the new `filename_taken` at :125**. 39 `history::tests` pass |
| `src/core.rs` | Recording hook on the shared pipeline, exactly once | ✓ VERIFIED | `src/core.rs:186` — one call, inside the success arm after `saved_path` is set; non-fatal with `[warning] Lineage was not recorded for …` (WR-01) |
| `src/core.rs` | Collision-free `n == 1` arm of the `source_image` branch (02-05) | ✓ VERIFIED | `src/core.rs:97-107` — `candidate` starts at the canonical `base` (`:85`), loops while `crate::history::filename_taken(&settings.output_dir, &candidate)`, bumps through `suffix_numbered(&base, bump, 2)`; bounded by `MAX_NAME_BUMPS` = 1000 (`:211`) |
| `src/history.rs` | The "is this filename already claimed" probe (02-05) | ✓ VERIFIED | `src/history.rs:125` `pub fn filename_taken` — `dir.join(filename).is_file()` OR a record naming it OR (on an unreadable sidecar) `true`. Four unit tests: `filename_taken_is_false_for_a_name_nothing_claims`, `seed_a_file_the_store_does_not_know`, `honours_a_record_whose_file_is_gone`, `treats_an_unreadable_sidecar_as_taken` — all pass |
| `src/utils.rs` | `pub fn datetime_iso` + `civil_from_days` relocated from `web.rs` | ✓ VERIFIED | `src/utils.rs:345`, `:367`; `rg 'fn datetime_iso\|fn civil_from_days' src/web.rs` returns nothing; `src/history.rs` uses it at 386 and 488 |
| `src/lib.rs` | `pub mod history;` in flat alphabetical position | ✓ VERIFIED | `src/lib.rs:6`, between `core` and `models` |
| `tests/core.rs` | `generate_writes_history_sidecar`, `edit_writes_child_node_under_its_source`, `out_of_dir_result_is_not_recorded` | ✓ VERIFIED | Lines 168, 231, 294 — all pass |
| `src/cli.rs` | `HistoryArgs`, `ForkArgs`, `cmd_history`, `cmd_fork` | ✓ VERIFIED | `src/cli.rs:196`, `:166`, `:629`, `:533`; `history --json` and `fork --from` flags present in `--help` |
| `tests/cli.rs` | Tree-grammar + fork e2e coverage, and the G-1 deterministic regression | ✓ VERIFIED | `#history_tree_prints_connectors` + `#two_forks_from_one_source_keep_both_images` (`:1067`) + `#fork_keeps_both_branches_visible` (`:1053`, the `||` escape removed). 12 history/fork tests at `tests/cli.rs:690..1142`, all pass |
| `src/tui.rs` | `TABS[5]`, `HistoryState`, `HistoryRow`, `history_rows`, `flatten_nodes`, `load_history`, `draw_history_tab`, `handle_history_key`, `history_move`, `history_select`, `open_history_pick`, `scroll_selection`, `PickerOrigin` | ✓ VERIFIED | `src/tui.rs:43`, 1004, 1032, 1054, 1090, 1096, 1120, 1132, 1190, 1265, 925; 9 `tui::tests` history tests pass |
| `src/web.rs` | `/api/history` route + handler, `/api/fork` route + handler, `ForkPayload` | ✓ VERIFIED | `src/web.rs:141-142` routes, `:469` handler, `:506` handler, `struct ForkPayload` beside `EditPayload` |
| `web_interface.html` | `historyForm`, `history_node`, `historyBtn`, `history_prompt`, `history_engine`, `history_refs`, `history_output`, `loadHistory`, `executeFork` | ✓ VERIFIED | Lines 563, 568, 598, 574, 580, 588, 594, 1122, 1166; byte-identical to `7f0d820` (git diff over the range is empty for this file) |
| `tests/web.rs` | 10 history/fork endpoint + form tests | ✓ VERIFIED | `tests/web.rs:483..851`, all pass; suite reports 35 passed / 0 failed |
| `.gitignore` | `.imagai-history.json` entry, pre-existing entries unchanged | ✓ VERIFIED | `.gitignore:8`; `/target`, `/generated_images`, `.env`, `*.env`, `.DS_Store` intact. Not touched by the G-1 commits |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `src/core.rs` | `src/history.rs` | every successful save into `output_dir` calls `history::record_result` | ✓ WIRED | `src/core.rs:186` — the single recording point, so CLI/TUI/web cannot drift (BRCH-04) |
| `src/core.rs` | `src/history.rs` | **the `n == 1` `source_image` arm calls `crate::history::filename_taken` before committing to a name (02-05)** | ✓ WIRED | `src/core.rs:99` — inside the `for bump in 0..MAX_NAME_BUMPS` loop at `:98`; fully qualified, matching the `record_result` style at `:186` |
| `src/history.rs` | `src/utils.rs` | node `created` from the one shared timestamp helper | ✓ WIRED | `src/history.rs:386`, `:488` call `crate::utils::datetime_iso`; `web.rs` defines no second source |
| `src/history.rs` | `.imagai-history.json` | read-modify-write of the whole sidecar per save (D-11) | ✓ WIRED | `src/history.rs:183-189` `append_records` — read, extend, pretty-print, temp-file + `rename` (WR-01) |
| `src/cli.rs` | `src/history.rs` | `cmd_history` → `load_view`, serializing the same `HistoryView` | ✓ WIRED | `src/cli.rs:629`; `HistoryView` derives `Serialize` (`src/history.rs:79`) |
| `src/cli.rs` | `src/history.rs` | `cmd_fork` → `resolve_fork_source`, the single resolver | ✓ WIRED | `src/cli.rs:505`; TUI and web route through the same function |
| `src/cli.rs` | `src/core.rs` | fork builds the same request and calls `generate_image_core` | ✓ WIRED | `src/cli.rs:612`; no parallel pipeline (D-13) |
| `src/tui.rs` | `src/history.rs` | the History tab renders a `HistoryView` from `load_view` | ✓ WIRED | `src/tui.rs:1004`; `load_view_treats_unknown_parent_as_a_root_and_survives_a_cycle` protects the walker |
| `src/tui.rs` | `src/tui.rs` | fork reuses Edit-tab state rather than a second request path | ✓ WIRED | `history_select` assigns `self.edit_source` and `self.tab = 1` (`src/tui.rs:1054-1088`) |
| `src/web.rs` | `src/history.rs` | history and fork handlers call `load_view` / `resolve_fork_source` | ✓ WIRED | `src/web.rs:469`, `:506+` — same store the CLI reads |
| `src/web.rs` | `src/web.rs` | fork response is `result_items_json` | ✓ WIRED | `src/web.rs:281` definition, `:460` (edit) and `:692` (fork) callers; live probe confirmed identical key sets |
| `web_interface.html` | `src/web.rs` | `loadHistory` fetches `/api/history`, `executeFork` posts `/api/fork`, then `loadHistory()` again | ✓ WIRED | `web_interface.html:1126`, `:1189`, `:1237` |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `src/core.rs` save arm | `record_result` args | real `saved_path` + real prompt + real model name | ✓ Yes — live run wrote `.imagai-history.json` with a real record per saved image | ✓ FLOWING |
| `src/core.rs` `n==1` name arm (new) | `candidate` | `crate::history::filename_taken` over `settings.output_dir` | ✓ Yes — live: canonical when free, `_1`/`_2` when claimed by a file or a record | ✓ FLOWING |
| `src/history.rs` store | `records` | `fs::read_to_string` of the sidecar | ✓ Yes — live `history --json` printed the persisted envelope | ✓ FLOWING |
| `src/history.rs` view | `roots` | the sidecar, reduced + ordered | ✓ Yes — live tree rendered 4 nodes with correct parent links | ✓ FLOWING |
| `src/web.rs` `/api/history` | `view.roots` | `load_view` | ✓ Yes — live HTTP 200 with nested children and `parent` == root `id` | ✓ FLOWING |
| `src/web.rs` `/api/fork` | `source_image` | `resolve_fork_source` → `is_plain_filename`-guarded | ✓ Yes — live POST saved `…-edit.png` and returned `image_data` | ✓ FLOWING |
| `web_interface.html` picker | `#history_node` options | `GET /api/history` roots, no client re-sort | ✓ Yes — live GET populated the select; `sort` appears only in comments saying not to | ✓ FLOWING |
| `src/tui.rs` History rows | `state.rows` | `history_rows(&view)` | ✓ Yes — unit-asserted connector/`✗`/`… n more` rendering | ✓ FLOWING |

### Requirements Coverage

| Requirement | Source Plans | Description | Status | Evidence |
|-------------|--------------|-------------|--------|----------|
| BRCH-01 | 02-02, 02-03, 02-04 | View iteration history | ✓ SATISFIED | CLI tree + `--json`, TUI History tab, `GET /api/history`; 3 surfaces all consume the same `HistoryView` |
| BRCH-02 | 02-02, 02-03, 02-04, **02-05** | Fork a past image into a new edit | ✓ SATISFIED | `fork --from`, TUI Enter, `POST /api/fork` — all through `resolve_fork_source` + `generate_image_core`. **Re-verified after G-1**: live three forks from one root produced three distinct files and three distinct records |
| BRCH-03 | 02-02, 02-03, 02-04, **02-05** | Keep the original when forking | ✓ SATISFIED (**G-1 closed**) | The named original is never modified, and a *sibling* fork no longer destroys a prior fork: live `fork_keeps_both_branches_visible` and `two_forks_from_one_source_keep_both_images` both pass, and the live three-fork run left all three images on disk |
| BRCH-04 | 02-01, 02-03, 02-04 | Same history across all surfaces | ✓ SATISFIED | One recorder (`src/core.rs:186`), one store, one serializer — CLI/web key sets confirmed identical live |

No orphaned requirement IDs. None of the four is claimed by only one plan;
BRCH-02/BRCH-03 are additionally claimed by the 02-05 gap-closure plan.

### UI Considerations (from 02-ui-spec.md `## UI Considerations`)

`web_interface.html` is byte-identical to `7f0d820` across the gap-closure
range, and `src/tui.rs` / `src/web.rs` are unchanged, so the previous round's
UI-Considerations audit carries forward. Every category was re-asserted as a
live or test-backed assertion rather than taken on the SPEC's word.

| Category | Element(s) | Status | Evidence |
|----------|------------|--------|----------|
| empty | E1, E2, E3, E4, E5 | ✓ VERIFIED | `IMAGAI__OUTPUT_DIR=/tmp/imagai_empty imagai history` → `🌳 No history yet — generate or edit an image first.`; `history --json` → `nodes: 0, roots: []`; SPA sentinel `Pick a node to fork from` (`web_interface.html:1143`) + `button.disabled` (`:1158`); `history_empty_state` passes |
| populated | E1–E5 | ✓ VERIFIED | Live 4-node tree: id8, 32-char prompt field, filename, `YYYY-MM-DD HH:MM`, `+- `/`` `- `` connectors, three-space depth-1 indent |
| zero-one-many | E1–E5 | ✓ VERIFIED | Live `(1 node)` / `(3 nodes)` / `(4 nodes)`; singular/plural driven by one branch each (`src/cli.rs` IN-05 fix) |
| partial | E1–E5 | ✓ VERIFIED | Live `17183ed2  (unknown prompt)  untracked.png  2026-10-10 09:43` for a lazily adopted file; live `✗ ee5295cb` + `[warning] 1 node points at a file that is no longer in the output directory.` for a deleted one |
| overflow | E1–E5 | ✓ VERIFIED | `prompt_field` clips at 32 chars + `…` (`src/history.rs:404-406`); filename and id8 never clipped; TUI `… {n} more` in `Color::DarkGray` (`src/tui.rs:1607`); `history_scroll_marks_remaining_rows` passes |
| long-text | E2, E3, E5 | ✓ VERIFIED | `Paragraph::wrap(Wrap { trim: true })` on the detail pane prompt row (`src/tui.rs:1261`); web option labels and error sentences are unbounded single `<option>`/`<div>` |
| error | E1–E5 | ✓ VERIFIED | Verbatim Copywriting Contract sentences from `src/history.rs:513-594`; live corrupt-sidecar run exited 1 with `Could not read history at …`; TUI routes through `push_log` (`history_enter_refuses_missing_node` asserts `no longer in`); web returns `{"error":"'../../.env' is not a plain filename inside output_dir","success":false}` |
| loading | E1–E5 | ✓ VERIFIED (explicit negative) | No new affordance: `loadHistory` populates after the awaited fetch following the `loadOutputImages()` pattern (`web_interface.html:1122-1137`); zero new CSS rules and zero new DOM for a spinner |

Dismissed considerations: none — as the SPEC records.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| — | — | `rg 'TODO\|FIXME\|XXX\|unimplemented!\|todo!'` over `src/`, `tests/`, `web_interface.html` | — | Zero hits |
| — | — | `unwrap()`/`expect()` in `src/history.rs` outside `mod tests` | — | Zero hits — `mod tests` opens at `src/history.rs:597` |
| — | — | `_ =>` catch-all in the tab dispatch | — | Absent — `rg -n '_ =>' src/tui.rs` filtered on tab/about returns nothing |
| — | — | New CSS rule in the SPA | — | None. Rule count 22 at `7f0d820` and at HEAD — unchanged across the G-1 commits |
| — | — | New dependency | — | None. `git diff --name-only 7f0d820..HEAD -- Cargo.toml Cargo.lock .gitignore` is empty |

**What the G-1 commits changed** (`git diff --stat 7f0d820..HEAD`):
`src/core.rs` +25/-2, `src/history.rs` +75, `tests/cli.rs` +80/-1. Three
files, additive. No Phase 1 naming behaviour changed when there is no
collision (truth #23).

The one plan-text inaccuracy recorded in the previous round still stands and is
still only a reporting note, not code debt: the 02-04 acceptance criterion
expects "21" pre-existing CSS rules; the real count is and was 22. The
criterion's intent — no new rule — holds.

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Full test suite | `cargo test` | `test result: ok. 80 passed` (lib) / `0 passed` (main) / `40 passed` (tests/cli.rs) / `8 passed` (tests/core.rs) / `35 passed` (tests/web.rs) / `0 doctests` — **163 passed, 0 failed** | ✓ PASS |
| G-1 regression | `cargo test --test cli two_forks_from_one_source_keep_both_images` | `test result: ok. 1 passed` | ✓ PASS |
| G-1 pin | `cargo test --test cli fork_keeps_both_branches_visible` | `test result: ok. 1 passed` (asserts exactly `cat-edit_1.png`, no escape) | ✓ PASS |
| G-1 probe units | `cargo test --lib filename_taken` | `test result: ok. 4 passed` (free / on-disk / record-claim / unreadable) | ✓ PASS |
| CR-01..CR-06 regression set | `cargo test --test cli history`, `--lib history`, `--test core`, `--test web` | 4 + 39 + 8 + 35 passed, 0 failed — including `history_reports_a_non_utf8_sidecar_instead_of_an_empty_tree`, `fork_from_a_file_that_lives_only_in_the_cwd_is_refused`, `fork_from_a_bare_file_when_cwd_is_output_dir_adopts_it`, `out_of_dir_result_is_not_recorded`, `default_filename_with_multiple_images_does_not_collide`, `serve_image_blocks_traversal`, `fork_endpoint_reduces_a_traversal_shaped_record_filename`, `fork_endpoint_refuses_a_file_that_lives_only_in_the_cwd`, `history_endpoint_reports_unreadable_store`, `load_view_survives_a_deep_chain_on_a_small_stack`, `load_view_treats_unknown_parent_as_a_root_and_survives_a_cycle`, `a_duplicate_filename_resolves_to_the_newest_record` | ✓ PASS |
| Format | `cargo fmt --check` | exit 0, no diff | ✓ PASS |
| Lints | `cargo clippy --all-targets` | `Finished dev profile … in 1.06s`, zero `warning:`/`error:` lines (grep count 0) | ✓ PASS |
| Live G-1 inversion (CLI) | generate a root, then 3 x `imagai fork --from <root id>` against a mock provider on 127.0.0.1:8731 | 4 files on disk (root, `…-edit.png`, `…-edit_1.png`, `…-edit_2.png`); sidecar 4 records / 4 distinct filenames; `history` → `(4 nodes)`, no `✗`, no warning, exit 0 | ✓ PASS |
| Live record-half probe (CLI) | delete `…-edit_1.png` (record kept per D-20), fork again | fork 3 saved `…-edit_2.png`, **not** the deleted `…-edit_1.png`; `history` → `✗ ee5295cb` + `[warning] 1 node points at a file that is no longer in the output directory.`; 4 records / 4 distinct filenames | ✓ PASS |
| Live canonical name (CLI) | fresh dir, one fork | saved `…-edit.png` exactly — unchanged single-edit naming | ✓ PASS |
| Live edit chain (CLI) | root → fork → `edit --from …-edit.png` | `…-edit-edit.png`, 3 records / 3 distinct filenames, root → child → grandchild | ✓ PASS |
| Live web fork × 2 | `POST /api/fork` twice from one root node | `…-edit.png` and `…-edit_1.png`; `/api/history` → `nodes: 3, missing: 0`, both children `parent` == root id | ✓ PASS |
| Live web traversal | 6 separator-carrying `from` values | all `success:false` with `is not a plain filename inside output_dir`; **mock request counter delta = 0**; no escaped file on disk | ✓ PASS |
| Live sidecar not served | `GET /api/images/.imagai-history.json` and `.tmp` | both 404 | ✓ PASS |
| Live corrupt store | fork against a sidecar containing `not json` | `[Error] Could not read history at …: expected ident at line 1 column 2`, exit 1 — not a silent empty tree | ✓ PASS |
| Live partial state | fork from an untracked file | adopted root rendered `(unknown prompt)` | ✓ PASS |

### Prohibitions Review

| Prohibition | Status | Evidence |
|-------------|--------|----------|
| No second recorder per frontend | ✓ HELD | `rg -c 'record_result' src/cli.rs src/tui.rs src/web.rs src/core.rs` → 0, 0, 0, 1 |
| No second result serializer | ✓ HELD | `rg -c 'fn result_items_json' src/web.rs` = 1, called by both `edit` and `fork` |
| No client-side re-sort of history | ✓ HELD | `sort` appears in `web_interface.html` only inside two "do not re-sort" comments (`:917`, `:1132`) |
| No new CSS rule / JS toolchain / dependency | ✓ HELD | CSS rule count 22 at `7f0d820` and HEAD; no `package.json`; `Cargo.toml` untouched |
| No `unwrap`/`expect` in production `src/history.rs` or new `src/tui.rs` code | ✓ HELD | measured zero in `src/history.rs` before line 597 |
| No silent empty tree on an unreadable store | ✓ HELD | `history_reports_a_non_utf8_sidecar_instead_of_an_empty_tree`, `history_endpoint_reports_unreadable_store`, live corrupt-store run (exit 1, named path) |
| No CWD-relative fork resolution (CR-01/CR-03) | ✓ HELD | `fork_from_a_file_that_lives_only_in_the_cwd_is_refused` and `fork_from_a_bare_file_when_cwd_is_output_dir_adopts_it` both pass |
| No path traversal from `filename` (T-02-01/T-02-09/T-02-10) | ✓ HELD | every consumer reduces through `history::node_path` (`src/web.rs:264`, `src/history.rs:210`); `load_view_reduces_a_filename_that_carries_a_parent_component` and `fork_endpoint_reduces_a_traversal_shaped_record_filename` pass |
| **No two records naming one file (T-02-13, new in 02-05)** | ✓ HELD | `filename_taken` covers both halves of the claim; live record-half probe forced the bump past a name whose file was deleted; `two_forks_from_one_source_keep_both_images` asserts 3 records / 3 distinct filenames / both children parented to the root / no missing marker |

### Human Verification Required

None. Every must-have is asserted by a passing test or a live command run in
this verification round. The `workflow.human_verify_mode = "end-of-phase"`
harvest found no `<verify><human-check>` blocks on `auto` tasks in any of the
five plans, and no TUI-rendering or visual-quality item remains unverified by a
test.

## Gaps Summary

**No gaps.** The single blocker from the previous round is closed.

### G-1 — closed

The previous report's blocker was: a second fork from the same source derived
the first fork's `{stem}-edit.<ext>` name and overwrote its image, leaving two
records naming one file while `imagai history` reported `(3 nodes)` with no
`✗` marker and no warning — the same failure CR-06 removed from the adjacent
default-filename branch.

Closure at HEAD (`src/core.rs:85-108`): the `else { base }` arm is now a probe
loop. `candidate` starts at the canonical `base` and, while
`crate::history::filename_taken(&settings.output_dir, &candidate)` holds, bumps
through the shared `suffix_numbered(&base, bump, 2)` scheme — so the bumped
names are exactly the `_N` names `n > 1` already produces. The loop is bounded
by `MAX_NAME_BUMPS = 1000` because an unreadable sidecar reads as taken. The
canonical name is kept when nothing claims it, so Phase 1's single-edit naming
is unchanged.

`filename_taken` (`src/history.rs:125`) covers both halves of the claim: a file
on disk (`dir.join(filename).is_file()`), and a sidecar record naming it — which
matters because D-20 keeps a node whose file was deleted externally, so that
name is not free again. An unreadable sidecar reads as taken: the safe
direction is to disambiguate, never to overwrite.

Verified end to end, not taken on the SUMMARY's word:

- **Live** — three forks from one root produced four files and four distinct
  filenames; `imagai history` printed `(4 nodes)` with no `✗` and no warning.
- **Live, the record half specifically** — after deleting `…-edit_1.png` while
  its record stayed in the store, the next fork saved `…-edit_2.png` rather
  than reusing the deleted name; the deleted node kept its `✗` marker and the
  warning line.
- **Live, canonical name preserved** — a fresh directory with a single fork
  saved `…-edit.png`, unchanged.
- **Test** — `two_forks_from_one_source_keep_both_images` asserts both
  `cat-edit.png` and `cat-edit_1.png` exist as files, three records with three
  distinct filenames, both children parented to the root, the root listed
  exactly once, and no missing-file marker.
- **Test** — `fork_keeps_both_branches_visible` now asserts exactly
  `cat-edit_1.png`; the permissive `|| cat-edit-1.png` escape is gone, so the
  collision cannot pass silently again.
- **Suite** — 163 passed / 0 failed; `cargo fmt --check` exit 0;
  `cargo clippy --all-targets` clean.

### WR-05 — accepted residual, unchanged and correctly recorded

WR-05 (a fork from an explicitly out-of-dir file records its child with
`parent: None`) is recorded in `02-REVIEW-FIX.md:177-197` as a partial
resolution with a locked-decision conflict: D-22 mandates an out-of-dir file
stays untracked, so there is genuinely no parent to record and `parent: None`
is the correct documented outcome. The behaviour on HEAD still matches the
locked decision — an out-of-dir `--from` returns `ForkSource::File { recorded:
false }` (`src/history.rs:550-554`), so the child's `record_for_filename`
lookup finds no record and `parent` is `None`. This does **not** contradict
D-22, so it is not scored as an open blocker.

---

_Verified: 2026-10-10T09:46:55Z_
_Verifier: the agent (gsd-verifier)_
_Evidence: cargo test 163 passed / 0 failed; cargo fmt --check exit 0; cargo clippy --all-targets clean; live CLI + web runs against a local mock OpenAI-compatible provider on 127.0.0.1:8731 (web on 127.0.0.1:8735); HEAD d92cd83_
