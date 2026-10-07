---
phase: 01-image-editing
plan: 03
subsystem: api
tags: [edit, axum, web, traversal-guard, gallery, images-edits]

requires:
  - phase: 01-image-editing
    plan: "01-01"
    provides: "`ImageGenerationRequest` with `source_image`/`ref_images`, `edit_transport` capability gate, `generate_image_core` edit dispatch and `Source` PNG lineage"
  - phase: 01-image-editing
    plan: "01-02"
    provides: "`list_output_images` newest-first helper shared by CLI auto-pick and the web gallery"
provides:
  - "`POST /api/edit` taking prompt, optional engine, source and ref filenames plus output name, resolved under `output_dir` and executed through `generate_image_core`"
  - "`output_dir_file` filename guard plus `is_plain_filename` explicit refusal for separator-bearing values (T-01-10, T-01-11)"
  - "Over-16 image count reported with requested and allowed counts, never truncated"
  - "Browser edit form with gallery-fed source/reference pickers rendering `image_data` into the shared preview area"
affects: [01-04, phase-02-branching]

actuals:
  tokens: 7756
  tasks: 3
  commits: 3
plan_head_before: 6f9d7b64bd6f7a5af0b5a5aac3a0a9b401182315

tech-stack:
  added: []
  patterns:
    - "Trust boundary first: traversal-shape refusal and count guard run before engine resolution, filesystem reads and network calls"
    - "One result-JSON builder (`result_items_json`) shared by `/api/generate` and `/api/edit` so the two envelopes cannot drift apart"
    - "Browser sends filenames, never bytes: pickers are populated from `GET /api/images`, keeping payloads small and the upload widget out of the edit path"

key-files:
  created: []
  modified:
    - "src/web.rs"
    - "tests/web.rs"
    - "web_interface.html"

key-decisions:
  - "Separator-bearing values are refused with `'<raw>' is not a plain filename inside output_dir` rather than silently reduced — the user asked for something that does not exist, and saying so beats guessing"
  - "The edit handler reports failures as HTTP 200 with `success: false`, matching every other handler in the file; the existing `generate_endpoint_unknown_engine` test pins that shape"
  - "No `extra_params` on the edit path: Stability and OpenRouter generation knobs have no meaning on an edit request, so `..Default::default()` supplies the same crate defaults `cmd_edit` uses"
  - "The over-limit guard counts 1 (source or auto-pick) + refs and runs before any read, so reference filenames need not exist for the rejection to fire"

patterns-established:
  - "Filename guard shape: `output_dir_file` (reduce via `Path::file_name` + join) paired with `is_plain_filename` (refuse when the raw value is not already its own final component)"
  - "Edit-form JS builds the body from explicit field values only, with `refs` from `selectedOptions` so multi-select transmits order"

requirements-completed: [EDIT-01, EDIT-02, EDIT-03]

coverage:
  - id: D1
    description: "`POST /api/edit` with prompt + source filename returns a saved, previewable edited image via the shared core (`source-edit.png`, `data:image/png;base64,` preview, `Source` lineage, one `images/edits` request, zero `images/generations`)"
    requirement: EDIT-01
    verification:
      - kind: e2e
        ref: "tests/web.rs#edit_endpoint_saves_edited_image"
        status: pass
    human_judgment: false
  - id: D2
    description: "Traversal-shaped `source`, `refs` and `output` values are refused with no provider request issued"
    requirement: EDIT-01
    verification:
      - kind: e2e
        ref: "tests/web.rs#edit_endpoint_rejects_traversal_source"
        status: pass
      - kind: e2e
        ref: "tests/web.rs#edit_endpoint_rejects_traversal_reference"
        status: pass
      - kind: e2e
        ref: "tests/web.rs#edit_endpoint_rejects_traversal_output"
        status: pass
    human_judgment: false
  - id: D3
    description: "Missing source names the file; source-plus-16-refs reports 17-requested vs 16-allowed; `dall-e-3` engine fails naming the model with no request sent"
    requirement: EDIT-03
    verification:
      - kind: e2e
        ref: "tests/web.rs#edit_endpoint_reports_missing_source"
        status: pass
      - kind: e2e
        ref: "tests/web.rs#edit_endpoint_rejects_more_than_sixteen_images"
        status: pass
      - kind: e2e
        ref: "tests/web.rs#edit_endpoint_reports_incapable_engine"
        status: pass
    human_judgment: false
  - id: D4
    description: "`GET /` serves the edit form with source and reference pickers wired to `/api/edit`"
    requirement: EDIT-02
    verification:
      - kind: e2e
        ref: "tests/web.rs#edit_form_is_served_with_source_and_ref_pickers"
        status: pass
      - kind: other
        ref: "grep -cF fetch('/api/edit' and fetch('/api/images' in web_interface.html (1 each); all five edit_* ids present"
        status: pass
    human_judgment: false
  - id: D5
    description: "End-to-end browser flow: generate one image, pick it as edit source, submit a change, see the rendered edit"
    requirement: EDIT-02
    verification: []
    human_judgment: true
    rationale: "No browser tooling is configured in this repo; rendering and picker interaction cannot be automated here. Degrades to pickers-populate plus engine-error-names-engine when no edit-capable engine is configured."

