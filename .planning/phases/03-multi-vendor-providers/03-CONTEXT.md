# Phase 3: Multi-Vendor Providers - Context

**Gathered:** 2026-10-11
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 3 delivers PRVD-01: a user can configure a **named, non-OpenAI-compatible provider adapter** alongside the existing OpenAI-compatible engines and select it per request. The provider has its own request and response wire shape; selecting it builds that provider's format with no OpenAI body leaking in.

In scope: a declared format per engine, one named non-OpenAI adapter shipped in v1, the shared pipeline routing to it, the format shown in `imagai list-engines`, selection on all three surfaces, and the env var documented in `.env.example`.

Out of boundary: full edit support for the new format (v1 = generation only, fail-fast otherwise — D-05's convention), a second non-OpenAI adapter, a third format family, video (VID-01, v2), and the full distribution/setup documentation (Phase 4's DIST-01).

</domain>

<decisions>
## Implementation Decisions

### Adapter seam & configuration
- **D-24:** A new env field `IMAGAI__ENGINES__<NAME>__FORMAT` declares the wire format, taking a **closed set of format names**. Config-only: no new dependency, no Cargo/dependency change, no code compiled per user. Rejected: inferring from `base_url` (today's unreliable sniffing); a Rust trait registered in code only (a code change per provider).
- **D-25:** Exactly **one** named non-OpenAI adapter ships in v1. Its job is to prove the seam end-to-end through the shared pipeline, not to cover a vendor list. Rejected: two adapters (double the surface for one requirement, PRVD-01); a user-filled JSON template (nothing runtime-tested).
- **D-26:** The new format is **generation-only in v1**. An edit or fork against it fails fast naming the engine, per D-05's fail-fast convention — never a fake OpenAI-shaped edit. Reference behavior stays: `--ref` multi-reference edits continue to work on OpenAI-compatible engines unchanged.
- **D-27:** The adapter maps its response onto the existing `ImageGenerationResponse` (`url` or `b64`), so `save_image_from_url`/`save_image_from_b64`, `record_result`, PNG metadata and the web gallery all keep working with **no change**. Rejected: a separate response path duplicating save and record.

### Sniffing, precedence, regression risk
- **D-28:** The existing string sniffing (`model.contains("dall-e-3")`, `contains("stability")`, `base_url` `openrouter.ai` + `gemini`) is **kept as the default when `FORMAT` is unset**. Existing engines must keep working unchanged (ROADMAP success criterion 3, "no regression"). Rejected: making FORMAT required for all engines (breaking); deleting sniffing (breaking).
- **D-29:** When `FORMAT` and the sniffed inference disagree, the **explicit `FORMAT` wins**; sniffing applies only with no declared format. Rejected: sniffing wins (defeats the purpose of declaring).
- **D-30:** `edit_transport`'s capability gate becomes **format-driven**: the declared format states whether the engine accepts image input, replacing name/model string matching as the source of truth. The capability gate must still fail before any socket opens, naming engine+model (D-05).
- **D-31:** `extra_params` remains the escape hatch for provider-specific fields, **unchanged for both families**. Rejected: per-format field allowlists (invented mechanism that constrains what platform features already do).

### Surfaces & discovery
- **D-32:** `imagai list-engines` gains a **format column** (name, format, model, base_url), so the new provider is visible in discovery (success criterion 3). Rejected: a new `list-providers` subcommand; format visible only in `--json`.
- **D-33:** Per-request selection is **unchanged and generic**: `--engine <name>` on CLI, the engine row in the TUI, the `engine` field on the web API. Only the adapter behind the selected name changes. Success criterion 4 (all three surfaces) is therefore satisfied by construction, not by new picker code. Rejected: a new `--provider` flag distinct from `--engine`.
- **D-34:** TUI and web need **no new picker logic** beyond rendering the format in the existing engine lists. The format column is display-only; no filter, no format selector.
- **D-35:** An unparseable response from the new format produces the **existing per-item `{"error": …}` envelope** and `anyhow::bail!` naming engine, HTTP status, and body. Rejected: a new error type; silent fallback to another engine (would spend credits on a request the user didn't ask for — D-05's rationale).

### Scope boundaries
- **D-36:** The shipped adapter must differ from OpenAI in **both** request keys and response keys — not `{model, prompt, n, size, response_format}` on the way out, not `{data:[{url|b64_json}]}` on the way in. This is what makes "no OpenAI body leaking in" (success criterion 2) checkable. Rejected: a near-OpenAI shape (cheap, proves nothing).
- **D-37:** The new format's **documented error shape** is mapped into the standard error envelope; unmapped shapes fall back to best-effort raw body with status. Never a panic, never a silent success.
- **D-38:** The `FORMAT` env var is recorded in **`.env.example` in this phase**. Phase 4 (DIST-01) owns the full multi-engine setup documentation; only the variable itself and its allowed values are added here.
- **D-39:** Video (VID-01) stays **v2 and out of this phase**, but the adapter seam is designed as the mechanism video will reuse later — the format registry must be extensible without restructuring.

### the agent's Discretion
User accepted all sixteen recommended answers wholesale. Remaining latitude: which concrete real-world vendor the shipped adapter targets (must satisfy D-36's genuinely-different-shape test and be mockable in tests), the internal module shape for the format registry and the adapter, exact enum/naming in Rust, test layout, and the precise `list-engines` column layout — provided every locked decision above and the codebase conventions hold.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase definition
- `.planning/ROADMAP.md` § Phase 3 — goal, success criteria (all four), `**Mode:** mvp`
- `.planning/REQUIREMENTS.md` — PRVD-01
- `.planning/PROJECT.md` — Constraints (Rust, reuse the current architecture, speed, no build step), Key Decisions row "Multi-vendor providers, not OpenAI-compatible-only"

### Codebase evidence (existing system to extend)
- `.planning/codebase/INTEGRATIONS.md` — the full external-integration map: default base URL `https://api.openai.com/v1`, the OpenAI-compatible images API, chat completions, the OpenRouter vision path, model discovery via `GET {base_url}/models`, and the auth shape
- `.planning/codebase/ARCHITECTURE.md` — the shared pipeline and the documented `extra_params`-assembled-per-frontend anti-pattern (do not repeat)
- `.planning/codebase/STRUCTURE.md` § Where to Add New Code — CLI flag, web field, TUI row, provider body, DTO insertion points
- `.planning/codebase/CONVENTIONS.md` — naming, `anyhow`, `#[serde(default)]`, test layout, fmt/clippy-clean baseline
- `src/config.rs` — `EngineConfig { api_key, base_url, model }`, `Settings::load()` env parsing (`IMAGAI__ENGINES__<NAME>__*`), `get_engine`, `engine_names`
- `src/provider.rs` — `build_images_body`, `edit_transport`, `generate_images`, `generate_edits`, `resolve_url`, `http_client`, `truncate`, and the existing string-sniffing dispatch this phase generalizes
- `src/models.rs` — `ImageGenerationRequest` (incl. `extra_params`), `ImagesResponse`
- Phase 1/2 canonical refs — D-03, D-05, D-06, D-07, D-28 still bind

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `provider::http_client()` (`src/provider.rs`) — the single outbound HTTP helper; every adapter must go through it, never build its own client
- `resolve_url()` / `build_images_body()` / `edit_transport()` (`src/provider.rs`) — the dispatch seam this phase generalizes
- `ImageGenerationResponse` / `ImagesResponse` (`src/models.rs`) — the response shape the adapter must map onto (D-27)
- `save_image_from_url` / `save_image_from_b64` (`src/utils.rs`) — result persistence, reused unchanged
- `record_result` (`src/history.rs`) — history recording on the shared save path, unchanged
- `extra_params` on `ImageGenerationRequest` — the existing provider-specific field mechanism, kept (D-31)
- `Settings::get_engine` / `engine_names` / `list_output_images` — selection and discovery helpers
- Mock server modes (`tests/common/mod.rs`) — the pattern for a new adapter's mock endpoint

### Established Patterns
- Frontend builds a request DTO → `generate_image_core` orchestrates → provider does HTTP; a new format must land in `provider.rs`/a sibling module, never in the frontends
- Config comes free via `IMAGAI__ENGINES__<NAME>__*` env vars — the FORMAT field is the fourth one
- Fail fast with `anyhow::bail!` naming engine+model before any socket opens (D-05)
- `#[serde(default)]` on every optional API field; tolerant response parsers (the `GET /models` tolerant parse is the precedent for "parse documented shape, fall back leniently")
- Model discovery: `GET {base_url}/models`, used by `list-engines --all` and the TUI engine browser

### Integration Points
- `EngineConfig` (`src/config.rs`) gains a `format` field; `Settings::load()` parses `FORMAT` alongside `API_KEY`/`BASE_URL`/`MODEL`
- New dispatch point in `src/provider.rs` (or a new `src/provider_format.rs` sibling) replacing/augmenting the string sniffing
- CLI: `print_available_engines` (`src/cli.rs`) gains the format column (D-32); the existing `--engine` flag needs no change
- TUI: engine list rendering gains the format; no new controls (D-34)
- Web: `GeneratePayload.engine` unchanged; if the engine list is served, it carries the format
- `.env.example` — the FORMAT variable and its allowed values (D-38)

</code_context>

<specifics>
## Specific Ideas

No specific requirements beyond the decisions above. The user's stated driving intent (PROJECT.md Key Decisions): the author will use named non-compatible vendors, including video later — hence a declared, extensible seam rather than more string sniffing.

</specifics>

<deferred>
## Deferred Ideas

- Full edit/fork support for the non-OpenAI format — future phase (D-26)
- A second and third non-OpenAI adapter — future, once the seam is proven (D-25)
- Video provider (VID-01) — v2; it will reuse this seam (D-39)
- Full provider setup documentation (multi-engine `.env` walkthrough) — Phase 4, DIST-01 (D-38 covers only the variable)
- `list-engines --all` behavior changes for the new format's model discovery (its `GET /models` may not exist on the new vendor) — decide during planning if discovery fails, else Phase 4

</deferred>

---

*Phase: 3-Multi-Vendor Providers*
*Context gathered: 2026-10-11*
