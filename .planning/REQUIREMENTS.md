# Requirements: imagai

**Defined:** 2026-10-04
**Core Value:** One command → one generated or edited image, from any surface, against any configured provider — with iteration history preserved so users can branch instead of starting over.

## v1 Requirements

Requirements for initial release. Each maps to roadmap phases.

### Editing

- [ ] **EDIT-01**: User can edit an existing image from a file path by combining it with a new prompt
- [ ] **EDIT-02**: User can use any past generation as the source for a new edit
- [ ] **EDIT-03**: User can pass multiple reference images to a single edit request

### Branching

- [ ] **BRCH-01**: User can view the iteration history tree of a generation session
- [ ] **BRCH-02**: User can fork any past image into a new edit while keeping the original lineage
- [ ] **BRCH-03**: User can step back to a prior branch point
- [ ] **BRCH-04**: Editing and branching work identically on CLI, TUI, and web

### Providers

- [ ] **PRVD-01**: User can configure named non-OpenAI-compatible provider adapters alongside existing engines, selected per request

*(Already Validated in PROJECT.md — no new work, must not regress: text-to-image via CLI/TUI/web, multi-engine OpenAI-compatible transport, per-request engine selection, result persistence with PNG metadata.)*

### Robustness

- [ ] **ROB-01**: `imagai generate` exits non-zero when generation fails, so scripts and CI can detect errors

### Distribution

- [ ] **DIST-01**: Other users can install and run the tool from a single binary with clear provider setup docs

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Video

- **VID-01**: User can generate video from a text prompt through a configured video provider (separate, non-OpenAI-compatible integration — decided to follow images)

### Hardening

- **ROB-02**: Provider calls retry 429/5xx responses with exponential backoff and honor `Retry-After`
- **ROB-03**: User can cancel an in-flight generation from TUI or web and sees progress during long generations

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Hosted / multi-user web service | Local single-binary tool; web UI is a local surface, not a deployment target |
| Web authentication / LAN hardening | `imagai web` stays localhost-only by convention (see `.planning/codebase/CONCERNS.md`) until a real deployment need appears |
| Fine-tuning / model training | Generation and editing only |
| Desktop / mobile GUI apps | CLI, TUI, and web are the product |
| Audio generation | Never requested; video covers moving media |
| Video in v1 | Decided: images ship first; video needs a separate provider integration |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| EDIT-01 | — | Pending |
| EDIT-02 | — | Pending |
| EDIT-03 | — | Pending |
| BRCH-01 | — | Pending |
| BRCH-02 | — | Pending |
| BRCH-03 | — | Pending |
| BRCH-04 | — | Pending |
| PRVD-01 | — | Pending |
| ROB-01 | — | Pending |
| DIST-01 | — | Pending |

**Coverage:**
- v1 requirements: 10 total
- Mapped to phases: 0
- Unmapped: 10 ⚠️ (roadmap creation fills this)

---
*Requirements defined: 2026-10-04*
*Last updated: 2026-10-04 after initial definition*
