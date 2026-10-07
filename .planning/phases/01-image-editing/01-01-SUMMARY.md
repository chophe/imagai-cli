---
phase: 01-image-editing
plan: 01
subsystem: api
tags: [reqwest, multipart, images-edits, openrouter, png-metadata, clap, capability-gate]

requires:
  - phase: 01-image-editing
    plan: "01-00"
    provides: "Human `approve` decision at the wave-0 supply-chain gate, pinning mime_guess 2.0.5 and unicase 2.10.0 by exact sha256"
provides:
  - "`imagai edit --image <path> -p \"<change>\"` end to end: multipart POST to images/edits, saved as `<source-stem>-edit.<ext>` in the flat output_dir"
  - "`EditTransport` + `edit_transport(name, cfg)` capability gate (D-05) that bails naming the engine and model before any socket opens"
  - "`generate_edits` transport covering both wire shapes — multipart `image[]` parts and OpenRouter chat-vision data-URI parts"
  - "`source_image` / `ref_images` typed fields on `ImageGenerationRequest` (fixes the documented per-frontend `extra_params` duplication anti-pattern)"
  - "PNG `Source` tEXt lineage chunk with ISO-8859-1 lossy mapping (D-07)"
  - "`list_output_images` newest-first helper plus `IMAGE_EXTENSIONS` moved from web.rs and extended with `gif`"
  - "Mock raw-bytes request capture, making multipart wire-format assertions possible with no new dependency"
  - "Repeatable `--ref` flag (D-04) with a hard 16-image limit, source always first"
affects: [01-02, 01-03, 01-04, phase-02-branching, phase-03-providers]

actuals:
  tokens: 13254
  tasks: 3
  commits: 4
plan_head_before: 798dfcc0d89576f297b1f9663ff70b61b98face0

tech-stack:
  added: []
  patterns:
    - "Capability gate decides the wire shape before any socket opens: `edit_transport(name, cfg)` takes the engine *name* so its bail message can identify the engine (D-05)"
    - "Ordered image list built once at the wire boundary — source first, then user references — shared by both the multipart and chat-vision transports, so neither can drift"
    - "User-supplied paths formatted with `{path:?}` (byte-safe Debug) and never routed through `provider::truncate`, which panics on multi-byte input"
    - "Mock captures raw request `Bytes` alongside the parsed JSON view, so multipart wire assertions need no multipart parser"

key-files:
  created: []
  modified:
    - "Cargo.toml"
    - "Cargo.lock"
    - "src/models.rs"
    - "src/provider.rs"
    - "src/core.rs"
    - "src/cli.rs"
    - "src/utils.rs"
    - "src/web.rs"
    - "src/tui.rs"
    - "tests/common/mod.rs"
    - "tests/cli.rs"
    - "tests/core.rs"

key-decisions:
  - "`source_image: Option<String>` stays singular and primary; `ref_images: Vec<String>` is an adjunct list rather than promoting the whole thing to `sources: Vec<String>` — a promoted plural would make D-06 (which stem names the file) and D-07 (one Source lineage field) unanswerable"
  - "The capability gate routes on a `gpt-image` model substring, never on `crate::cli::is_image_model` — that heuristic is display-only and `dall-e-3` passes it while still being unable to edit"
  - "The multipart form carries only `model`, `prompt` and `image[]`; `quality`, `style`, `response_format` and `n` belong to `/images/generations` and are rejected by the edits endpoint"
  - "Over 16 images is a reported failure, not a truncation — dropping references silently would hand the model a different request than the user asked for"
  - "Reference images are never sorted or deduplicated: reordering changes the blend, and silently dropping a duplicate hides a user mistake"
  - "The non-zero exit guard lives in `cmd_edit` only, never in the shared print loop, so `imagai generate` keeps its existing exit behaviour byte-identical (ROB-01 stays Phase 4's)"

patterns-established:
  - "Edit vs generate dispatch on `request.source_image.is_none()` inside `generate_image_core`, so the generation path is provably untouched rather than merely believed to be"
  - "Request DTOs carry typed image-input fields; edit fields are never re-assembled per frontend"
  - "Latin-1 lossy mapping before any PNG tEXt write, with the chunk skipped entirely when nothing is representable"

requirements-completed: [EDIT-01, EDIT-03]

