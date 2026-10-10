---
gsd_state_version: "1.0"
current_phase: 3
current_phase_name: Multi-Vendor Providers
status: planning
stopped_at: Phase 2 complete, ready to plan Phase 3
last_updated: "2026-10-10T18:01:16.591Z"
last_activity: 2026-10-10
last_activity_desc: Phase 2 complete, transitioned to Phase 3
state_head: 411444199fd1d91479aea9a1a2030419276867c6
progress:
  total_phases: 4
  completed_phases: 2
  total_plans: 10
  completed_plans: 10
  percent: 50
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-10-10)

**Core value:** One command → one generated or edited image, from any surface, against any configured provider — with the iteration history preserved so users can branch instead of starting over.
**Current focus:** Phase 3 — Multi-Vendor Providers (ready to plan)

## Current Position

Phase: 3 — Multi-Vendor Providers
Plan: Not started
Status: Ready to plan
Last activity: 2026-10-10 — Phase 2 complete, transitioned to Phase 3

Progress: [█████░░░░░] 50%

## Performance Metrics

**Velocity:**

- Total plans completed: 10
- Average duration: -
- Total execution time: 0 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 01 | 5 | - | - |
| 02 | 5 | ~125m | ~25m/plan |

**Recent Trend:**

- Last 5 plans: -
- Trend: N/A

*Updated after each plan completion*
**Per-Plan Metrics:**

| Plan | Duration | Tasks | Files |
|------|----------|-------|-------|
| Phase 2 P05 | 15min | 2 tasks | 3 files |

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
- [Phase 2]: G-1: a filename claim has two halves (file on disk + sidecar record); history::filename_taken probes both and reads an unreadable sidecar as taken
- [Phase 2]: G-1: the n==1 source_image arm bumps through the shared suffix_numbered _N scheme, bounded by MAX_NAME_BUMPS, keeping the canonical name when free
- [Phase 2]: T-02-01/T-02-05/T-02-10 closure: reduce a record `filename` through `history::node_path` at the CLI's last join before the read — one guard covers both `fork` and `edit --from`, and a separator-carrying record is refused with the operator's own value named
- [Phase 2]: WR-05 accepted residual — a fork from an explicitly out-of-dir file records `parent: None`; consistent with locked decision D-22 (out-of-dir outputs stay untracked), so there is genuinely no parent to record

### Pending Todos

None yet.

### Blockers/Concerns

- `created`/`modified` gallery timestamps both derive from mtime — recorded bug, owned by a future phase (Phase 2 shipped real node timestamps in `.imagai-history.json`; the gallery display is untouched)
- History has no prune/delete/gc surface (D-21 defers it) and a hand-edited sidecar's `../` filename is reduced to its final component before any join (T-02-01)
- [Phase 2 UI review — advisory, non-blocking]: 4 open UI follow-ups recorded at `.planning/phases/02-branching-iteration-history/02-UI-REVIEW.md` (14/24 overall): the TUI `… {n} more` overflow indicator never renders, the web history view lacks the contract's empty-state sentence, the TUI detail pane over-wraps, and the SPA label slices UTF-16 units (surrogate-pair split vs the CLI's clean 32-char field). Follow-up-phase items, retained in Deferred Items below
- ~~Phase 2 G-1: a second fork from the same source overwrites the first forked image (src/core.rs:85 plus the else-base arm at src/core.rs:88-89; n is always 1 so line 87 suffix_numbered never runs). Two forks from one node id wrote one file while the sidecar held two records naming it, and history marked neither missing. Fix route: code-review-fix dispatch.~~ **CLOSED by 02-05** (`5264a55`, `f5cf9c2`): `history::filename_taken` now probes both halves of a filename claim and the `n == 1` source_image arm bumps through `suffix_numbered`; `tests/cli.rs#two_forks_from_one_source_keep_both_images` proves both forks survive with distinct names.

## Deferred Items

| Category | Item | Status | Deferred At | Milestone |
|----------|------|--------|-------------|-----------|
| ui-fidelity | TUI History overflow: the `… {n} more` line is pushed but ratatui drops it — 30 nodes render 14 rows with no overflow cue (02-UI-REVIEW finding 2) | open | 2026-10-10 | v1 |
| ui-copy | Web history view lacks the Copywriting Contract empty-state sentence `No history yet — generate or edit an image first.` (02-UI-REVIEW finding 1) | open | 2026-10-10 | v1 |
| ui-fidelity | TUI History detail pane wraps the whole paragraph; ` [ Fork edit ] ` can be pushed off-pane (02-UI-REVIEW finding 3) | open | 2026-10-10 | v1 |
| ui-parity | SPA `historyNodeLabel` slices by UTF-16 units and splits surrogate pairs where the CLI clips cleanly — BRCH-04 row-grammar parity break (02-UI-REVIEW finding 4) | open | 2026-10-10 | v1 |
| lineage | WR-05: a fork from an explicitly out-of-dir file records `parent: None` — accepted residual consistent with locked decision D-22 (02-REVIEW-FIX.md:177-197) | accepted | 2026-10-10 | v1 |

## Session Continuity

Last session: 2026-10-10T16:20:00Z
Stopped at: Phase 2 complete, ready to plan Phase 3
Resume file: None
