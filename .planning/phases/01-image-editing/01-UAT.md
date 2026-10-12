---
status: complete
phase: 01-image-editing
source: [01-00-SUMMARY.md, 01-01-SUMMARY.md, 01-02-SUMMARY.md, 01-03-SUMMARY.md, 01-04-SUMMARY.md]
started: 2026-10-07T19:15:00+03:30
updated: 2026-10-12T12:06:19Z
---

## Current Test

[testing complete]

## Tests

### 1. Supply-chain gate left manifests untouched (01-00 D2)
expected: Cargo.lock and Cargo.toml byte-unchanged by plan 01-00
result: pass
source: automated
coverage_id: D2

### 2. Edit from explicit path saves source-derived file (01-01 D1)
expected: `imagai edit --image <path> -p "<change>"` saves `<source-stem>-edit.<ext>` into output_dir and prints the saved path
result: pass
source: automated
coverage_id: D1

### 3. Saved PNG carries lineage chunks (01-01 D2)
expected: Saved PNG carries Prompt, Model and Source tEXt chunks in order, Source holding the filename only
result: pass
source: automated
coverage_id: D2

### 4. Multipart images/edits wire shape (01-01 D3)
expected: Provider receives multipart/form-data on images/edits with model, prompt and one image[] part per image
result: pass
source: automated
coverage_id: D3

### 5. Incapable engine fails before any request (01-01 D4)
expected: Engine that cannot take image input exits 1 naming engine and model, issuing no HTTP request
result: pass
source: automated
coverage_id: D4

### 6. Multi-reference order on both transports (01-01 D5)
expected: `--ref` adds references in user order, source first, on multipart and chat-vision transports
result: pass
source: automated
coverage_id: D5

### 7. Over-limit refs reported, nothing sent (01-01 D6)
expected: More than 16 images is a reported failure naming both counts, not a truncation, and sends nothing
result: pass
source: automated
coverage_id: D6

### 8. Generate flow unchanged (01-01 D7)
expected: `imagai generate` behaviour, request body and exit codes unchanged
result: pass
source: automated
coverage_id: D7

### 9. Approved crates only in lockfile (01-01 D8)
expected: Only mime_guess 2.0.5 and unicase 2.10.0 entered Cargo.lock with approved checksums
result: pass
source: automated
coverage_id: D8

### 10. Auto-pick newest image without --image (01-02 D1)
expected: `imagai edit -p "<change>"` with no `--image` edits newest image, prints `Using most recent image` plus filename, saves `<stem>-edit.png`
result: pass
source: automated
coverage_id: D1

### 11. Empty output dir names dir and flag (01-02 D2)
expected: Empty `output_dir` with no `--image` exits 1 naming both the output directory and `--image`
result: pass
source: automated
coverage_id: D2

### 12. Explicit --image wins silently (01-02 D3)
expected: Explicit `--image` always wins over auto-pick and produces no pick line on stdout
result: pass
source: automated
coverage_id: D3

### 13. Listing helper ordering and filtering (01-02 D4)
expected: `list_output_images` returns newest-first, ignores non-image files, tiebreaks identical mtimes by path
result: pass
source: automated
coverage_id: D4

### 14. Gallery shape unchanged after helper move (01-02 D5)
expected: `GET /api/images` still lists every output image newest-first with the same five keys
result: pass
source: automated
coverage_id: D5

### 15. POST /api/edit saves previewable edit (01-03 D1)
expected: `POST /api/edit` with prompt + source returns saved, previewable edited image via shared core
result: pass
source: automated
coverage_id: D1

### 16. Traversal refused with no request (01-03 D2)
expected: Traversal-shaped `source`, `refs` and `output` values refused with no provider request issued
result: pass
source: automated
coverage_id: D2

### 17. Edit error paths name the cause (01-03 D3)
expected: Missing source names the file; 17 refs reports 17-vs-16; dall-e-3 fails naming the model, no request sent
result: pass
source: automated
coverage_id: D3

### 18. Edit form served with wired pickers (01-03 D4)
expected: `GET /` serves the edit form with source and reference pickers wired to `/api/edit`
result: pass
source: automated
coverage_id: D4

### 19. TUI Edit tab submits through shared core (01-04 D1)
expected: Edit tab reachable via Tab/BackTab; submit sends through `generate_image_core` with saved path in log pane
result: pass
source: automated
coverage_id: D1

### 20. TUI source picker behaviour (01-04 D2)
expected: Source picker lists output_dir images newest first; arrows move highlight, Enter fills Source, Esc leaves it unchanged
result: pass
source: automated
coverage_id: D2

### 21. TUI refs and field errors (01-04 D3)
expected: Comma-separated refs reach provider in order with empties dropped; empty instruction, missing ref, unknown engine each abort with a naming log
result: pass
source: automated
coverage_id: D3

### 22. Other TUI tabs unchanged (01-04 D4)
expected: Generate, Engines and About tabs behave exactly as before
result: pass
source: automated
coverage_id: D4

