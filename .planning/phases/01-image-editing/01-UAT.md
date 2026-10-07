---
status: complete
phase: 01-image-editing
source: [01-00-SUMMARY.md, 01-01-SUMMARY.md, 01-02-SUMMARY.md, 01-03-SUMMARY.md, 01-04-SUMMARY.md]
started: 2026-10-07T19:15:00+03:30
updated: 2026-10-07T19:15:00+03:30
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
