# imagai

## What This Is

A single Rust binary (`imagai`) that generates and edits images — and eventually video — through three interchangeable surfaces: CLI, TUI, and embedded web UI, with every saved result recorded in a forkable iteration history (`imagai history` tree, `imagai fork --from`). It talks to AI image/video providers directly, so users drive generation, editing, and iteration from one tool instead of hopping between provider web UIs. Providers are pluggable per its declared wire format (`IMAGAI__ENGINES__<NAME>__FORMAT`): OpenAI-compatible engines and named non-compatible vendors (v1 ships the `fal` adapter) sit side by side and are selected per request. Built to be handed to other users.

## Core Value

One command → one generated or edited image, from any surface, against any configured provider — with the iteration history preserved so users can branch instead of starting over.

## Requirements

### Validated

<!-- Shipped and confirmed valuable — inferred from the existing codebase (.planning/codebase/). -->

- ✓ Text-to-image generation from the CLI (`imagai generate`) — existing
- ✓ Interactive TUI surface with form + log pane (`imagai tui`, ratatui) — existing
- ✓ Embedded web UI + REST API (`imagai web`, axum + single-file SPA) — existing
- ✓ Multi-engine config with OpenAI-compatible HTTP transport (`src/config.rs`, `src/provider.rs`) — existing
- ✓ Result persistence: filename strategies, PNG metadata injection, URL/b64 save (`src/utils.rs`) — existing
- ✓ Model/engine listing across providers (`imagai list-engines`) — existing
- ✓ Image editing from explicit path, past result, or multi-reference (`imagai edit`) — Phase 1 (EDIT-01)
- ✓ Newest-output auto-pick + shared `list_output_images` helper across CLI/web — Phase 1 (EDIT-02)
- ✓ Multi-reference edits with 16-image guard on all three surfaces — Phase 1 (EDIT-03)
- ✓ **BRCH-01**: View an iteration history for an image and fork/branch any past result without losing the original lineage (`.imagai-history.json` sidecar, `imagai history` tree, TUI History tab, web `/api/history`) — Phase 2
- ✓ **BRCH-02**: Branching and editing work identically on all three surfaces (CLI, TUI, web) (one recorder in `generate_image_core`, one `HistoryView` serializer, one `resolve_fork_source` resolver) — Phase 2
- ✓ **PRVD-01**: User can configure named non-OpenAI-compatible providers (multi-vendor adapters) alongside existing OpenAI-compatible engines, selected per request — Phase 3 (`IMAGAI__ENGINES__<NAME>__FORMAT`, `src/formats.rs` registry, the `fal` adapter; proven: fal body keys `[image_size,num_images,output_format,prompt]` with `Authorization: Key …`, OpenAI body byte-identical, selectable from CLI/TUI/web)

### Active

<!-- Current scope. Hypotheses until shipped and validated. -->

- [ ] **VID-01**: User can generate video from a text prompt through a configured video provider (later phase — image work ships first)
- [ ] **DIST-01**: Other users can install and run the tool from a single binary with clear provider setup docs
- [ ] History-tree UI polish from the advisory UI review (`.planning/phases/02-branching-iteration-history/02-UI-REVIEW.md`, 14/24 — non-blocking follow-ups: TUI overflow indicator never renders, web missing the empty-state sentence, TUI detail-pane over-wrap, SPA UTF-16 label slicing) — surfaced by Phase 2, not yet scheduled

### Out of Scope

- Hosted / multi-user web service — this is a local single-binary tool; the web UI is a local surface, not a deployment target
- Web authentication / LAN exposure hardening — same reason; `imagai web` stays localhost-only by convention until a real deployment need appears (see `.planning/codebase/CONCERNS.md`)
- Fine-tuning / model training — generation and editing only
- Desktop/mobile GUI apps — the three existing surfaces are the product
- Audio generation — not requested; video covers moving media

## Context

- **Brownfield**: this repo already implements the full text-to-image path across three surfaces (~3,417 LOC, 10 modules). Evidence: `.planning/codebase/` (complete map, refreshed 2026-10-03).
- **Architecture**: modular monolith — thin frontends (`src/cli.rs`, `src/tui.rs`, `src/web.rs`) converge on `generate_image_core` (`src/core.rs`); transport in `src/provider.rs` (reqwest + rustls); one binary via clap subcommands.
- **Known gaps that shape this work** (from `.planning/codebase/CONCERNS.md`):
  - ~~No img2img/edit path at all~~ — addressed by Phase 1 (`imagai edit` on all three surfaces via shared core)
  - Generation failures exit 0 (breaks scripting); errors carried as strings, not `Result`
  - No retries/backoff for 429/5xx; no cancellation/progress for long generations
  - Web server has no auth, wide-open CORS, no graceful shutdown
  - Duplication risk: `extra_params` assembled separately per frontend (documented anti-pattern)
