---
status: complete
phase: 02-branching-iteration-history
source: [02-01-SUMMARY.md, 02-02-SUMMARY.md, 02-03-SUMMARY.md, 02-04-SUMMARY.md, 02-VERIFICATION.md]
started: 2026-10-10T03:20:00+03:30
updated: 2026-10-12T12:06:19Z
result: 24 passed, 0 failed
---

## Current Test

[testing complete]

## Tests

Phase 1's 24 checks are regression checks for this phase; all 24 re-ran green
(`cargo test` 158 passed / 0 failed, plus the named Phase-1 test groups below).
Checks 1–12 cover Phase 2's new behaviour. Every `source: automated` entry was
executed, not inferred — CLI items through the built binary against a local mock
OpenAI-compatible provider on `127.0.0.1:8731` (web on `127.0.0.1:8734/8735`),
web items through real HTTP POST/GET.

### 1. Sidecar written for a generate (02-01)
expected: after `imagai generate -p "a red cat on a sofa"`, `<output_dir>/.imagai-history.json` exists with exactly one record — prompt, model `dall-e-3`, 8-hex id, `parent: null`
result: pass
source: automated
evidence: live run → `✅ Image 1 generated successfully! Saved to: /tmp/imagai_uat_out2/a_red_cat_on_a_sofa_20261010_065000.png`; sidecar created with 1 record; `generate_writes_history_sidecar` ok

### 2. Edit becomes a child of its source (02-01, D-09)
expected: `imagai edit --from <src> -p "make it blue"` writes a second record whose `parent` is the first record's id and whose filename ends `-edit.png`
result: pass
source: automated
evidence: live run → `✅ … Saved to: /tmp/imagai_uat_out2/a_blue_dog_…-edit.png`; tree shows the child under the source root; `edit_writes_child_node_under_its_source` ok

### 3. Deleted file stays listed, marked missing (02-01, D-20)
expected: deleting a recorded image keeps its node in the tree with the `✗ ` marker and a plural warning line
result: pass
source: automated
evidence: live run E → `   `- ✗ 10b1fe48  make it a sunset  …-edit.png  2026-10-10 03:20` + `[warning] 2 nodes point at files that are no longer in the output directory.`

### 4. Moved folder keeps history valid (02-01, D-23)
expected: `cp -R output_dir newdir; IMAGAI__OUTPUT_DIR=newdir imagai history` renders every node
result: pass
source: automated
evidence: live run M → `🌳 Iteration history — /tmp/imagai_moved (2 nodes, 2 sessions)` with both roots, exit 0

### 5. Out-of-dir output stays untracked (02-01, D-22)
expected: `imagai generate -o ../escape.png` writes the file but leaves the sidecar unchanged
result: pass
source: automated
evidence: live run N → file created at `/tmp/…/../imagai_escape.png`; sidecar record count stayed at 2; `out_of_dir_result_is_not_recorded` ok

### 6. `imagai history` renders the ASCII tree grammar (02-02, UI-SPEC E1)
expected: roots newest-first, children oldest-first, id8 + 32-char prompt column + filename + `YYYY-MM-DD HH:MM`; `+- ` for a non-last child, `` `- `` for the last, three-space depth-1 indent, no connector at depth 0
result: pass
source: automated
evidence: live run with two children under one root →
```
c5d9a61e  a red cat on a sofa               a_red_cat_on_a_sofa_….png  2026-10-10 03:50
   +- adbe764a  make it blue                      …-edit.png  2026-10-10 03:50
   `- 94d8990d  make it green                     …-edit.png  2026-10-10 03:50