### 23. End-to-end browser edit flow
expected: Generate one image in the browser, pick it as the edit source, submit a change instruction, and see the rendered edited image in the page. Needs an edit-capable engine configured; without one, the pickers still populate from the gallery and the engine error names the engine.
result: pass
reported_by: user
verified_at: 2026-10-07T19:20:00+03:30
### 24. Interactive TUI walkthrough
expected: Tab to the Edit tab, open the source picker and select an image, type a change instruction, submit and see the saved path in the log pane. The Engines tab `f` key still fetches the model list.
result: pass
reported_by: user
verified_at: 2026-10-07T19:22:00+03:30

## Summary

total: 24
passed: 24
issues: 0
pending: 0
skipped: 0

## Gaps

[none yet]

## Notes

- 01-00 D1 (human approve decision for mime_guess 2.0.5 + unicase 2.10.0) was recorded in 01-00-SUMMARY.md at the wave-0 gate; pre-condition, not re-tested here.
- UI checkpoints: 0 auto-verified (no browser MCP in this session), 2 queued for manual review (tests 23–24).
- Cold-start injection skipped: no server/bootstrap file patterns among changed paths (Rust CLI, no startup scripts).

## Re-verification (2026-10-10, stale-report refresh)

Phase 2's security fixes (commit 56984f8 and follow-ups) changed five covered files after
the 2026-10-09 verification (`src/cli.rs`, `tests/cli.rs`, `src/core.rs`, `src/web.rs`,
`tests/web.rs`), so all 24 checks were re-run against the current tree.

- Checks 1–22 (automated): **pass** — full suite green in a single run (`cargo test`: lib 80 /
  cli 43 / core 8 / web 35, 0 failed; 166 total), `cargo fmt --check` clean,
  `cargo clippy --all-targets` zero warnings, plus live binary runs against an in-process mock
  provider covering explicit-path edit, auto-pick, multi-ref wire order, the 16-image cap on
  CLI and web, the capability gate, the `-edit` filename scheme, PNG lineage chunks, the
  `--from` plain-filename guard (a separator-bearing record is refused with nothing sent),
  and exit-1-on-failure.
- Check 23 (browser edit flow): **pass** — re-verified live in a real browser against the
  running `imagai web`: pickers populated from the server, instruction submitted through the
  form, saved path shown, edited image rendered in the page.
- Check 24 (TUI walkthrough): **pass (scoped)** — pty-driven session verified the Edit tab is
  reachable via Tab, the source picker opens and lists output-dir images newest-first, arrows
  move the picker selection, and Enter commits the pick into the source field. The
  type→submit→saved-path tail is covered by the 18 passing `tui::tests` and the user's
  2026-10-07 walkthrough.

Result: 24/24 pass (22 automated + 2 walkthroughs). No regression from the shared-file changes.
01-VERIFICATION.md regenerated through the fingerprint command the same session
(digest `v3:sha256:72f289a1b221a622c2a89cbaa23978e8cd887f29dd354f38dfe2740496396673`, status `passed`).

## Re-verification (2026-10-12, Phase 3 shared-file refresh)

Phase 3 (multi-vendor providers) rewrote nine covered files after the 2026-10-10
pass (`src/cli.rs`, `src/models.rs`, `src/provider.rs`, `src/tui.rs`, `src/web.rs`,
`tests/cli.rs`, `tests/core.rs`, `tests/web.rs`, `tests/common/mod.rs`), so all 24
checks were re-run against the live tree at HEAD `23ce426`.

- Checks 1–22 (automated): **pass** — full suite green in a single `cargo test` run
  (lib 108 / cli 52 / core 26 / web 40 = 226, 0 failed), `cargo fmt --check` clean,
  `cargo clippy --all-targets -D warnings` exit 0, the named per-check integration
  tests (compiled binary vs the in-process mock) all pass, and fresh live binary runs
  against a standalone capture mock reproduced explicit-path edit (`src-edit.png`,
  wire = `model`/`prompt`/`image[]` only), auto-pick (`Using most recent image`),
  the 17-vs-16 over-limit refusal with zero provider posts, and the
  `source → ref-a → ref-b` wire order.
- Check 23 (browser edit flow): **pass (carried forward)** — interactive browser
  re-run not performed by the verifier this round (no browser automation available);
  its covering automated equivalents are green (`edit_form_is_served_with_source_and_ref_pickers`,
  `edit_endpoint_*` incl. the traversal trust boundary and 16-cap), and Phase 3 did not
  change the `/api/edit` trust boundary. User walkthrough stands from 2026-10-07.
- Check 24 (TUI walkthrough): **pass (carried forward)** — the type→submit→saved-path
  tail is covered by the 19 passing `tui::tests` (Edit tab, pickers, History tab),
  which Phase 3's `src/tui.rs` changes kept green. User walkthrough stands from 2026-10-07.

Result: 24/24 pass. 01-VERIFICATION.md regenerated through the fingerprint command
(digest `v3:sha256:554b73f824736b24878d8725fcf34e33997e39635820417faa05e4ec556e0570`,
status `passed`).
