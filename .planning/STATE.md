---
gsd_state_version: "1.0"
current_phase: 2
current_phase_name: Branching & Iteration History
status: phase-complete
stopped_at: Phase 02 complete (4/4 plans), ready to plan Phase 3
last_updated: "2026-10-10T04:26:59.319Z"
last_activity: 2026-10-10
last_activity_desc: Phase 02 complete, transitioned to Phase 3
state_head: 7f0d8204c5f2e675dc3ce1b2b4950e516fe3d327
progress:
  total_phases: 4
  completed_phases: 2
  total_plans: 9
  completed_plans: 9
  percent: 50
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-10-09)

**Core value:** One command → one generated or edited image, from any surface, against any configured provider — with the iteration history preserved so users can branch instead of starting over.
**Current focus:** Phase 2 — Branching & Iteration History (complete; Phase 3 next)

## Current Position

Phase: 2 — Branching & Iteration History
Plan: 4/4 complete (02-01, 02-02, 02-03, 02-04)
Status: Complete
Last activity: 2026-10-10 — Phase 02 complete, transitioned to Phase 3

Progress: [█████░░░░░] 50%

## Performance Metrics

**Velocity:**

- Total plans completed: 9
- Average duration: -
- Total execution time: 0 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 01 | 5 | - | - |
| 02 | 4 | ~110m | ~27m/plan |

**Recent Trend:**

- Last 5 plans: -
- Trend: N/A

*Updated after each plan completion*

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- [Phase 2]: D-08 sidecar history store — one `.imagai-history.json` inside `output_dir`, read-modify-write per save, no locking/journal/compaction
- [Phase 2]: D-09/D-10 lineage derivation — a plain generation is a root, an edit is the child of the node matching its source filename, pre-existing files are lazily adopted
- [Phase 2]: D-12/D-13 fork — any past image is forkable by node id, recorded filename or path; a fork references the same file and keeps the `-edit` naming scheme
- [Phase 2]: D-17/D-19 shared grammar — one `history::history_row_text` renders the CLI tree, the TUI rows and the SPA option labels; one `HistoryView` serializer feeds CLI `--json` and `/api/history`
- [Phase 2]: D-20/D-23 truthfulness — a node whose file vanished stays in the tree marked `missing`, and only filenames relative to `output_dir` are recorded
- [Roadmap]: 4 coarse phases — Editing → Branching → Providers → Ship; video (VID-01) stays v2
- [Roadmap]: ROB-01 + DIST-01 grouped as "Ship to Users" (release readiness)
- [Phase 1]: D-03 auto-pick — omitted `--image` uses newest output image, printed before request; explicit path always wins
- [Phase 1]: D-05 capability gate — `edit_transport` decides wire shape before any socket opens, bails naming engine+model
- [Phase 1]: D-07 PNG lineage — Prompt/Model/Source tEXt chunks, Source holds filename only
- [Phase 1]: web stays localhost-only by convention; symlink-in-output-dir and TLS-trust accepts logged in 01-SECURITY.md

### Pending Todos

None yet.

### Blockers/Concerns

- `created`/`modified` gallery timestamps both derive from mtime — recorded bug, owned by a future phase (Phase 2 shipped real node timestamps in `.imagai-history.json`; the gallery display is untouched)
- History has no prune/delete/gc surface (D-21 defers it) and a hand-edited sidecar's `../` filename is reduced to its final component before any join (T-02-01)
- Phase 2 G-1: a second fork from the same source overwrites the first forked image (src/core.rs:85 plus the else-base arm at src/core.rs:88-89; n is always 1 so line 87 suffix_numbered never runs). Two forks from one node id wrote one file while the sidecar held two records naming it, and history marked neither missing. Fix route: code-review-fix dispatch.

## Deferred Items

| Category | Item | Status | Deferred At | Milestone |
|----------|------|--------|-------------|-----------|
| *(none)* | | | | |

## Session Continuity

Last session: 2026-10-10
Stopped at: Phase 02 complete (4/4 plans), ready to plan Phase 3
Resume file: None
