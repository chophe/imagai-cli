# imagai

## What This Is

A single Rust binary (`imagai`) that generates and edits images — and eventually video — through three interchangeable surfaces: CLI, TUI, and embedded web UI. It talks to AI image/video providers directly, so users drive generation, editing, and iteration from one tool instead of hopping between provider web UIs. Built to be handed to other users.

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

### Active

<!-- Current scope. Hypotheses until shipped and validated. -->

- [ ] **EDIT-01**: User can edit an existing image (from a file path or a past result) by combining it with a new prompt, and get the result back in the same flow
- [ ] **BRCH-01**: User can view an iteration history for an image and fork/branch any past result into a new edit without losing the original lineage
- [ ] **BRCH-02**: Branching and editing work identically on all three surfaces (CLI, TUI, web)
- [ ] **PRVD-01**: User can configure named non-OpenAI-compatible providers (multi-vendor adapters) alongside existing OpenAI-compatible engines, selected per request
- [ ] **VID-01**: User can generate video from a text prompt through a configured video provider (later phase — image work ships first)
- [ ] **DIST-01**: Other users can install and run the tool from a single binary with clear provider setup docs

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
  - No img2img/edit path at all — the generation request has no image input
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
| Branching = single-step edit AND forkable history | User wants to edit a chosen image and keep a branchable lineage of iterations | — Pending |
| Video deferred to a later phase; images first | Video needs a separate (non-OpenAI-compatible) provider integration; images path already exists | — Pending |
| Multi-vendor providers, not OpenAI-compatible-only | User will use named non-compatible vendors (incl. video) | — Pending |
| Ship across all three surfaces (CLI, TUI, web) | Existing architecture already guarantees parity via shared core; new features must land on all three | — Pending |
| Audience = other users | Drives distribution (DIST-01) and setup documentation | — Pending |

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
*Last updated: 2026-10-03 after initialization*
