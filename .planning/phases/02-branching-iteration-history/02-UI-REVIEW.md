# Phase 2 — UI Review

**Audited:** 2026-10-10
**Baseline:** `02-UI-SPEC.md` (approved 2026-10-09, gsd-ui-checker 5 PASS / 2 FLAG) + `02-CONTEXT.md` D-08..D-23
**Screenshots:** not captured — no web dev server (ports 3000/5173/8080 returned `000`). Live evidence instead: the real `target/debug/imagai` binary was run against a seeded `.imagai-history.json` and a local mock provider, plus the real TUI driven through a pty.
**Interaction captures:** off (`workflow.ui_interaction_capture` absent from `<config>`; no interaction key found in `.planning/config*`)
**Live evidence:** CLI `history` / `history --json` / `fork`; `GET /api/history`, `POST /api/fork` (8 error/success cases); TUI History tab rendered at 20×120 and 40×120 with 30-node, 2-node, and missing-node sidecars.

---

## Contract Compliance Verdict

**PARTIAL — advisory, not blocking.**

Three explicit, checkable criteria in the contract are unmet, plus one BRCH-04 parity break.

| # | Criterion | Verdict | Evidence |
|---|-----------|---------|----------|
| 1 | Copywriting Contract, `Empty state body` — web shows it "as the single disabled option" | **FAIL** | `No history yet — generate or edit an image first.` appears nowhere in `web_interface.html` (`rg` = 0 hits) |
| 2 | UI Considerations, `overflow` — "TUI pane clips at pane width with a `… {n} more` line in `Color::DarkGray`" | **FAIL** | Live 20-row render of 30 nodes: 14 rows, **zero** `… more` lines |
| 3 | UI Considerations, `long-text` — "The prompt row is the only wrapped line … every other detail row is one clipped line" | **FAIL** | `tui.rs:1261` wraps the whole paragraph; ` [ Fork edit ] ` can be pushed off-pane |
| 4 | BRCH-04 — "same nodes, same order, same operations" / same row grammar on all surfaces | **FAIL** | SPA `historyNodeLabel` slices by UTF-16 units; 20×🎉 prompt emits a lone surrogate where the CLI emits a clean 32-char field |

Everything else in the contract is met exactly: **no new CSS rule** (`<style>` block byte-identical to the Phase-1 commit — verified by diff), **no new color**, the verbatim copy table across CLI/TUI/web, the three error sentences byte-identical between CLI and API, newest-first/oldest-first ordering, the tab re-indexing, the trust boundary, and `is_plain_filename`.

**Registry Safety:** skipped — no `components.json`, `shadcn_initialized: false`, and the contract declares zero registries and zero third-party blocks. The SPA has no imports at all.

---

## Pillar Scores

| Pillar | Score | Key Finding |
|--------|-------|-------------|
| 1. Copywriting | **2/4** | 13 strings verbatim across 3 surfaces; the web empty-state sentence is absent entirely |
| 2. Visuals | **2/4** | 55/45 split, borders, row styling all correct; overflow indicator is invisible and the detail pane over-wraps |
| 3. Color | **3/4** | `<style>` byte-identical, zero new colors; `Color::Yellow` misused for a record state, not a warning |
| 4. Typography | **2/4** | Zero new rules; SPA column arithmetic counts UTF-16 units and splits surrogate pairs |
| 5. Spacing | **3/4** | No new CSS rule, reuses the inventoried classes and mirrors `#editForm`; one bespoke `size="8"` listbox |
| 6. Experience Design | **2/4** | Strong error/state discipline; invisible overflow cue, missing web empty state, unscrollable detail pane |

**Overall: 14/24**

---

## Top 3 Priority Fixes

### 1. The `… {n} more` overflow indicator is never rendered (BLOCKER)

**Impact:** A user with 20 images in `output_dir` sees a full-looking History pane and no signal that more nodes exist below. They must blind-scroll to discover them. The contract's overflow criterion is unmet.

**Cause — the row was not reserved.** `draw_history_tree` computes `visible = area.height - 2`, then:

```rust
// src/tui.rs:1144-1145
let start = scroll.min(len.saturating_sub(1));
let end = (start + visible).min(len);
```

then pushes one line per visible row **plus** the `… {n} more` line (`tui.rs:1181-1186`) — `visible + 1` lines into a `visible`-row inner area. ratatui's `render_lines` breaks at `y >= area.height` (`ratatui-widgets-0.3.2/src/paragraph.rs:450-459`), so the last line is dropped.

