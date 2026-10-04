# Phase 1: Image Editing - Context

**Gathered:** 2026-10-04
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 1 delivers image editing: combining an existing image with a new prompt and getting a saved result back — from a file path (EDIT-01), from a past generation without re-supplying its path (EDIT-02), and with multiple reference images in one request (EDIT-03). The same edit flow must be available and equivalent on CLI, TUI, and web (BRCH-04's surface-parity half for editing).

Out of boundary: iteration history trees, fork/step-back lineage (Phase 2); new provider wire formats (Phase 3 — Phase 1 works against the engines already configured); exit-code fixes and distribution docs (Phase 4).

</domain>

<decisions>
## Implementation Decisions

### Edit command shape
- **D-01:** A new `imagai edit` subcommand (own clap `EditArgs`, parallel to `GenerateArgs`) — not `generate --image`. `generate` remains untouched. — **Reversibility:** reversible — clap subcommands are cheap to add/remove; no published contract yet
- **D-02:** The edit prompt is a **change instruction** ("make it sunset"), not a full re-prompt — the source image carries the visual context. Downstream agents should not design prompts that restate the whole scene.

### Picking a past image (EDIT-02)
- **D-03:** `--image <path>` always accepts an explicit path; when omitted, the tool uses the **most recent image in `output_dir`** (by mtime) and prints which file it picked. TUI/web expose a small picker listing `output_dir` images, newest first. No history index in this phase — that is Phase 2's scope. — **Reversibility:** reversible — mtime heuristic is local to this phase and replaced by the Phase 2 history tree

### Multi-reference handling and unsupported engines (EDIT-03)
- **D-04:** Multiple references are supplied as a repeatable `--ref <path>` flag (clap `append`), a JSON array on the web API (`GeneratePayload`-style field with `#[serde(default)]`), and a ref list in the TUI.
- **D-05:** If the configured engine cannot accept image input, **fail fast** with a clear error naming the engine (`anyhow::bail!` convention) — never silently fall back to plain text-to-image generation. Spending credits on the wrong request is worse than an error.

### Where results land
- **D-06:** Edited results go into the **same flat `output_dir`** as generations (keeps the existing web gallery and `list_images` working unchanged). Filename = source stem + `-edit` suffix following the `suffix_numbered` pattern. — **Reversibility:** costly — filename scheme is user-visible in saved files and docs; changing it later orphans existing files
- **D-07:** PNG metadata (via `inject_png_metadata`) records the edit prompt **and the source filename** — minimal lineage until Phase 2's full tree.

### the agent's Discretion
User delegated all gray-area choices ("answer by yourself") and approved the resulting decision set wholesale. The researcher/planner have full latitude on: provider wire-format details for the edits endpoint, request DTO field naming, internal function decomposition, test strategy, and how the TUI picker is drawn.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase definition
- `.planning/ROADMAP.md` § Phase 1 — goal, success criteria, `**Mode:** mvp`
- `.planning/REQUIREMENTS.md` — EDIT-01, EDIT-02, EDIT-03 (v1); BRCH-04 surface-parity clause applies to editing
- `.planning/PROJECT.md` — Core Value, Constraints (no build step, Rust stack, speed), Key Decisions

### Codebase evidence (existing system to extend)
- `.planning/codebase/ARCHITECTURE.md` — shared pipeline `generate_image_core`, three-frontend shape, documented anti-patterns (`extra_params` duplication across frontends — do not repeat it for edit fields)
- `.planning/codebase/STRUCTURE.md` § Where to Add New Code — CLI flag, web field, TUI row, provider body, request DTO insertion points
- `.planning/codebase/CONVENTIONS.md` — error handling (`anyhow`), naming (`cmd_*`, `ROW_*`), `#[serde(default)]` on optional API fields, test layout
- `.planning/codebase/CONCERNS.md` § Known Bugs — exit-0-on-failure and `truncate()` panics live on the error path the edit flow will share; do not encode them as expected behavior in new tests

No external specs — requirements fully captured in decisions above.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `generate_image_core` (`src/core.rs:12`) — the shared pipeline; edit flow should converge here rather than fork a parallel path
- `save_image_from_url` / `save_image_from_b64` (`src/utils.rs`) — result persistence reused as-is
- `inject_png_metadata` (`src/utils.rs:234`) — extended for edit prompt + source filename (D-07)
- `suffix_numbered` (`src/core.rs:122`) — naming pattern base for the `-edit` suffix (D-06)
- Mock server `ImagesMode`/`ChatMode` (`tests/common/mod.rs`) — needs a new edit-capable mode for tests

### Established Patterns
- Frontends build a request DTO, core orchestrates, provider does HTTP — edit must follow the same layering
- Config comes free via `IMAGAI__ENGINES__<NAME>__*` env vars — no config-code changes needed for engines
- Fail-fast errors via `anyhow::bail!` with status + truncated body (supports D-05)

### Integration Points
- CLI: `Commands` enum + `src/cli.rs` handler (new `Edit` variant)
- Web: router at `src/web.rs:88` + a payload struct with `#[serde(default)]`
- TUI: `TABS`/`draw_*` functions and `ROW_*` constants (`src/tui.rs`)
- Models: `ImageGenerationRequest` (`src/models.rs:8`) gains image-input fields — currently has none (flagged in STATE.md as a known gap)

</code_context>

<specifics>
## Specific Ideas

No specific requirements — user delegated design specifics; decisions above reflect codebase-grounded defaults approved verbatim.

</specifics>

<deferred>
## Deferred Ideas

- Iteration history tree / lineage persistence — Phase 2 (BRCH-01..03)
- Named non-OpenAI-compatible provider adapters — Phase 3 (PRVD-01)
- Retry/backoff and cancellation/progress — v2 (ROB-02, ROB-03)
- Video generation — v2 (VID-01)

</deferred>

---

*Phase: 1-Image Editing*
*Context gathered: 2026-10-04*
