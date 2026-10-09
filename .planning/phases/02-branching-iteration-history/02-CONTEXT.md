# Phase 2: Branching & Iteration History - Context

**Gathered:** 2026-10-09
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 2 delivers the iteration lineage on top of the shipped edit path (Phase 1): a persistent history tree rooted in `output_dir`, plus fork and step-back operations that let a user pick any past image and branch a new edit from it without losing the original lineage. All four success criteria (BRCH-01 view history tree, BRCH-02 fork without losing lineage, BRCH-03 step back, BRCH-04 identical behavior on CLI/TUI/web) are in scope, across all three surfaces via the shared core.

Out of boundary: new provider wire formats (Phase 3), exit-code fixes and distribution docs (Phase 4), prune/delete/garbage collection of history nodes (deferred — no delete surface this phase), video (v2), retries/cancellation (v2).

</domain>

<decisions>
## Implementation Decisions

### History store & data model
- **D-08:** A single JSON sidecar `.imagai-history.json` lives **inside `output_dir`** — human-readable, no new dependency, survives restarts. Rejected: JSONL journal, PNG `tEXt`-only lineage (O(n) traversal).
- **D-09:** A plain generation (no `source_image`) is recorded as a tree **root node**; an edit whose source is a recorded node becomes its child. Sessions are derived from parent links, not stored separately.
- **D-10:** **Lazy adoption** of pre-existing `output_dir` images: on first history read, existing files become nodes; parent is recovered from the `Source` tEXt chunk when present (D-07 metadata), otherwise the file becomes a root. No migration/rebuild command.
- **D-11:** Read-modify-write of the whole sidecar per save. This is a single-user local tool; gallery-scale rewrite cost is accepted. No locking, no journal, no compaction.

### Fork & step-back semantics
- **D-12:** Any past image is forkable — by filename/path or by history node id. A file not yet in history is **auto-adopted as a root node** at fork time, then forked (never an error).
- **D-13:** Fork **references the same file** — it starts a new edit whose source is the chosen image. No physical copy of the image. Output naming follows the existing edit scheme (`<stem>-edit.<ext>`, `suffix_numbered` for `n > 1`), keeping `output_dir` flat.
- **D-14:** "Step back" is **forward-only**: choosing an ancestor node as the source of a new edit. Nothing is deleted, no destructive rewind. The original branch remains intact.
- **D-15:** After a fork, **both branches are visible** in the history tree (success criterion 2). Forking never detaches a subtree from the session.

### Surface shapes
- **D-16:** CLI gains `imagai history` (tree view; `--json` for scripts) and `imagai fork --from <image|node-id> -p "<prompt>"` (multi-reference `--ref` inherited from `edit`; `--ref` cap of 16 unchanged). `edit` gains an equivalent way to name a past image. Success-criterion phrasing ("view", "fork", "step back") maps to these two commands plus `edit`.
- **D-17:** CLI tree rendering is an **indented ASCII tree** — short node id (8-hex), prompt snippet, filename, timestamp. `--json` emits the same structure machine-readable.
- **D-18:** TUI gains a **History tab** (tree pane, selected-node detail, fork action on the selected node) and the Edit tab's source picker gains a "pick from history" entry — mirroring Phase 1's `open_source_picker`. Draw code follows the existing `draw_*` conventions; failures report through `App::push_log`, never `unwrap`.
- **D-19:** Web API gains `GET /api/history` (tree JSON, newest-first default) and `POST /api/fork` (`from` = filename inside `output_dir`), rendered as a history view in the single-file SPA reusing the existing picker markup. `is_plain_filename` trust-boundary validation applies to every path-shaped field, exactly as in `POST /api/edit`.

### Robustness & scope
- **D-20:** A history node referencing an externally deleted file is **kept and marked `missing`**, with a warning on listing. History stays truthful; nodes are never silently dropped.
- **D-21:** **No prune/delete/garbage-collection surface in this phase.** Deferral: history pruning/gc. `output_dir` remains the single source of truth for images; the sidecar only records lineage.
- **D-22:** Only results written into the configured `output_dir` by generate/edit/fork are recorded. Results written to an explicit out-of-dir path stay untracked.
- **D-23:** The store records **filenames relative to `output_dir` only** — never absolute paths — so moving the folder keeps history valid.