coverage:
  - id: D1
    description: "`imagai edit --image <path> -p \"<change>\"` saves `<source-stem>-edit.<ext>` into output_dir and prints the saved path"
    requirement: EDIT-01
    verification:
      - kind: e2e
        ref: "tests/cli.rs#edit_saves_source_derived_image_with_lineage_metadata"
        status: pass
      - kind: manual_procedural
        ref: "full-stack local run: `IMAGAI__OUTPUT_DIR=<dir> cargo run -- edit --image <dir>/source.png -p \"make it sunset\"` printed `Saved to: <dir>/source-edit.png`"
        status: pass
    human_judgment: false
  - id: D2
    description: "The saved PNG carries Prompt, Model and Source tEXt chunks in order, with Source holding the filename only and never the output directory path"
    requirement: EDIT-01
    verification:
      - kind: e2e
        ref: "tests/cli.rs#edit_saves_source_derived_image_with_lineage_metadata (asserts source.png present, output dir path absent)"
        status: pass
      - kind: unit
        ref: "src/utils.rs#inject_png_metadata_adds_source_lineage"
        status: pass
      - kind: manual_procedural
        ref: "chunk walk of the saved PNG: tEXt Prompt=make it sunset, tEXt Model=gpt-image-1, tEXt Source=source.png"
        status: pass
    human_judgment: false
  - id: D3
    description: "The provider receives multipart/form-data on images/edits carrying only model, prompt and one image[] part per supplied image"
    requirement: EDIT-01
    verification:
      - kind: e2e
        ref: "tests/cli.rs#edit_saves_source_derived_image_with_lineage_metadata (raw body contains image[]; zero images/generations requests)"
        status: pass
      - kind: other
        ref: "source audit: generate_edits contains 0 occurrences of quality, style, response_format or \"n\""
        status: pass
    human_judgment: false
  - id: D4
    description: "An engine that cannot take image input exits 1 naming that engine and its model, issuing no HTTP request at all"
    requirement: EDIT-01
    verification:
      - kind: e2e
        ref: "tests/cli.rs#edit_rejects_incapable_engine_without_sending_a_request (asserts mock.requests() is empty)"
        status: pass
      - kind: unit
        ref: "src/provider.rs::edit_transport_rejects_dall_e, edit_transport_rejects_stability, edit_transport_error_names_engine_and_model"
        status: pass
    human_judgment: false
  - id: D5
    description: "`--ref` adds references in the user's order, source first, on both the multipart and the chat-vision transport"
    requirement: EDIT-03
    verification:
      - kind: e2e
        ref: "tests/cli.rs#edit_sends_multiple_reference_images_in_order (three image[] parts, ascending byte offsets, source/ref-a/ref-b in order)"
        status: pass
      - kind: e2e
        ref: "tests/core.rs#edit_via_openrouter_chat_sends_data_uri_parts (4 content parts, text at index 0, each a data:image/png;base64, URI, zero /v1/images/edits requests)"
        status: pass
    human_judgment: false
  - id: D6
    description: "More than 16 images is a reported failure naming both counts, not a truncation, and sends nothing"
    requirement: EDIT-03
    verification:
      - kind: e2e
        ref: "tests/cli.rs#edit_rejects_more_than_sixteen_images (stderr carries 17 and 16; mock.requests() empty)"
        status: pass
    human_judgment: false
  - id: D7
    description: "`imagai generate` behaviour, request body and exit codes are unchanged"
    requirement: EDIT-01
    verification:
      - kind: e2e
        ref: "tests/cli.rs#generate_reports_api_errors still asserts .success() (generate's exit-0 behaviour is deliberately preserved; ROB-01 is Phase 4's)"
        status: pass
      - kind: other
        ref: "all 51 pre-existing tests still pass; suite grew 51 -> 68"
        status: pass
    human_judgment: false
  - id: D8
    description: "The approved supply-chain gate is honoured: only mime_guess 2.0.5 and unicase 2.10.0 entered Cargo.lock, with the exact approved checksums"
    requirement: EDIT-01
    verification:
      - kind: other
        ref: "Cargo.lock diff is +18 lines with exactly two added name entries; both sha256 values match plan 01-00 byte-for-byte"
        status: pass
      - kind: other
        ref: ".planning/phases/01-image-editing/01-00-SUMMARY.md records Outcome: approve"
        status: pass
    human_judgment: false

duration: 36 min
started: 2026-10-07T13:33:08Z
completed: 2026-10-07T14:09:18Z
status: complete
---