**The sibling that does it right:** `draw_source_picker` — the exact idiom the History pane claims to mirror — reserves the row:

```rust
// src/tui.rs:1573-1577
let start = scroll.min(len.saturating_sub(1));
let mut end = (start + visible).min(len);
if end < len {
    end = end.saturating_sub(1).max(start);
}
```

**Fix:** copy that three-line reservation into `draw_history_tree` and make `remaining_line` count from the reserved `end`. The existing unit test (`tui.rs:2231`) only asserts the *string* from `remaining_line()` in isolation — it never renders a frame, which is why this shipped. Add a `TestBackend` frame assertion on the last inner row.

**Live proof:** 30-node sidecar, 20×120 pty, 2×`Tab` to History — 14 node rows rendered, `grep -c more` = **0**.

---

### 2. The web empty state is missing (BLOCKER)

**Impact:** A first-time user opening the web UI sees `Pick a node to fork from` — an instruction to do something impossible — and a disabled `Fork selected node` button with no explanation of why. The contract explicitly resolves E3's empty state; the copy does not exist in the SPA.

**Cause:** `loadHistory` disables the button but leaves the sentinel in place:

```js
// web_interface.html:1157-1158
// An empty history has nothing to fork from.
button.disabled = result.roots.length === 0;
```

The sentinel is appended unconditionally at `web_interface.html:1141-1144`, before the node rows, so it survives an empty list.

**Fix:** when `result.roots.length === 0`, build the select from a single disabled option whose text is the contract string `No history yet — generate or edit an image first.`, and drop the sentinel in that branch only. `rg -n "No history yet" web_interface.html` should return one hit after the fix.

---

### 3. SPA node labels break on non-BMP prompts (BRCH-04 parity + typography)

**Impact:** Any prompt containing emoji or other astral characters renders a replacement glyph (U+FFFD / tofu box) and a misaligned column in the History view, while the CLI renders the same node cleanly. BRCH-04 requires identical behavior across surfaces.

**Cause — UTF-16 code units instead of characters:**

```js
// web_interface.html:1115
const clipped = prompt.length > 32 ? prompt.slice(0, 29) + '...' : prompt.padEnd(32);
```

`String.prototype.length` counts UTF-16 code units; `prompt_field` (`history.rs:405-407`) counts `chars()`. Live comparison, same 20×🎉 node:

| Surface | Rendered prompt field |
|---------|----------------------|
| CLI (`history.rs:405`) | `🎉` ×20 + 12 spaces (20 chars, padded to 32) — correct |
| SPA (`web_interface.html:1115`) | `🎉` ×14 + **lone high surrogate** + `...` — U+FFFD in the DOM |

`padEnd(32)` also pads to 32 *units*, so the field is short by one column whenever an astral char is present.

**Fix:**

```js
const chars = Array.from(prompt);
const clipped = chars.length > 32 ? chars.slice(0, 29).join('') + '...' : prompt.padEnd(32);
```

Also add the graceful fallback `history::timestamp_field` already has (`history.rs:435-440`): the JS `node.created.slice(0,10)` assumes RFC 3339 and renders `undefined` for a hand-edited sidecar entry.

---

## Additional Findings

Ordered by severity. All are WARNINGs.