### the agent's Discretion
User accepted all recommended answers wholesale. Remaining latitude: internal module decomposition (how the store is read/written, whether a new `src/history.rs` module or `src/utils.rs` helpers), node id scheme details, tree-drawing specifics, test layout, and error-message wording — provided the locked decisions above and the codebase conventions hold.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase definition
- `.planning/ROADMAP.md` § Phase 2 — goal, success criteria, `**Mode:** mvp`
- `.planning/REQUIREMENTS.md` — BRCH-01, BRCH-02, BRCH-03, BRCH-04 (v1)
- `.planning/PROJECT.md` — Core Value (branching is named in it), Constraints, Key Decisions row "Branching = single-step edit AND forkable history"

### Codebase evidence (existing system to extend)
- `.planning/phases/01-image-editing/01-CONTEXT.md` — D-01…D-07 (edit command shape, D-03 auto-pick replaced by history selection, D-06 filename scheme, D-07 PNG lineage)
- `.planning/codebase/ARCHITECTURE.md` — shared pipeline `generate_image_core`, three-frontend shape
- `.planning/codebase/STRUCTURE.md` § Where to Add New Code — CLI subcommand, web field, TUI row/tab insertion points
- `.planning/codebase/CONVENTIONS.md` — naming (`cmd_*`, `ROW_*`, `draw_*`), `anyhow` errors, `#[serde(default)]` on API fields, test layout, `cargo fmt`/`cargo clippy --all-targets` clean baseline
- `.planning/codebase/TESTING.md` — mock server modes, test fixtures
- `.planning/codebase/CONCERNS.md` § Known Bugs — exit-0-on-failure, `truncate()` panics, gallery timestamps from mtime (recorded bug owned by a future phase)

No external specs — requirements fully captured in decisions above.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `generate_image_core` (`src/core.rs:12`) — the shared pipeline every surface converges on; fork/edit must reuse it, not fork a parallel path
- `inject_png_metadata` (`src/utils.rs`) — writes Prompt/Model/Source `tEXt` chunks; `Source` carries the parent filename that lazy adoption (D-10) recovers
- `list_output_images` (`src/utils.rs`) — newest-first `output_dir` listing, shared by CLI and web; the base for history listing
- `suffix_numbered` (`src/core.rs`) — naming pattern for fork/edit outputs
- `save_image_from_url` / `save_image_from_b64` (`src/utils.rs`) — result persistence, reused as-is
- Mock server `ImagesMode`/`ChatMode` (`tests/common/mod.rs`) — extended with history-aware test modes; `edit_test_settings`, `seed_png` TUI test helpers already exist

### Established Patterns
- Frontend builds a request DTO → core orchestrates → provider does HTTP; branching must follow the same layering (all three surfaces)
- `output_dir` is flat and shared by the web gallery; new files must not break `list_images`
- Capability gate before any socket opens; fail fast with `anyhow::bail!` naming engine+model (D-05)
- New clap subcommand = new `*Args` struct in `src/cli.rs` + `Commands` enum variant; web = payload struct with `#[serde(default)]` + `pub fn router` (`src/web.rs`); TUI = `TABS` + `draw_*` + `ROW_*` constants
- Errors: per-item `Option<String>` in batch responses; never panic in production paths (`unwrap` only in `#[cfg(test)]`)

### Integration Points
- CLI: `Commands` enum + `cmd_*` handlers in `src/cli.rs` (new `History`, `Fork` variants)
- Web: `router` (`src/web.rs`) + `EditPayload`-style DTOs; SPA is a single embedded HTML file (`include_str!`, no build step)
- TUI: `TABS`, `draw_*`, `ROW_*`, source-picker overlay in `src/tui.rs`
- Core/model touchpoints: `ImageGenerationRequest` (`src/models.rs`) already carries `source_image` / `ref_images`; no wire-format change needed for fork — only lineage recording
- New persistence surface: sidecar `.imagai-history.json` in `output_dir` — first persistent store in the project (STATE.md flags "no persistence layer exists")

</code_context>

<specifics>
## Specific Ideas

No specific requirements — user accepted all recommended grey-area answers; the decisions above are the approved design.

</specifics>

<deferred>
## Deferred Ideas

- History prune/delete/garbage-collection surface — future phase (D-21)
- Gallery timestamps derive from mtime (`created`/`modified` identical) — recorded bug, owned by a future phase; history nodes get real timestamps, gallery display untouched
- Named non-OpenAI-compatible provider adapters — Phase 3
- Retry/backoff and cancellation/progress — v2 (ROB-02, ROB-03)
- Video generation — v2 (VID-01)

</deferred>

---

*Phase: 2-Branching & Iteration History*
*Context gathered: 2026-10-09*
