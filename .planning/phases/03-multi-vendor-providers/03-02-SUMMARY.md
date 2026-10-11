---
phase: 03-multi-vendor-providers
plan: 02
subsystem: provider
tags: [fal-ai, adapter, wire-format, response-mapping, error-mapping]

requires:
  - phase: 03-multi-vendor-providers
    provides: "03-01's ProviderFormat registry and the format-resolved dispatch seam this plan enters behind ProviderFormat::Fal"
  - phase: 01-image-editing
    provides: the shared pipeline's save path (save_image_from_url / save_image_from_b64) and PNG lineage metadata the fal response maps onto

provides:
  - The fal adapter: fal_base_url, build_fal_images_body, fal_image_size, fal_error_message, fal_generate
  - FalImagesResponse and FalImage DTOs carrying no OpenAI key
  - A fal mock mode and per-path Authorization capture in tests/common/mod.rs
  - End-to-end proof a fal engine saves a PNG through the unchanged shared pipeline

affects: [03-03 discovery, 03-04 web surface, 03-05 regression and parity net]

actuals:
  tokens: 4638
  tasks: 3
  commits: 1

tech-stack:
  added: []
  patterns:
    - "Adapter-behind-a-guard dispatch: generate_images resolves the declared format first and enters the fal branch only behind matches!(format, ProviderFormat::Fal), which makes D-28's no-regression guarantee structural rather than incidental"
    - "Response-key asymmetry as a testable property: the fal DTOs document the absent OpenAI keys (no data, no b64_json, no top-level url) where the next reader looks for the shape"
    - "Adapter keys assigned after the extra_params merge, so the adapter's own invariant (output_format: png) always wins over an operator override"
    - "Documented-error-shape mapping into the standard message grammar, with an unmapped shape falling back to the truncated raw body plus status"

key-files:
  created: []
  modified:
    - src/provider.rs
    - src/models.rs
    - tests/common/mod.rs
    - tests/core.rs

key-decisions:
  - "D-25: exactly one named non-OpenAI adapter ships — fal.ai, chosen because it differs from OpenAI in both request and response keys (D-36) and because one endpoint family serves image and video models alike, which is what makes the seam the mechanism v2 video reuses (D-39)"
  - "D-27: the adapter maps onto ImageGenerationResponse carrying image_url or image_b64_json, so save_image_from_url/save_image_from_b64, record_result, PNG metadata and the web gallery all work with no change"
  - "D-36: model id is the URL path, auth is `Authorization: Key …`, and the body carries only prompt/num_images/image_size/output_format — no model, n, size, response_format, quality or style"
  - "output_format is forced to png and assigned last, because the shared save path writes PNG tEXt chunks and cannot be changed (T-03-07)"
  - "usage and estimated_cost stay None: fal reports timings and seed, not usage, and inventing a cost mapping would be a guess the phase did not ask for"
  - "FalMode::Error grew an error_type field so the flat infrastructure failure shape (detail + error_type) is observable; the plan's arm carried only detail"

patterns-established:
  - "Per-path Authorization capture in the mock, so an adapter's auth scheme is asserted rather than assumed (T-03-04)"
  - "A mock mode enum declared in full on the tracer task even when only one arm is needed, so the mock stays in one edit"

requirements-completed: [PRVD-01]

coverage:
  - id: D1
    description: "A fal engine generates one image end to end through the unchanged shared pipeline, carrying no OpenAI body key but prompt"
    requirement: PRVD-01
    verification:
      - kind: integration
        ref: "cargo test --test core fal_engine_saves_image_through_the_shared_pipeline"
        status: pass
  - id: D2
    description: "A data-URI fal response saves without a second network fetch"
    requirement: PRVD-01
    verification:
      - kind: integration
        ref: "cargo test --test core fal_data_uri_response_saves_without_a_fetch"
        status: pass
  - id: D3
    description: "Every documented fal error shape maps into the standard envelope; an unmapped shape falls back to the truncated raw body with the status"
    requirement: PRVD-01
    verification:
      - kind: integration
        ref: "cargo test --test core fal_flat_error_body_is_mapped_into_the_envelope, fal_detail_array_error_body_is_mapped_into_the_envelope, fal_error_body_without_detail_falls_back_to_the_raw_body, fal_unparseable_success_response_bails_naming_engine_status_and_body"
        status: pass

---

# Phase 3 Plan 2: The fal adapter Summary

**One-liner:** `POST {base}/{model_id}` with `Authorization: Key …` and a body of `prompt`/`num_images`/`image_size`/`output_format`, mapped onto the existing `ImageGenerationResponse` so the save, history and gallery paths are reused unchanged.

## What Was Built