duration: 25 min
started: 2026-10-07T15:08:00Z
completed: 2026-10-07T15:33:08Z
status: complete
---

# Phase 1 Plan 3: Web Edit Surface Summary

**`POST /api/edit` with an output_dir filename guard plus a gallery-fed browser edit form — the same edit core the CLI uses, now behind this phase's only real trust boundary.**

## Performance

- **Duration:** 25 min
- **Started:** 2026-10-07T15:08:00Z
- **Completed:** 2026-10-07T15:33:08Z
- **Tasks:** 3
- **Files modified:** 3 (source 1, tests 1, html 1)

## Accomplishments

- **The edit flow is on its second surface.** `POST /api/edit` resolves prompt + source/refs/output filenames under `output_dir`, builds the same `ImageGenerationRequest` the CLI builds, and calls the same `generate_image_core` — edited results land in the flat `output_dir`, so the gallery keeps working (D-06).
- **The trust boundary is explicit, not silent.** Separator-bearing values are refused with a message naming the raw input; over-16 image counts are reported with both counts; an incapable engine fails naming itself with zero requests sent. All three traversal tests prove the guard by `mock.requests()` being empty.
- **The browser form reuses the existing layout.** Source and reference pickers populate from `GET /api/images` (newest-first, no client-side re-sort), the refs array comes from `selectedOptions` in selection order, and results render `image_data` into the shared preview area with the same styling as generation.
- **Full suite green at plan close:** 41 lib / 26 cli / 4 core / 19 web, `cargo clippy --all-targets` exit 0 with zero diagnostics, `cargo fmt --check` exit 0.

## Task Commits

Each task was committed atomically:

1. **Task 1: `POST /api/edit` — one image, one prompt, one saved file** — `eda557e` (feat)
2. **Task 2: The web trust boundary — traversal, reference limit, capability gate** — `103e85d` (feat)
3. **Task 3: Browser edit form — source picker, reference picker, rendered result** — `fc73c5f` (feat)

## Files Created/Modified

- `src/web.rs` — `EditPayload`, `POST /api/edit` route + `edit` handler, `output_dir_file` guard, `is_plain_filename` refusal, `result_items_json` extracted from `generate` and shared
- `tests/web.rs` — `seed_source_png` + `edit_capable_settings` helpers; 8 new tests: `edit_endpoint_saves_edited_image`, three traversal refusals, missing source, incapable engine, over-16 refs, served-form pickers
- `web_interface.html` — edit form (`edit_prompt`/`edit_source`/`edit_refs`/`edit_engine`/`edit_output`), `loadOutputImages()`, `executeEdit()`, edit submit listener, engine-list mirroring into `edit_engine`

## Verification

All plan-level `<verification>` clauses hold:

| Clause | Result |
|---|---|
| `cargo fmt --check` exits 0 | PASS |
| `cargo clippy --all-targets` exits 0 | PASS — zero diagnostics |
| `cargo test` exits 0, every suite `ok` | PASS — 41 lib / 26 cli / 4 core / 19 web |
| Pre-existing `generate`/`generate-cli`/gallery web tests still `ok` | PASS — all six named tests `ok` in the 19 |
| `cargo test --test web` reports at least 18 tests `ok` | PASS — 19 (11 pre-existing + 8 new) |
| Manual browser-flow confirmation | DEGRADED — no browser tooling configured; recorded as coverage D5 (`human_judgment: true`) for the verifier |

## Decisions Made

- **Refuse, don't reduce.** Silent reduction (`../../.env` → `.env`) is safe but confusing; the handler says the value is not a plain filename inside `output_dir` instead.
- **Guards run before everything.** Traversal refusal and the count check precede engine resolution, filesystem reads and network calls — the count test's reference filenames deliberately do not exist, proving the order.
- **Envelopes stay HTTP 200.** Like every other handler in the file, failures report `success: false` in the body; no new status codes.
- **`loadEngines` also feeds `edit_engine`.** The plan specified the select but not its population; mirroring the same engine list keeps the picker functional while an empty value still falls back to the default engine.

## Deviations from Plan

### Auto-fixed Issues