# Phase 1 Plan 1: Image Editing Transport Summary

**`imagai edit` end to end — a multipart `image[]` upload to `images/edits`, saved as `<source-stem>-edit.<ext>` carrying Prompt/Model/Source lineage, behind a capability gate that refuses to spend credits on an engine that cannot edit.**

## Performance

- **Duration:** 36 min
- **Started:** 2026-10-07T13:33:08Z
- **Completed:** 2026-10-07T14:09:18Z
- **Tasks:** 3
- **Files modified:** 12 (source 8, tests 3, lockfile 1)

## Accomplishments

- **One command proves the whole edit stack.** `imagai edit --image source.png -p "make it sunset"` reaches the provider, comes back, and lands in the same flat `output_dir` the generation path already uses — through clap routing, the shared DTO, `generate_image_core`, the provider transport and the shared save helpers.
- **The capability gate never spends credits on a request the engine cannot serve.** `edit_transport` bails naming both the engine and its model before a socket opens; `cmd_edit` propagates the error to exit 1. The test asserts `mock.requests()` is empty — the observable form of "no credits spent".
- **The wire format is asserted, not assumed.** The mock now captures raw request bytes, so tests read the actual multipart body. `generate_edits` sends `model`, `prompt` and one `image[]` per image and nothing else — zero occurrences of `quality`, `style`, `response_format` or `n`.
- **The supply-chain gate was honoured exactly.** `Cargo.lock` gained 18 lines and precisely two packages; both sha256 checksums match plan 01-00's human-approved values byte-for-byte.
- **EDIT-03 works on both transports.** Source first, references in the user's given order, never sorted or deduplicated — proven by ascending byte offsets on the multipart body and by `messages[0].content` ordering on the chat path.
- **`generate` is untouched, provably.** It still posts JSON to `images/generations`, and `generate_reports_api_errors` still asserts `.success()` — its exit-0 behaviour stays Phase 4's to fix.

## Task Commits

Each task was committed atomically. Task 1 follows the TDD RED → GREEN contract as two commits:

1. **Task 1 RED: add failing test for imagai edit walking skeleton** — `ca3bc5a` (test)
2. **Task 1 GREEN: implement imagai edit end to end** — `acf0a77` (feat)
3. **Task 2: cover the edit capability gate and pre-flight failures** — `9225c14` (test)
4. **Task 3: cover multi-reference edits on both transports** — `e5a5424` (test)

## Files Created/Modified

- `Cargo.toml` — `multipart` added to the already-locked `reqwest` 0.13.1 feature list; no direct dependency added
- `Cargo.lock` — `mime_guess` 2.0.5 and `unicase` 2.10.0 resolved, with the checksums approved at plan 01-00's gate
- `src/models.rs` — `source_image: Option<String>` and `ref_images: Vec<String>` on the shared request DTO
- `src/provider.rs` — `EditTransport`, `edit_transport(name, cfg)`, `generate_edits`, the ordered-image helper, and the chat-vision data-URI block
- `src/core.rs` — dispatch on `source_image`, `<stem>-edit.<ext>` derivation, source-filename threading into the save path
- `src/cli.rs` — `Commands::Edit`, `EditArgs`, `cmd_edit`, plus `read_prompt_from_stdin` and `print_edit_results` extracted from `cmd_generate`
- `src/utils.rs` — `Source` tEXt chunk with Latin-1 mapping, `IMAGE_EXTENSIONS`, `list_output_images`, trailing `source` parameter on the save helpers
- `src/web.rs` — imports `IMAGE_EXTENSIONS` from utils; request literal gained the two new fields
- `src/tui.rs` — request literal gained the two new fields
- `tests/common/mod.rs` — raw `Bytes` capture, `raw_requests_for`, `images/edits` routed to `images_logic`
- `tests/cli.rs` — `cmd_edit`/`seed_source_png` helpers, `png_has_metadata` gains a source argument, and the eight edit tests
- `tests/core.rs` — `edit_via_openrouter_chat_sends_data_uri_parts`

## Verification

All plan-level `<verification>` clauses hold:

