# Roadmap: imagai

**Created:** 2026-10-04
**Phases:** 4
**Granularity:** coarse

Brownfield extension of the existing modular monolith (shared core `src/core.rs`, transport `src/provider.rs`, thin frontends `src/cli.rs` / `src/tui.rs` / `src/web.rs`). Phases build on the shipped text-to-image path — no re-scaffolding.

## Phases

- [ ] **Phase 1: Image Editing** - Edit an existing image (file path or past result, multi-reference) through the shared core on all three surfaces
- [ ] **Phase 2: Branching & Iteration History** - Persist and expose the iteration lineage: history tree, fork, step-back, cross-surface parity
- [ ] **Phase 3: Multi-Vendor Providers** - Named non-OpenAI-compatible provider adapters alongside existing engines, selected per request
- [ ] **Phase 4: Ship to Users** - Non-zero exit on failure plus single-binary install and provider setup docs

## Phase Details

### Phase 1: Image Editing
**Goal:** Users can edit an existing image by combining it with a new prompt and get the result back in the same flow
**Mode:** mvp
**Depends on:** Nothing (first phase — builds directly on the existing text-to-image core)
**Requirements:** EDIT-01, EDIT-02, EDIT-03
**Success Criteria** (what must be TRUE):
  1. User can run an edit against a file path (e.g. `imagai edit --image <path> -p "<prompt>"`) and receives a saved, usable result
  2. User can pick any previously generated image as the edit source without manually re-supplying its path
  3. User can pass multiple reference images to a single edit request and the provider receives all of them
  4. The same edit flow (source + prompt + references) is available on CLI, TUI, and web, producing equivalent results

**Plans**: 5 plans

- [ ] 01-00-PLAN.md — Wave-0 supply-chain gate: human decide-in on vendoring `mime_guess` 2.0.5 and `unicase` 2.10.0 before the `multipart` feature pulls them in
- [ ] 01-01-PLAN.md — Walking skeleton: `imagai edit` from an explicit path (multipart + chat transports, capability gate, `-edit` filename, PNG lineage, multi-reference)
- [ ] 01-02-PLAN.md — EDIT-02: pick the newest `output_dir` image when `--image` is omitted; one shared `list_output_images` helper for CLI and web
- [ ] 01-03-PLAN.md — Web parity: `POST /api/edit` with the output_dir filename guard, plus the browser edit form with source/reference pickers
- [ ] 01-04-PLAN.md — TUI parity: Edit tab with its own row table, source picker overlay, and the first unit tests in `src/tui.rs`

**Wave structure:** 01-00 (w0) → 01-01 (w1) → 01-02 (w2) → 01-03 ∥ 01-04 (w3)
`01-00` is the phase's supply-chain gate. `01-01` declares `depends_on: [01-00]`, which transitively holds `01-02`, `01-03` and `01-04` until the human answers. Its single task is a `checkpoint:decision` with `gate="blocking-human"` — the combination that survives this project's `human_verify_mode: end-of-phase` and still refuses auto-selection in auto mode. `01-01` Task 1 also carries a `<precondition>` asserting the decision was recorded, as a second layer behind the `depends_on` edge. `01-03` and `01-04` share wave 3 because they touch disjoint files.
**UI hint**: yes

### Phase 2: Branching & Iteration History
**Goal:** Users can view an image's iteration lineage and fork any past result into a new edit without losing the original
**Mode:** mvp
**Depends on:** Phase 1 (edits are the nodes of the history tree)
**Requirements:** BRCH-01, BRCH-02, BRCH-03, BRCH-04
**Success Criteria** (what must be TRUE):
  1. User can view the iteration history tree of a generation session and see which image descended from which
  2. User can fork any past image into a new edit; the original lineage remains intact and both branches are visible afterward
  3. User can step back to a prior branch point and continue iterating from it
  4. Editing and branching behave identically on CLI, TUI, and web (same history, same operations)
**Plans**: TBD
**UI hint**: yes

### Phase 3: Multi-Vendor Providers
**Goal:** Users can configure named non-OpenAI-compatible providers alongside existing engines and select them per request
**Mode:** mvp
**Depends on:** Phase 1 (edit requests must route to whichever provider family is configured); runs independently of Phase 2
**Requirements:** PRVD-01
**Success Criteria** (what must be TRUE):
  1. User can configure a named non-OpenAI-compatible provider adapter (own request/response shape) next to existing OpenAI-compatible engines
  2. User selects the provider per request, and the request is built with that provider's wire format — no OpenAI body leaking in
  3. `imagai list-engines` shows the new provider, and existing OpenAI-compatible engines keep working unchanged (no regression)
  4. The new provider is selectable from all three surfaces (CLI flag, TUI, web engine picker)
**Plans**: TBD

### Phase 4: Ship to Users
**Goal:** Other users can install a single binary and trust it in scripts — failures are visible, setup is documented
**Mode:** mvp
**Depends on:** Phases 1–3 (documents the full editing/branching/provider surface)
**Requirements:** ROB-01, DIST-01
**Success Criteria** (what must be TRUE):
  1. `imagai generate` (and edit) exits non-zero with a visible error message when generation fails — bad key, provider error, save failure
  2. Successful runs still exit 0, so existing scripting behavior does not change on the happy path
  3. A fresh user can install the single binary and complete a first generation by following provider setup docs alone (README / `.env.example` cover every engine family including the Phase 3 adapter)
  4. Docs cover running all three surfaces (CLI, TUI, web) with a configured provider
**Plans**: TBD

## Coverage

| Requirement | Phase |
|-------------|-------|
| EDIT-01 | Phase 1 |
| EDIT-02 | Phase 1 |
| EDIT-03 | Phase 1 |
| BRCH-01 | Phase 2 |
| BRCH-02 | Phase 2 |
| BRCH-03 | Phase 2 |
| BRCH-04 | Phase 2 |
| PRVD-01 | Phase 3 |
| ROB-01 | Phase 4 |
| DIST-01 | Phase 4 |

**Mapped: 10/10 ✓ — no orphans, no duplicates**

## Progress Table

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Image Editing | 0/5 | Not started | - |
| 2. Branching & Iteration History | 0/3 (est.) | Not started | - |
| 3. Multi-Vendor Providers | 0/2 (est.) | Not started | - |
| 4. Ship to Users | 0/2 (est.) | Not started | - |