**1. [Test strengthening] Traversal tests pin the refusal message**
- **Found during:** Task 2
- **Issue:** The plan's assertions (`success: false` + empty requests) pass under silent reduction too, so they cannot distinguish the required explicit refusal.
- **Fix:** Each traversal test additionally asserts the error contains `plain filename`. The assertions were RED before the `is_plain_filename` guard landed and GREEN after.
- **Files modified:** `tests/web.rs`
- **Committed in:** `103e85d`

**2. [Minor augmentation] `executeEdit` prints per-result `❌ <res.error>` and `loadEngines` mirrors into `edit_engine`**
- **Found during:** Task 3
- **Issue:** The plan names the `edit_engine` select but specifies no population; an unpopulated select renders empty. Per-result error display follows the generate path's shape for the new results array.
- **Fix:** Engine list mirrored into `edit_engine` (default selected when configured); `executeEdit` renders `res.image_data` on success and `❌ res.error` per failed result, `❌ result.error` on top-level failure.
- **Files modified:** `web_interface.html`
- **Committed in:** `fc73c5f`

**3. [Provenance] `edit_endpoint_saves_edited_image` asserts the `Source` lineage chunk**
- **Found during:** Task 1
- **Issue:** The plan's behavior lists the `Source` chunk but its assert list does not check it.
- **Fix:** Added a byte-scan assert that the saved PNG contains `source.png` (D-07 through the web path).
- **Files modified:** `tests/web.rs`
- **Committed in:** `eda557e`

### Process Deviations

**4. [Concurrency] Mid-execution, sibling plan 01-04's uncommitted `tui.rs` WIP broke the shared targets**
- **Found during:** Task 1 verification
- **Issue:** A parallel executor was mid-RED on the TUI surface in the same checkout: `src/tui.rs` carried 359 uncommitted insertions whose test code failed `cargo test` (lib-test target) and `cargo fmt --check`.
- **Fix:** Left `src/tui.rs` entirely alone — staged only my files per commit, ran `cargo fmt` scoped to `src/web.rs tests/web.rs`, and verified my suites (`web`/`cli`/`core`) plus a `web.rs`-filtered clippy in isolation. After the sibling landed, re-ran the full plan-level verification: all green.
- **Committed in:** n/a (no files touched)

**5. [Precedent] Commits landed on `main`, not a phase/agent branch**
- **Found during:** Task 1 commit
- **Issue:** Same situation 01-02 recorded: prior plan commits are on `main`, no other branches exist in this clone, and the dispatch required per-task atomic commits with no branch instruction.
- **Fix:** Followed the established in-repo precedent; one linear history.
- **Committed in:** `eda557e`, `103e85d`, `fc73c5f`

---

**Total deviations:** 3 auto-fixed (test strengthening, minor augmentation, provenance) + 2 process notes (concurrency, branch precedent)
**Impact on plan:** No scope creep. All behaviour matches the plan's acceptance criteria; additions only pin specified behaviour more tightly.

## Issues Encountered

- **Two self-inflicted edit-tool misfires during Task 2** (a clobbered doc-comment line and a dropped brace, each caught by an immediate read and repaired before compiling). No semantic effect; both repaired in the working tree before the Task 2 commit.
- **`Read` returned `[object Object]` for the summary template** (long-line tool quirk, also seen in 01-01); retrieved the template via `cat` through the shell instead.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- **Plans 01-01 through 01-04 are all complete on `main`.** CLI, web and TUI surfaces each drive the shared edit core; Phase 2's history tree inherits the `Source` lineage hook on every surface.
- **One item for the verifier:** coverage D5 (live browser flow) needs a human with an edit-capable engine, per the plan's own manual-confirmation clause.
- **Threat-model handoff:** T-01-10 through T-01-14 are implemented as specified; `/gsd-secure-phase` owns the retroactive audit. No new network, auth, file-access or schema surface beyond the planned `POST /api/edit` was introduced, so no `Threat Flags` section is warranted.
- **No stubs introduced.** The only `placeholder`-matching lines are HTML input `placeholder=` attributes (legitimate UX hints).

## Self-Check: PASSED

- [x] All 3 changed files exist on disk (`src/web.rs`, `tests/web.rs`, `web_interface.html`)
- [x] All three task commits present (`eda557e`, `103e85d`, `fc73c5f`)
- [x] Every task acceptance criterion re-verified (route + payload + guard + shared serializer; six trust-boundary tests; five edit ids + both fetch calls + `selectedOptions` + zero `uploadedImageData` in added lines)
- [x] All plan-level `<verification>` clauses re-run and passing (table above)
- [x] `STATE.md` and `ROADMAP.md` were NOT modified or staged — the orchestrator owns those writes

---
*Phase: 01-image-editing*
*Completed: 2026-10-07*