```
`history_tree_prints_connectors` ok

### 7. `imagai history --json` machine view (02-02)
expected: emits `output_dir`, `nodes`, `missing`, nested `roots` with `children`
result: pass
source: automated
evidence: live run 4 → `{"output_dir":"/tmp/imagai_uat_out","nodes":1,"missing":0,"roots":[{…"children":[]}]}`

### 8. `imagai fork --from <id8>` starts an edit (02-02)
expected: resolving an 8-hex node id forks an edit that saves under `<stem>-edit.<ext>`
result: pass
source: automated
evidence: live run 5 → `✅ Image 1 generated successfully! Saved to: …/a_red_cat_on_a_sofa_20261010_064913-edit.png`, exit 0; `fork_from_node_id_records_child` ok

### 9. Two forks from one source keep both images (02-02, D-14/D-15 — G-1 closed)
expected: forking twice from one source leaves both images on disk under distinct names
result: pass
source: automated
evidence: G-1 was the original fail/blocker (`src/core.rs:85` + the unreachable `n>1` arm) and was closed by 02-05 (`5264a55` + `f5cf9c2`): `record_result` now probes `history::filename_taken` and bumps through `suffix_numbered`. Live run at `23ce426` — `fork --from source.png` then `fork --from <node-id>` → `source-edit.png` **and** `source-edit_1.png`, two files, two records, no overwrite, no `✗`, no warning; `two_forks_from_one_source_keep_both_images` and `fork_keeps_both_branches_visible` both pass.

### 10. Unknown / ambiguous / deleted / unreadable copy (02-02)
expected: each of the four failure states prints its exact Copywriting Contract sentence and exits 1
result: pass
source: automated
evidence: live run G `Error: Could not read history at /tmp/…/.imagai-history.json: expected ident at line 1 column 2`, exit 1; live run H `History node 4424ac29 points at 'a_red_cat_on_a_sofa_….png', which is no longer in /tmp/imagai_uat_out3.`, exit 1; live run I `Ambiguous node id prefix 'aaa' — candidates: aaa11111, aaa22222`, exit 1; live run 7 `No history node or image named 'nope_not_there' in … — run 'imagai history' to see what is available.`, exit 1

### 11. Empty / single-node / multi-session history copy (02-02, UI-SPEC E1)
expected: zero nodes → the empty sentence; one root → no connector and no session clause; many → plural `nodes`/`sessions`
result: pass
source: automated
evidence: live run L → `🌳 No history yet — generate or edit an image first.`; live run K → `🌳 Iteration history — … (1 node)` with a bare root row; live run D → `(4 nodes, 2 sessions)`

### 12. Adopted untracked file renders partial-state copy (02-02, UI-SPEC E4)
expected: `imagai fork --from untracked.png` adopts the file as a root and renders `(unknown prompt)` / `(unknown model)` rather than a blank field
result: pass
source: automated
evidence: live run J2 →
```
4a5c7754  (unknown prompt)                  untracked.png  2026-10-10 03:22
   `- 922ea7f5  make it blue                      untracked-edit.png  2026-10-10 03:22
```

### 13. TUI History tab at index 2 with the same order (02-03)
expected: `TABS` gains `History` at index 2 between Edit and Engines; `history_rows` walks the same `HistoryView` as the CLI renderer
result: pass
source: automated
evidence: `src/tui.rs:43` `const TABS: [&str; 5] = ["Generate","Edit","History","Engines","About"]`; `tabs_has_history_at_index_two`, `history_tree_lines_use_the_contract_connectors`, `history_tree_marks_missing_nodes`, `history_scroll_marks_remaining_rows` all ok

### 14. TUI Enter forks the selected node (02-03)
expected: Enter writes the node filename into `edit_source`, switches to the Edit tab, and pushes the same `Source image: {name}` info log `picker_select` emits
result: pass
source: automated
evidence: `history_enter_forks_selected_node_into_edit`, `history_enter_refuses_missing_node`, `history_enter_completes_pending_pick` all ok; `src/tui.rs:1054-1088` `history_select`

### 15. TUI pick-from-history round trip (02-03)
expected: the Edit tab source picker's first row is ` Pick from history… `; selecting it moves to the History tab with a pending pick; Enter there writes the chosen filename and restores the Edit tab; Esc leaves `edit_source` untouched either way
result: pass
source: automated
evidence: `picker_history_entry_moves_to_history_tab`, `picker_and_history_escape_leave_source_unchanged`, `picker_lists_newest_first` all ok

### 16. Tab re-index left no catch-all (02-03, T-02-08)
expected: `draw`/`draw_footer` cover 0..=4 explicitly; the old `_ =>` arm is gone
result: pass
source: automated
evidence: `rg -n '_ =>' src/tui.rs | rg 'tab|about'` → no output; footer hint exact: `q Quit | Tab switch tab | ↑/↓ navigate | Enter fork from node | p pick from history` (`src/tui.rs:47`)

### 17. `GET /api/history` shape (02-04)
expected: 200 with `success`, `output_dir`, `nodes`, `missing`, nested `roots`; empty dir is success with `nodes: 0`
result: pass
source: automated
evidence: live `GET /api/history` → `{'missing':0,'nodes':2,'output_dir':'imagai_uat_web3','roots':[...2]}`; node keys `['children','created','filename','id','missing','model','parent','prompt']`; `output_dir` is the bare directory name, not an absolute path; `history_endpoint_*` (4 tests) ok

### 18. `POST /api/fork` returns the `/api/edit` shape (02-04)
expected: fork response keys identical to edit's; `saved_path` ends `-edit.png`; `image_data` is a data URI
result: pass
source: automated
evidence: live fork keys `['error','image_b64_json','image_data','index','saved_path','success']` == live edit keys; `saved_path` tail `-edit.png`; `image_data` prefix `data:image/png;base64,`; `fork_endpoint_returns_result_items` ok

### 19. Web trust boundary on every path-shaped field (02-04, T-02-09)
expected: a separator in `from`, `refs` or `output` is refused before any read or network call; empty/missing `prompt`, unknown `from`, >16 refs and a missing `from` each return `success:false`
result: pass
source: automated
evidence: live probes — `../../.env`, `..\..\env`, `../../escaped.png`, `../outside.png` → `success:false` with `'…' is not a plain filename inside output_dir`; `empty prompt`/`missing prompt field` → `Prompt cannot be empty.`; `unknown from` → `No history node or image named 'nope_not_there' in …`; `17 refs` → `17 images requested but the provider accepts at most 16`; `no from` → `No source named — pass 'from' with a node id or filename.`; no escaped file on disk; mock counters unchanged

