---
phase: 02-branching-iteration-history
plan: 03
type: execute
wave: 2
subsystem: tui-surface
tags: [tui, ratatui, history-tab, picker]
requires: [02-01]
provides:
  - TUI History tab (tree pane + Node detail pane) at TABS index 2
  - fork-from-selected-node and the pick-from-history round trip
affects: [src/tui.rs, src/history.rs, src/cli.rs]
tech-stack:
  added: []
  patterns: [pure dispatchers for exhaustive tab matching, one scroller for two lists, PickerOrigin discriminator on one picker type]
key-files:
  created: []
  modified: [src/tui.rs, src/history.rs, src/cli.rs]
decisions:
  - "draw/draw_footer dispatch through the extracted pure dispatchers tab_surface/tab_footer_hints (match on usize with 0..=4 cannot be exhaustive), so both are unit-testable and an out-of-range index renders nothing rather than silently landing on About (T-02-08)"
  - "The tree row grammar moved to history::history_row_text so the CLI, TUI and (next) SPA render one implementation — BRCH-04 by construction, not by matching screenshots"
  - "'p' on the History tab opens the Edit-tab picker with the pick pending, matching the UI-SPEC key table ('p forks via the Edit-tab picker path'); this is also what makes PickerOrigin::History a live variant instead of dead code"
metrics:
  duration: "~35m"
  completed: 2026-10-10T09:20:00Z
  tasks: 2
  commits: 1
  files: 3
plan_head_before: 592c440
plan_head_after: 3569561
status: complete
actuals:
  tokens: 31000
  tasks: 2
  commits: 1
estimate:
  tokens: 34000
  tasks: 3
  confidence: low
---

# Phase 2 Plan 03: TUI History tab + fork Summary

**One-liner:** A History tab at index 2 lists the same nodes in the same order as `imagai history`, forks the selected node into the Edit tab, and the Edit tab's source picker gains a ` Pick from history… ` hand-off — one scroller, one picker type, no `unwrap` outside tests.

## What Was Built

**`src/history.rs`** — the row grammar moved out of `src/cli.rs` into shared
primitives so the surfaces cannot drift:

- `prompt_field` (fixed 32-character column, `...` clip at 29, `(unknown prompt)` for a lazily adopted file)
- `timestamp_field` (`2026-10-09T11:40:00` → `2026-10-09 11:40`)
- `history_row_text(node, depth, is_last)` — indent, connector, `✗ ` marker, id8, prompt, filename, timestamp

`src/cli.rs::render_history_text` now calls `history::history_row_text` (its local
copies of the two column helpers and the row body are gone).

**`src/tui.rs`**

- `TABS: ["Generate", "Edit", "History", "Engines", "About"]` with the hazard comment.
- `enum TabSurface` + `fn tab_surface(usize) -> Option<TabSurface>` and `fn tab_footer_hints(...)` — the two pure dispatchers extracted from `draw` and `draw_footer`; both match exhaustively with no `_ =>` arm.
- `HISTORY_FOOTER_HINT` = `q Quit | Tab switch tab | ↑/↓ navigate | Enter fork from node | p pick from history`; `PICK_FROM_HISTORY` = ` Pick from history… `; `MAX_HISTORY_DEPTH = 64`.
- `HistoryState { rows, selected, scroll, height, loaded }` and `HistoryRow { line, node }`; `history_rows` / `flatten_nodes` (depth-guarded) build the rows with the shared grammar, and `remaining_line` produces the trailing `… {n} more` line.
- `App` gains `history: Option<HistoryState>` and `history_pick_for_edit: bool`.
- `load_history` (logs the store error, never unwraps, never a silent empty tree; also logs the `{N} history node(s) point at files no longer in the output directory` warning), `draw_history_tab` (55/45 split, ` History ` tree pane with `▶ ` selection in Cyan+BOLD and Gray unselected rows, `… {n} more` in DarkGray, the UI-SPEC empty-state line), `draw_history_detail` (` Node ` pane, `label {:<10}` rows, the wrapped prompt row, `file ✗ … (missing from disk)` in Yellow, and the ` [ Fork edit ] ` CTA in Green).
- `handle_history_key` (↑/↓/j/k move, Enter forks, `p` picks via the Edit-tab picker, Esc abandons a pending pick leaving `edit_source` untouched, `q` quits), wired into `handle_key` after the picker interception and before the Edit branch.
- `history_select` (fork action: writes the reduced filename into `edit_source`, switches to the Edit tab, pushes `Source image: {name}` byte-identical to `picker_select`; refuses a missing node with the contract sentence), `history_move` and `picker_move` sharing the extracted `scroll_selection`, `open_history_pick`.
- `PickerOrigin { Source, History }` on `PickerState`; `open_source_picker` prepends the history row when no pick is pending and omits it when one is, `picker_select` hands off to `open_history_pick` on that row and clears the pending flag on a file choice.
- `drain_messages` reloads the view after a saved result; the run loop reloads it when Tab enters the tab, and the `f` engines interception moved from index 2 to 3.