| Clause | Result |
|---|---|
| Plan 01-00 recorded `approve` before this plan started | PASS — `Outcome: approve` present |
| `Cargo.lock` carries both crates with the gate's checksums, no other package changed | PASS — +18 lines, exactly 2 added `name` entries, both sha256 match |
| `cargo fmt --check` exits 0 | PASS |
| `cargo clippy --all-targets` exits 0 | PASS — zero diagnostics |
| `cargo test` exits 0, every suite `ok` | PASS — 30 unit / 24 cli / 4 core / 10 web = **68** (baseline 51; plan floors were 28 / 24 / 4) |
| The four named pre-existing `generate` tests still `ok` | PASS |
| `generate` still exits 0 on provider failure; `edit` exits 1 | PASS — `generate_reports_api_errors` still asserts `.success()` |
| Full-stack local run saves `<dir>/source-edit.png` | PASS — confirmed against a live HTTP server, printing `Saved to: <dir>/source-edit.png` |

The full-stack run's saved PNG was chunk-walked to confirm exactly three `tEXt` chunks in order: `Prompt=make it sunset`, `Model=gpt-image-1`, `Source=source.png`.

Live-provider behaviour is deliberately **not** asserted — research assumptions A1 and A2 remain unresolved and an end-of-phase human verification covers them.

## Decisions Made

- **`source_image` stays singular and primary.** A promoted plural `sources: Vec<String>` would make two locked decisions unanswerable: D-06 derives the filename from *the* source stem, and D-07 records a single `Source` value. The pluralization is contained at the identity layer and expressed at the wire boundary, where it is cheap.
- **Gate on a `gpt-image` substring, never on `is_image_model`.** That heuristic is display-only; `dall-e-3` passes it and still cannot edit. Routing on it would burn credits on a 400.
- **Over 16 images fails loudly.** Truncating would hand the model a different request than the user asked for — the message states the total, the limit, and the reference count.
- **References are never reordered or deduplicated.** Order is what the user asked the model to blend; a silent dedupe would hide a user mistake.
- **The exit-code guard lives in `cmd_edit`, not the shared printer.** Putting it in `print_edit_results` would have silently changed `generate`'s exit behaviour, which D-01 and Phase 4's ROB-01 both forbid.

## Deviations from Plan

### Plan Corrections

**1. [Plan accuracy] Task 3's implementation was written in Task 1, not Task 3**
- **Found during:** Task 3
- **Issue:** The plan describes Task 3 as replacing the single-part block in `generate_edits` with a loop and adding a 16-image guard. Task 1's specification already required emitting "one `image[]` part per supplied image", so the loop and the limit were built and committed in Task 1 (`acf0a77`).
- **Fix:** Task 3 delivers what remained — `--ref` wired into the request, and the tests proving ordering, the limit and the chat-vision data-URI path. The behaviour is unchanged from what the plan specifies; only the commit boundary differs.
- **Files modified:** `src/cli.rs`, `tests/cli.rs`, `tests/core.rs`
- **Verification:** All six Task 3 acceptance criteria pass.
- **Committed in:** `e5a5424`

### Auto-fixed Issues