### 20. Lineage store not served over HTTP (02-04, WR-03)
expected: `GET /api/images/.imagai-history.json` (and the `.tmp`) returns 404 and never leaks prompts/models
result: pass
source: automated
evidence: live `GET /api/images/.imagai-history.json` and `.imagai-history.json.tmp` → 404 `{"error":"Endpoint not found","success":false}`; `serve_image_refuses_the_lineage_sidecar` asserts the seeded `SECRET-PROMPT` is absent from the body and ok

### 21. SPA history view served and wired (02-04, UI-SPEC E3)
expected: `GET /` contains `historyForm`, `history_node`, `historyBtn` labelled `Fork selected node`, the `Pick a node to fork from` sentinel, a disabled option reporting the missing count, `fetch('/api/fork'`, `fetch('/api/history'`, `loadHistory()` and `executeFork()`; no new CSS rule
result: pass
source: automated
evidence: live `GET /` — every token present; `history_form_is_served_with_fork_controls` ok; CSS rule count `rg -c '^\s{8}\.[a-z-]+ \{' web_interface.html` = 22, identical at `f9ec7bd` (pre-implementation), `35cd140`, `592c440`, `f688365`, `7f0d820` and HEAD

### 22. Browser fork refreshes the tree without a reload (02-04)
expected: `executeFork()` calls `loadHistory()` on success, so the new node and any missing-count change appear without a reload
result: pass
source: automated
evidence: `web_interface.html:1237` `loadHistory()` after the fork; live `GET /api/history` reported `nodes 2 → 4` across a fork with no server restart

### 23. Phase 1 regression — edit from an explicit path saves a source-derived file
expected: `imagai edit --image <path> -p "<change>"` saves `<stem>-edit.<ext>` into `output_dir` and prints the saved path
result: pass
source: automated
evidence: `cargo test --test cli edit_saves_source_derived_image_with_lineage_metadata` ok; live run B confirmed

### 24. Phase 1 regression — traversal refused, generate flow, terminals, gallery
expected: Phase 1's remaining 22 checks (supply-chain gate, PNG lineage chunks, multipart wire shape, incapable-engine gate, multi-ref order, over-limit refs, generate unchanged, approved crates only, auto-pick, empty-dir message, explicit-image precedence, listing helper, gallery shape, POST /api/edit, edit error paths, edit form pickers, TUI Edit tab, TUI picker, TUI refs, other TUI tabs, browser edit flow, TUI walkthrough) hold
result: pass
source: automated
evidence: `cargo test` — 76 lib + 39 cli + 8 core + 35 web = 158 passed, 0 failed; every Phase-1-named test group present in the green run

## Summary

| Metric | Value |
|--------|-------|
| Total checks | 24 |
| Passed | 24 |
| Failed | 0 |
| Phase 1 regressions | 24/24 re-ran green (carried as #23 and #24) |

## Re-verification (2026-10-12, Phase 3 shared-file refresh)

Phase 3 (multi-vendor providers) rewrote shared covered files (`src/cli.rs`,
`src/core.rs`, `src/tui.rs`, `src/web.rs`, `tests/*`) after this UAT's last pass,
so all 24 checks were re-run against the live tree at HEAD `23ce426`.

- Checks 1–12, 13–24 (automated): **pass** — full suite green in a single
  `cargo test` run (lib 108 / cli 52 / core 26 / web 40 = 226, 0 failed),
  `cargo fmt --check` clean, `cargo clippy --all-targets -D warnings` exit 0, the
  named per-check integration tests (compiled binary vs the in-process mock) all
  pass, and fresh live binary runs against a standalone loopback capture mock
  reproduced: generate/edit/fork each recording a node with `parent` linkage;
  two forks from one source → `source-edit.png` + `source-edit_1.png` (distinct,
  G-1 holds); the ASCII history tree; the D-23 relative-filename sidecar; the
  fal-format edit failing fast naming engine+model+format; and the
  `../source.png` traversal record refused by `resolve_from`'s `node_path`
  reduction with `is not a plain filename`.
- Check #9 (G-1): **pass** — see above; closed on HEAD, re-confirmed live this round.
- Phase 1 regressions (#23–#24): **pass** — Phase 1's edit/traversal/generate/gallery
  checks present and green in the 226-test run; browser (#23) and interactive-TUI
  (#24) walkthroughs carried forward, with their covering automated equivalents
  (web edit-form/endpoint tests; 19 `tui::tests`) green this round.

Result: **24/24 pass**. 02-VERIFICATION.md regenerated through the fingerprint
command (digest `v3:sha256:98359b0a5544166fac5f053e205b1b289f76983f1cc4b4be66a90fdff9aa7ee0`,
status `passed`).

## Blockers

None. G-1 (check #9) was closed by Phase 02-05 (`5264a55` + `f5cf9c2`) and
re-confirmed green against HEAD `23ce426` this session. No source was modified.

---

_Updated: 2026-10-12T12:06:19Z_