| # | Location | Finding |
|---|----------|---------|
| 4 | `tui.rs:1261`, `1256-1259` | **Detail pane wraps every row; the CTA has no reserved slot.** Contract: "The prompt row is the only wrapped line … every other detail row is one clipped line." `Paragraph::wrap(Wrap{trim:true})` is applied to the whole paragraph, so a long `file` or `created` value wraps and pushes ` [ Fork edit ] ` below the pane floor. There is no `.scroll()` on that paragraph, so it is unreachable. `WordWrapper` with `trim:true` also strips the 2-space indent of continuation lines, breaking the label column. Fix: render the prompt row as its own wrapped paragraph and the other rows via `LineTruncator` (no `.wrap()`), or reserve the last inner row for the CTA. |
| 5 | `tui.rs:1256-1259` | **The fork CTA's focus state is unreachable.** Contract: "`Color::Black` on `Color::Green` + `BOLD` when the pane is focused." The History pane has no focus notion, so the row is permanently the unfocused half (`fg(Green)`). `draw_edit_form` implements the full idiom at `tui.rs:1464-1470` and never connects to the History pane. |
| 6 | `web_interface.html:1186` | **Web fork progress copy diverges from the CLI.** Emits `🖼️ Branch point: ${body.from}` where `from` is the 8-hex node id (`option.value = node.id`, line 1149). The CLI emits `🔀 Forking from {filename} (node {id8})`. Same operation, two different lines on the shared output surface — and the SPA's shows a bare id with no filename. |
| 7 | `web_interface.html:1160-1162` | **A failed `/api/history` is silent.** `catch` logs to `console.error` only; the select keeps stale content and the CTA keeps its previous state. Contract error criterion: "an unreadable store is never rendered as a silent empty tree" — the web surface does exactly that on a fetch failure. |
| 8 | `tui.rs:1226` | **`Color::Yellow` misused for a record state.** The contract reserves Yellow for the *warning* (`[warning]` / `push_log(Warn, …)`). The missing-file detail row is a recorded state, not a warning, and the warning is separately pushed to the log pane — so two semantic roles share one token and the detail pane has no distinct error color. |
| 9 | `tui.rs:1149-1147` (pane width) | **The tree pane clips mid-timestamp.** Live at 120 cols: `a0000029  prompt number 29   img-29.png  2026-1` — 8 of the timestamp's 16 characters are lost. The CLI row is 72 chars; the contract's overflow criterion says the *prompt* clips and "filename and id8 never clipped". Common terminal widths make half the timestamp unreadable. |
| 10 | `web_interface.html:568` | **Bespoke `size="8"` listbox.** Sibling pickers use `size="5"` (`#edit_refs`) or default dropdowns. Contract says "web `<select>` scrolls natively" — satisfied — but the height is not an inventoried value and the history view's rhythm differs from the edit view's. |
| 11 | `web_interface.html:561-601` | **No `.form-grid` wrapper, contrary to the Component Inventory.** The inventory row for `.form-grid` / `.form-group` says "History view layout — two-column grid, `full-width` for the node select". `#historyForm` places its `.form-group` children directly in the form, so the extra fields (prompt, engine, refs, output) stack full-width. This *matches* `#editForm`'s existing structure, so the reuse is coherent — but the contract's declared "two-column grid" layout is unrealized. Contract-internal ambiguity resolved toward the shipped markup. |

---

## Detailed Findings

### Pillar 1: Copywriting (2/4)

Verified verbatim, live where marked **[L]**:

| Contract string | Status |
|---|---|
| `🌳 Iteration history — {output_dir} ({N} nodes, {S} sessions)` | **[L]** `cli.rs:670` — exact |
| `🌳 No history yet — generate or edit an image first.` | **[L]** `cli.rs:660` — exact |
| `[warning] {N} node{s} point{s} at files that are no longer in the output directory.` | **[L]** `cli.rs:683-690` — singular `1 node points at` and plural `{N} nodes point at` both verified |
| `(unknown prompt)` / `(unknown model)` | **[L]** `history.rs:403`, `tui.rs:1232`, `tui.rs:1238`, `web_interface.html:1114` |
| `✗ ` missing marker after connector, before id8 | **[L]** `history.rs:456`, `history_row_text` — single implementation, three surfaces |
| `🔀 Forking from {filename} (node {id8})` | **[L]** `cli.rs:523` — exact |
| `No history yet — generate or edit an image first.` (TUI, alone in ` History `) | **[L]** `tui.rs:1153`, `Color::DarkGray` |
| ` [ Fork edit ] ` | **[L]** `tui.rs:1257` |
| History footer hint, all five clauses | **[L]** `tui.rs:47`, dispatched at `tui.rs:1915` |
| `Fork selected node` | `web_interface.html:599` |
| `Pick a node to fork from` | `web_interface.html:1143` |
| `{N} node(s) in history are missing from disk` | `web_interface.html:1136-1138` |
| `❌ Fork failed: {error}` | `web_interface.html:1239` |
| Three error sentences, CLI vs web | **[L]** byte-identical — see below |

Error-sentence parity, live-tested through the real HTTP handler and the real CLI:

```
WEB  /api/fork {"from":"deadbeef"}   →  No history node or image named 'deadbeef' in
                                          /tmp/ui-audit/gen — run 'imagai history' …
CLI  fork --from deadbeef            →  [Error] No history node or image named 'deadbeef' …
WEB  /api/fork {"from":"5a6b7c8d"}   →  History node 5a6b7c8d points at '2026-10-09-street.png',
                                          which is no longer in /tmp/ui-audit/gen.
CLI  fork --from 5a6b7c8d            →  [Error] History node 5a6b7c8d points at …
```