## Deviations from Plan

### Structural adjustments (required by the clippy-clean baseline)

**1. `HistoryState` has no `view` field** — the plan's struct listed `view: history::HistoryView`, but nothing reads it once the rows are flattened (the tree reads `rows`, the detail pane reads the row's node, the missing count is logged at load time). `dead_code` is a warning this project forbids in `src/`, so the field is dropped rather than left unread. `HistoryRow` likewise drops its redundant `filename` field (`node` carries it).

**2. `p` on the History tab opens the Edit-tab picker instead of calling `open_history_pick()`** — the plan's item 5 wired `p` to `open_history_pick`, which from the History tab would only re-set the flag you are already on. The UI-SPEC key table defines `p` as "forks via the Edit-tab picker path", so `p` switches to the Edit tab and opens the picker with the pick pending. This is also the only reachable constructor of `PickerOrigin::History`, which keeps that variant from being dead code without an `#[allow]` in `src/`.

**3. `cargo test --lib <name> -- --exact` needs the `tui::tests::` prefix** — the plan's verify commands use bare names, which match zero lib tests ("0 passed"). All ten were re-run as `tui::tests::<name>`; each reports `ok. 1 passed`.

### Auto-fixed Issues

**4. [Rule 1 - Bug] Two new tests asserted the truth of a `false` return**
- **Found during:** the first RED→GREEN run of the new key tests
- **Issue:** `assert!(app.handle_history_key(KeyCode::Enter))` — only `Char('q')` returns `true`; Enter/Esc return `false`, so the assertions inverted.
- **Fix:** `assert!(!...)` with a `"… does not quit"` message.
- **Files modified:** src/tui.rs

**5. [Rule 1 - Bug] Picker-row index assumptions shifted by the prepended entry**
- **Found during:** `picker_lists_newest_first` / `picker_selection_lands_in_source_field` after the history row became entry 0
- **Issue:** the existing tests indexed the entry list as if it held only filenames, so "newest first" compared the wrong rows and the clamp index moved from 1 to 2.
- **Fix:** both tests now assert against the three-entry list (` Pick from history… ` first, then the images newest-first) and the clamp at index 2.
- **Files modified:** src/tui.rs

## Verification Evidence

```
cargo fmt --check                   -> exit 0
cargo clippy --all-targets          -> 0 warnings, 0 errors
cargo test --lib tui::tests::tabs_has_history_at_index_two -- --exact              -> ok. 1 passed
cargo test --lib tui::tests::history_tree_lines_use_the_contract_connectors --exact -> ok. 1 passed
cargo test --lib tui::tests::history_tree_marks_missing_nodes -- --exact           -> ok. 1 passed
cargo test --lib tui::tests::history_scroll_marks_remaining_rows -- --exact        -> ok. 1 passed
cargo test --lib tui::tests::history_enter_forks_selected_node_into_edit -- --exact -> ok. 1 passed
cargo test --lib tui::tests::history_enter_refuses_missing_node -- --exact         -> ok. 1 passed
cargo test --lib tui::tests::picker_history_entry_moves_to_history_tab -- --exact  -> ok. 1 passed
cargo test --lib tui::tests::history_enter_completes_pending_pick -- --exact       -> ok. 1 passed
cargo test --lib tui::tests::picker_and_history_escape_leave_source_unchanged -- --exact -> ok. 1 passed
cargo test --lib tui::tests::picker_lists_newest_first -- --exact                  -> ok. 1 passed
cargo test                          -> lib 67 ok, cli 36 ok, core 7 ok, web 19 ok
rg -n '_ =>' src/tui.rs | rg 'tab|about'  -> (no output)
rg -c 'scroll_selection\(' src/tui.rs     -> 3 (definition plus two call sites)
rg -n 'unwrap\(|expect\(' src/tui.rs | rg -v 'mod tests' -> only the pre-existing
    draw_source_picker hit at src/tui.rs:1561
```

Manual confirmation with a real terminal was not possible in this environment; the unit suite exercises the tab table, the row grammar, the fork action, the missing-node refusal and both escape guarantees directly against `App`.

## Known Stubs

None.

## Threat Flags

None — T-02-07 is mitigated by `history_select`'s `file_name()` reduction (asserted by `history_enter_forks_selected_node_into_edit`, whose `edit_source` is the bare filename) and T-02-08 by the exhaustive dispatchers asserted in `tabs_has_history_at_index_two`.

## Self-Check: PASSED

- `.planning/phases/02-branching-iteration-history/02-03-SUMMARY.md` — written
- `src/tui.rs` contains `const TABS: [&str; 5]`, `MAX_HISTORY_DEPTH`, `HistoryState`, `HistoryRow`, `PickerOrigin`, `history_rows`, `flatten_nodes`, `load_history`, `draw_history_tab`, `handle_history_key`, `history_move`, `history_select`, `open_history_pick`, `scroll_selection`
- Commit `3569561` is an ancestor of HEAD; no tracked deletions in the commit