`fal_generate` is the one non-OpenAI adapter. It builds the URL by joining the fal host to the model id, so the model is a path rather than a body key; auth is the explicit `Key` header, not `bearer_auth`; and the body merges `extra_params` first, then assigns `prompt`, `num_images`, `image_size` and `output_format` last so the adapter's own invariant wins. `fal_error_message` maps both documented failure shapes into the same `msg (type: t)` grammar `extract_api_error` produces, and returns `None` for anything unmapped so the caller falls back to the truncated raw body with the status. `generate_images` resolves the declared format at its top and enters the fal branch only behind `matches!(format, ProviderFormat::Fal)`.

## Tasks Completed

| Task | Name | Status | Commit |
|------|------|--------|--------|
| 1 | A fal engine generates one image (tracer) | completed | 5a6632f |
| 2 | The request builder — image_size, the size bail, extra_params | completed | e74b5b3 |
| 3 | Response and error mapping | completed | e74b5b3 |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] `image_size` was set unconditionally, so a size-less request bailed**
- **Found during:** Task 2 (unit test `fal_body_omits_image_size_for_an_empty_size`)
- **Issue:** Task 1's assignment `body["image_size"] = json!(fal_image_size(...)?)` hit the size bail for a request with no size, so a legitimate size-less request failed instead of omitting the key.
- **Fix:** guard the assignment with `if !request.size.is_empty()`, so the key is absent exactly when the request carries no size. This is the behaviour Task 2's `fal_body_omits_image_size_for_an_empty_size` requires, applied at the site Task 1 created.
- **Files modified:** src/provider.rs
- **Commit:** e74b5b3

**2. [Rule 2 - Missing functionality] `FalMode::Error` could not express the flat infrastructure shape**
- **Found during:** Task 3 (`fal_flat_error_body_is_mapped_into_the_envelope`)
- **Issue:** the plan's `FalMode::Error { status, detail }` emitted only `{"detail": …}`, so the documented flat shape's `error_type` companion could not be sent and the mapper's `error_type` branch was untestable.
- **Fix:** added `error_type: Option<String>` to the arm and emitted it when present. All three existing call sites updated.
- **Files modified:** tests/common/mod.rs, tests/core.rs
- **Commit:** e74b5b3

**3. [Rule 1 - Test assertion] a test indexed a sorted row by position**
- **Found during:** 03-03 Task 2
- **Issue:** `engines_tab_rows_name_the_format_column` asserted `rows[1]`/`rows[2]`, but `engine_names()` is sorted so `fal` precedes `mock`. Fixed in 03-03.
- **Commit:** 80c9d86

**4. [Rule 1 - Test edit] an edit replaced a signature instead of inserting beside it**
- **Found during:** 03-04 Task 2
- **Issue:** an `edit` call used a neighbouring test's signature as its `oldString`, leaving the original body orphaned.
- **Fix:** restored `history_form_is_served_with_fork_controls` intact and added `history_form_is_served_with_engine_format_labels` as a separate test. Fixed in 03-04.
- **Commit:** 7c057ab

**5. [Rule 2 - Test coverage] a zero `num_images` was not pinned**
- **Found during:** Task 2
- **Issue:** the plan required clamping `n` up to 1 but did not list a test for it.
- **Fix:** added `fal_body_clamps_a_zero_count_up_to_one`, so `request.n.max(1)` is covered.
- **Files modified:** src/provider.rs (tests)
- **Commit:** e74b5b3

## Verification Evidence

| Gate | Result |
|------|--------|
| `cargo fmt --check` | clean |
| `cargo clippy --all-targets` | clean (0 warnings) |
| `cargo test --test core fal_engine_saves_image_through_the_shared_pipeline -- --exact` | 1 passed |
| `cargo test --lib provider::tests::fal` | 6 passed |
| `cargo test --test core fal_` | 8 passed (7 new + 03-01's gate test) |
| `cargo test` (all four suites) | 98 lib / 43 cli / 16 core / 35 web = 192, all ok |

The wire shape was confirmed by eye through the tracer's assertions on the captured body — `prompt`, `num_images`, `image_size` and `output_format` present, every OpenAI images-API key absent, and the auth header exactly `Key test`.

Acceptance criteria: `fal_generate`, `build_fal_images_body` and `fal_base_url` exist with `generate_images` dispatching behind `matches!(format, ProviderFormat::Fal)`; `FalImagesResponse`/`FalImage` carry `#[serde(default)]` on every field; `tests/common/mod.rs` has `FalMode`, `set_fal_mode`, `auth_for` and a `HeaderMap`-taking `dispatch`; the diff to `generate_images` is confined to the resolve-and-dispatch block at its top.

## ROADMAP Success Criteria Touched

- Criterion 1 (complete): a named non-OpenAI-compatible provider adapter with its own request and response shape exists alongside the OpenAI-compatible engines.
- Criterion 2 (complete): selecting it builds that provider's format with no OpenAI body leaking in — asserted key-by-key in the tracer and by full key-set exclusion.

## Files Changed

- `src/provider.rs`, `src/models.rs`, `tests/common/mod.rs`, `tests/core.rs`

## Self-Check: PASSED