One implementation (`history::resolve_fork_source`, `history.rs:513`) produces all three sentences; every surface formats, none rewords. The unreadable-store sentence (`history.rs:166`) is pinned by two tests including "an unreadable sidecar is not overwritten by the next record" (`history.rs:662`).

**Why 2 and not 3:** 13 strings verbatim is genuinely strong, and the copy discipline is the best-executed part of this phase. But the web empty state is an explicit, checkable acceptance criterion that failed outright — the sentence does not exist in the SPA — and the web fork progress line diverges from the CLI's. Two copy failures against a contract whose Copywriting table is the normative source.

### Pillar 2: Visuals (2/4)

Correct and verified live:
- `Layout::horizontal([Percentage(55), Percentage(45)])` — identical to `draw_edit_tab` (`tui.rs:1126` vs `1330`).
- `Block::bordered().title(" History ")` and `" Node "` — exactly the contract's mockup.
- Selected row `▶ ` + `Color::Cyan` + `BOLD`; unselected `Color::Gray` — byte-for-byte `draw_source_picker`'s selected-row idiom.
- Detail pane `format!("{:<label_w$}", label)` + `Color::Gray` labels / `Color::White` values — the `label_w` idiom from `draw_edit_form`.
- SPA composes only inventoried classes; `#historyForm`'s skeleton (`.section-title` → `.form-group` set → `.help-text` per field → `.generate-btn`) is structurally identical to `#editForm`.

Failures: the invisible overflow indicator (Priority 1), the over-wide wrap scope (Finding 4), the unreachable focus state (Finding 5), and the clipped timestamp (Finding 9).

### Pillar 3: Color (3/4)

The strongest discipline in the phase.

- **Zero new CSS.** The `<style>` block (lines 7–319) is byte-identical between the Phase-1 base (`fc73c5f`) and HEAD — verified with `diff`. Rule 5 of the contract ("no new CSS rule in `web_interface.html`") is met exactly.
- **Zero new colors.** CLI tree output is plain uncolored text (live-verified — the tree carries no escape codes), keeping `--json` and `ag`-style pipes clean. `colored` appears only as `[Error]` (bold red, `cli.rs:541`) and `[warning]` (bold yellow, `cli.rs:691`), both pre-existing.
- TUI uses only pre-existing `Color::` entries: `Cyan` (selected row), `Gray` (unselected + labels), `DarkGray` (empty state, `… more`, "No node selected"), `White` (values), `Yellow` (missing file), `Green` (CTA).
- `#ff4757` (destructive) is unused — D-21 honored, no delete/prune surface anywhere.
- No hardcoded hex or `rgb(` in any new code.

One misuse: `Color::Yellow` on the missing-file detail row (`tui.rs:1226`) occupies the token the contract reserves for the warning, while the warning itself is a separate `push_log(Warn, …)`.

### Pillar 4: Typography (2/4)

The *ladder* is flawless: exactly two weights (400/600), the declared sizes (16/15/14/21px) all come from pre-existing rules, and no font-size or font-weight class was added. CLI/TUI take the terminal's font with relative emphasis only — no new size or width.

The failure is in the SPA's *column arithmetic*, which is typography-by-fixed-width-columns and is unit-incorrect (Priority 3). The contract's own checker flagged the 14/15/16 ladder as a size ladder within 2px (non-binding); that is not the issue here — the issue is that `padEnd`/`slice`/`length` operate on UTF-16 code units while the Rust reference operates on `chars()`.

### Pillar 5: Spacing (3/4)

No new CSS rule means the pixel scale is definitionally unchanged, and the new markup reuses only inventoried classes. Verified:
- `.form-grid` (30px gap), `.form-group`, `.form-group.full-width` (`grid-column: 1 / -1`), `.section-title` (30px 0 20px 0), `.help-text` (mt 5px), `.generate-btn`, `.output-content`, `.image-preview`.
- `#historyForm` mirrors `#editForm` field-for-field, so the two views share a rhythm.
- TUI: identical `Block::bordered()` panes, the `label {:<10}` + 2-space gutter from `draw_edit_form`, and `Line::from("")` as one blank row before the CTA.
- CLI: 2 spaces between every column, 3 spaces per ancestor level, one blank line per root session — exactly the contract's column rule, live-verified.
- No arbitrary `[Npx]` or `[Nrem]` values anywhere in the new code.