**2. [Rule 1 - Bug] Fixed a wrong-fixture test in `tests/core.rs`**
- **Found during:** Task 3
- **Issue:** `edit_via_openrouter_chat_sends_data_uri_parts` seeded one reference but asserted four content parts (the plan's figure assumes source + two refs), so it failed with `left: 3, right: 4`. The captured body was correct — text plus two `image_url` data URIs — so this was a bug in my test, not the transport.
- **Fix:** Seeded a second reference image so the fixture matches the plan's intent, and corrected the assertion message to "text + source + two refs".
- **Files modified:** `tests/core.rs`
- **Verification:** Test passes; the debug print added during diagnosis was removed.
- **Committed in:** `e5a5424`

**3. [Rule 1 - Bug] Two clippy lints in new code, fixed before commit**
- **Found during:** Task 1
- **Issue:** `edit_image_paths` used `filter_map(|p| p)` (an identity closure) and the new `raw` field tripped `clippy::type_complexity`. Clippy-clean is this repo's documented baseline (`CONVENTIONS.md`).
- **Fix:** Used `.flatten()` and introduced a `RawCaptured` type alias in `tests/common/mod.rs`.
- **Files modified:** `src/provider.rs`, `tests/common/mod.rs`
- **Verification:** `cargo clippy --all-targets` reports zero diagnostics.
- **Committed in:** `acf0a77`

### Process Deviations

**4. [Tooling limitation] The `check tdd-red-evidence` gate cannot validate any Rust RED**
- **Found during:** Task 1 RED
- **Issue:** The classifier parses Node's TAP summary (`# tests N`, `ok N - name`). Cargo emits `test result: FAILED. 0 passed; 1 failed`, which the parser cannot read, so it returns `INVALID_RED / zero_tests_discovered`. I confirmed this is a parser limitation rather than a signal about my work by feeding the classifier a *passing* cargo run: both a green and a red run classify as `INVALID_RED` (`unexpected_green` and `zero_tests_discovered` respectively) with `tests: 0`. The gate therefore cannot return `RED_EVIDENCE_OK` for any Rust project.
- **Fix:** None available without changing shared gsd-core, which is out of scope for this plan. The RED phase was instead verified substantively and the evidence persisted: exit code 101; the *named target test* is the one failing (`0 passed; 1 failed`); the failure is on the planned behaviour (`error: unrecognized subcommand 'edit'`) and not a syntax error, zero-test discovery, or fixture crash. The RED state was re-created from a clean checkout to prove reproducibility before committing.
- **Files modified:** none
- **Verification:** The committed RED commit `ca3bc5a` reproduces the failure; GREEN `acf0a77` makes it pass.
- **Committed in:** n/a (documentation)

**5. [Scope note] Task 2 produced no RED phase**
- **Found during:** Task 2
- **Issue:** Task 2 is marked `tdd="true"`, but Task 1's tracer had already built `edit_transport` and `cmd_edit`'s pre-flight checks. Task 2's ten tests passed on first run, so no genuine RED→GREEN cycle was possible.
- **Fix:** None. Manufacturing a RED by breaking working code would have been theatre. Task 2 ships as the characterization test suite it actually is, and this is recorded here rather than presented as a TDD cycle.
- **Files modified:** `src/provider.rs`, `tests/cli.rs`
- **Verification:** All ten tests pass; they lock in behaviour that previously had no coverage.
- **Committed in:** `9225c14`

## Issues Encountered

- **`Read` tool returned `[object Object]` for the six workflow references.** Verified the files were intact (2416 lines total, valid UTF-8); the tool chokes on their very long lines. Retrieved them via `fold -w 200 -s` through the shell instead, and read all six in full. No content was reconstructed from repository search.
- **A `git stash` used to split the RED/GREEN commits removed the working-tree changes instead of isolating them.** Detected immediately from `git status`, recovered with `git stash pop`, and switched to a backup-copy approach that reverts source files to HEAD while keeping the test changes. Verified byte-for-byte that all 12 changed files were restored before continuing.
- **Research assumption A6 resolved.** The plan resolved the `web.rs`/`get_image_extension` extension-set mismatch by moving `IMAGE_EXTENSIONS` into `utils.rs` and adding `gif`. Consequence, as the plan requires be stated: the web gallery now lists `.gif` files too. This is the intended alignment — a file you can save is a file you can pick.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- **Plan 01-02 is unblocked.** `list_output_images` is defined and tested in `src/utils.rs`; `cmd_edit`'s missing-`--image` branch exits 1 with a message naming `--image`, which is exactly the seam D-03's newest-image auto-pick lands on. `edit_without_image_fails_before_auto_pick_lands` documents the current behaviour and will need updating when that branch lands.
- **Plans 01-03 and 01-04 can proceed.** Both surfaces' request literals already carry `source_image`/`ref_images`, so their handlers have a compiling DTO to work against.
- **One coordination note for 01-03:** the web surface still keeps its own private `read_dir` loop; refactoring it onto `list_output_images` is 01-02's scope per the slice plan.
- **Phase 2's history tree has its lineage hook.** Every edit now records `Source` in the PNG, which is the minimal ancestor link Phase 2 builds on.

## Self-Check: PASSED

- [x] All 12 changed files exist on disk
- [x] All four commits present (`ca3bc5a`, `acf0a77`, `9225c14`, `e5a5424`)
- [x] Every Task 1, 2 and 3 acceptance criterion re-verified individually
- [x] All plan-level `<verification>` clauses re-run and passing
- [x] `cargo fmt --check`, `cargo clippy --all-targets` and `cargo test` all exit 0
- [x] Both approved checksums asserted against the resolved `Cargo.lock`
- [x] No stub, placeholder or TODO introduced
- [x] `STATE.md` and `ROADMAP.md` were NOT modified or staged — the orchestrator owns those writes after all wave agents complete

---
*Phase: 01-image-editing*
*Completed: 2026-10-07*