- **Video**: OpenAI-compatible video APIs are not an established standard — expect a separate provider integration, hence its own phase after images.
- **Audience**: other users, not just the author — distribution and setup docs are part of done.

## Constraints

- **Tech stack**: Rust (edition 2021), existing deps (clap, axum, ratatui, reqwest/rustls) — reuse the current architecture rather than restructuring
- **Frontend**: no build step for the web UI — single embedded HTML file (`web_interface.html` via `include_str!`) is an established pattern
- **Timeline**: prioritize speed of delivery — "as fast as can build"; scope cuts go to v2/Out of Scope, not into partial implementations
- **Cost surface**: users spend provider credits per call — failures must be visible (see exit-code bug above)

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Branching = single-step edit AND forkable history | User wants to edit a chosen image and keep a branchable lineage of iterations | Validated — Phase 2 shipped history + fork on all three surfaces |
| Video deferred to a later phase; images first | Video needs a separate (non-OpenAI-compatible) provider integration; images path already exists | — Pending |
| Multi-vendor providers, not OpenAI-compatible-only | User will use named non-compatible vendors (incl. video) | Validated — Phase 3 shipped `fal` beside the OpenAI-compatible engines |
| Ship across all three surfaces (CLI, TUI, web) | Existing architecture already guarantees parity via shared core; new features must land on all three | Validated — Phase 1 shipped edit parity, Phase 2 shipped branching parity |
| Audience = other users | Drives distribution (DIST-01) and setup documentation | — Pending |
| D-08: history lives in one `.imagai-history.json` sidecar inside `output_dir` | A local single-user tool gets a local store; no new dependency, no schema migration path | Validated — Phase 2 |
| D-10: pre-existing files are lazily adopted as roots | `(unknown prompt)` adoption beats a store that only knows post-Phase-2 files | Validated — Phase 2 |
| D-11: whole-sidecar read-modify-write per save | D-08's locked cost; write is temp-file + `rename`, so an interrupted write never destroys the store | Validated — Phase 2 |
| D-13: fork references the same file, never copies it | Forks append a record; the named original is never modified (T-02-06) | Validated — Phase 2 |
| D-14: step-back is forward-only (choose an ancestor id as `--from`) | Nothing is rewound or deleted; history is append-only by construction | Validated — Phase 2 |
| D-23: record filenames relative to `output_dir` only | Moving the folder keeps history valid | Validated — Phase 2 |
| D-24/D-28/D-29: the wire format is declared, never inferred — `IMAGAI__ENGINES__<NAME>__FORMAT` over a closed name set (`openai`, `fal`), resolved before every inference | String sniffing is a guess; a declaration is a fact, and the fal branch sits behind `matches!(format, ProviderFormat::Fal)` so no-regression is structural rather than incidental | Validated — Phase 3 (34/34 must-haves; `src/formats.rs`) |
| D-25: exactly one non-OpenAI adapter in v1 (`fal`) | One format proves the seam; a second would be copy-paste before the seam is proven | Validated — Phase 3 |
| D-26: the new format is generation-only in v1; edits against it are refused naming engine+model | A half-working edit that leaks an OpenAI-shaped body is worse than a clear refusal; the remedy names `IMAGAI__ENGINES__<NAME>__MODEL` or OpenRouter | Validated — Phase 3 (refusal proven live: rc=1, 0 requests) |
| D-27: the fal adapter maps onto the shared `ImageGenerationResponse` (`url` or b64 from a data URI) | Save/history/PNG-lineage are reused unchanged; the adapter owns only the wire shape | Validated — Phase 3 |
| D-30: the edit capability gate is format-driven, resolving the declared format before the transport question | The gate answers from the registry, not from engine-name heuristics | Validated — Phase 3 |
| 02-05: a filename claim has two halves (file on disk + sidecar record); `history::filename_taken` probes both and an unreadable sidecar reads as taken | Mirror of CR-06 for the `source_image` arm: a kept-but-missing node (D-20) still owns its name, and the safe direction is to disambiguate, never overwrite | Validated — Phase 2 (`56984f8`'s security round re-ran green) |
| Security-fix guard: reduce a record `filename` through `history::node_path` at the CLI's last join before the read | T-02-01/T-02-05/T-02-10: `--from <id8>` must not resolve an injected `../` record; one guard covers both `fork` and `edit` | Validated — Phase 2 (`56984f8`, 14/14 threats closed) |

## Context Paths

- Codebase map: `.planning/codebase/` (STACK, ARCHITECTURE, STRUCTURE, CONVENTIONS, TESTING, INTEGRATIONS, CONCERNS)
- Research: `.planning/research/` (if created during initialization)

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-10-12 after Phase 3*