Deductions: the bespoke `size="8"` listbox (Finding 10) and the missing `.form-grid` two-column layout (Finding 11).

### Pillar 6: Experience Design (2/4)

State coverage, verified live:

| Category | CLI | TUI | Web |
|----------|-----|-----|-----|
| empty | ✓ `🌳 No history yet…` | ✓ alone in ` History `, `DarkGray` | ✗ **sentence absent**; CTA disabled only |
| populated | ✓ | ✓ 14 rows live | ✓ |
| zero-one-many | ✓ `1 node` / `6 nodes`, sessions dropped at 1 | ✓ | ✓ |
| partial | ✓ `(unknown prompt)` / `(unknown model)` live | ✓ live, both in the detail pane | prompt only |
| overflow | ✓ prompt clipped to 29+`...` | ✗ **`… {n} more` invisible** | ✓ native scroll |
| long-text | n/a | ✗ whole pane wraps, CTA displaced | ✓ unbounded help-text |
| error | ✓ three sentences verbatim | ✓ via `push_log` | ✓ `{success:false,error}` + `❌ Fork failed:` |
| loading | ✓ n/a | ✓ n/a | ✓ existing spinner |

Also verified: BRCH-01/02/03/04 (fork from node id live-produced a new child of `1a2b3c4d` with all four siblings visible and correct oldest-first ordering); the `PickerOrigin` loop guard (`tui.rs:899-905`); exhaustive `tab_surface` with no `_ => about` catch-all (`tui.rs:65-74`); `is_plain_filename` refusing `../etc/passwd` without reducing it; and `cargo fmt --check` / `cargo clippy --all-targets` both clean.

Deductions: Priority 1 (invisible overflow), Priority 2 (missing web empty state), Finding 4 (unscrollable detail pane), Finding 7 (silent history-read failure on web).

---

## Files Audited

- `src/tui.rs` — History tab, tree pane (`draw_history_tree` 1132-1188), detail pane (`draw_history_detail` 1190-1263), `load_history` 1004-1030, `history_move` 1032-1044, `history_select` 1054-1086, `open_history_pick` 1090-1094, `handle_history_key` 1096-1118, `TABS`/`HISTORY_FOOTER_HINT`/`PICK_FROM_HISTORY` 43-50, `tab_surface` 65-74, `draw_source_picker` 1548-1612, `scroll_selection` 925-944, footer dispatch 1905-1918, `flatten_nodes` 1928-1943, `remaining_line` 1954-1956, tests 2099-2360
- `src/cli.rs` — `ForkArgs`/`HistoryArgs` 166-200, `cmd_fork` 533-621, `resolve_from` 501-511, `print_fork_line` 516-526, `cmd_history` 628-638, `render_history_text` 658-694
- `src/history.rs` — `prompt_field` 401-411, `history_row_text` 447-464, `resolve_fork_source` 513-574, `node_source` 578-594, `read_records` 160-172, `load_view` 230-282, `timestamp_field` 435-440, `adopt_root` 479-498
- `src/web.rs` — `ForkPayload` 91-109, routes 136-145, `is_plain_filename` 271-275, `result_items_json` 281-294, `history` handler 469-483, `output_dir_name` 494-499, `fork` handler 506-660
- `web_interface.html` — `<style>` block 7-319 (diffed), `#historyForm` 563-601, `historyNodeLabel` 1112-1119, `loadHistory` 1122-1163, `executeFork` 1166-1246, submit handler 1283-1308
- `.planning/phases/02-branching-iteration-history/02-UI-SPEC.md` (the contract), `02-CONTEXT.md`, `02-01..05-PLAN.md` / `-SUMMARY.md`, `02-REVIEW.md`, `02-VERIFICATION.md`, `02-UAT.md`
- Reference: `ratatui-widgets-0.3.2/src/paragraph.rs` (`render_paragraph` 419-447, `render_lines` 450-459), `ratatui-widgets-0.3.2/src/reflow.rs` (`WordWrapper` 202-234, `process_input_line` 85-200)
- Live: `target/debug/imagai` against seeded sidecars in `/tmp/ui-audit/{gen,big,miss,emoji,par}` and a local mock provider on `127.0.0.1:8199`

No source file was modified. Screenshot storage was git-gated before any attempt: `.planning/ui-reviews/.gitignore` created with `*.png` and `interaction/` patterns.
