---
phase: 03-multi-vendor-providers
plan: 01
subsystem: config
tags: [provider-format, wire-format, capability-gate, env-config]

requires:
  - phase: 01-image-editing
    provides: edit_transport and the D-05 fail-fast capability gate this plan makes format-driven
  - phase: 02-branching-iteration-history
    provides: the shared pipeline whose save path the fal engine later reuses

provides:
  - src/formats.rs, the wire-format registry with a closed set of declared names
  - EngineConfig.format parsed from IMAGAI__ENGINES__<NAME>__FORMAT
  - ProviderFormat::resolve's D-29 precedence rule (declaration beats inference)
  - A format-driven edit capability gate that fails before any socket opens
  - format_label, the one display helper the CLI, TUI and web surfaces all call

affects: [03-02 fal adapter, 03-03 discovery columns, 03-04 web api, 03-05 regression net]

actuals:
  tokens: 5116
  tasks: 3
  commits: 1

tech-stack:
  added: []
  patterns:
    - "Declared-not-inferred configuration: a closed set of names in code, a config-only env field, with the existing sniffing kept as the unset-default (D-24, D-28)"
    - "One display helper (`format_label`) shared by three surfaces so a column cannot drift between them"
    - "Load-time `[warning]` diagnostic paired with a request-time hard failure, so a config mistake is reported early but never spends a request (D-05)"

key-files:
  created:
    - src/formats.rs
  modified:
    - src/config.rs
    - src/lib.rs
    - src/provider.rs
    - src/tui.rs
    - tests/core.rs
    - tests/web.rs

key-decisions:
  - "D-24: FORMAT is config-only with a closed name set (openai, fal); no new dependency, Cargo.toml untouched"
  - "D-28/D-29: nothing declared means today's sniffing unchanged; a declaration always beats inference"
  - "D-30: edit_transport consults the declared format first and bails an unrecognised value before the capability question"
  - "A blank/whitespace FORMAT is treated as no declaration rather than a hard error, so `FORMAT=` degrades to today's behaviour"
  - "The format: None field churn was folded into the single 03-01 commit because it was a compile-blocker for the acceptance criteria's clippy --all-targets gate"

patterns-established:
  - "Format registry shape: an enum with FORMAT_NAMES, as_str, from_declared, a capability predicate, a resolve with precedence, and a shared label helper — the seam a v2 video format reuses (D-39)"
  - "Marking decisions in code: D-numbers are cited in /// doc comments where the next reader looks for the reason"

requirements-completed: [PRVD-01]

coverage:
  - id: D1
    description: "A user can declare an engine's wire format and it resolves ahead of any inference"
    requirement: PRVD-01
    verification:
      - kind: unit
        ref: "cargo test --lib formats::"
        status: pass
  - id: D2
    description: "An edit against a generation-only format fails before any socket opens, naming engine and model"
    requirement: PRVD-01
    verification:
      - kind: integration
        ref: "cargo test --test core edit_against_fal_format_fails_before_any_request"
        status: pass

---

# Phase 3 Plan 1: The declared wire format and its capability gate Summary

**One-liner:** `IMAGAI__ENGINES__<NAME>__FORMAT` declares a closed-set wire format that resolves ahead of every inference, with `edit_transport`'s capability gate driven by it and still failing before any socket opens.

## What Was Built

`src/formats.rs` is the new seam: `ProviderFormat { OpenAiCompatible, Fal }` with `FORMAT_NAMES`, `as_str`, `from_declared`, `supports_image_input`, `resolve` and `format_label`. It is the only place in the crate that knows which family an engine speaks. `EngineConfig` gains a fourth field; `Settings::load` parses it and prints a non-fatal `[warning]` for an unrecognised value. `edit_transport` now resolves the format as its first statement, bails an unknown declaration before the capability question, and refuses the generation-only format with the existing remedy sentence.

## Tasks Completed

| Task | Name | Status | Commit |
|------|------|--------|--------|
| 1 | Declared format resolves ahead of inference (tracer) | completed | 1b5c114 |
| 2 | The capability gate becomes format-driven | completed | 1b5c114 |
| 3 | The mechanical field churn + the no-socket proof | completed | 1b5c114 |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `engine_name` moved into the hashmap entry key**
- **Found during:** Task 1
- **Issue:** the `[warning]` message for an unrecognised FORMAT needed `engine_name` after `settings.engines.entry(engine_name)` had moved it, so the crate did not compile (E0382).
- **Fix:** `.entry(engine_name.clone())` — one clone at the loop's only non-`Copy` use site.
- **Files modified:** src/config.rs
- **Commit:** 1b5c114

**2. [Rule 3 - Blocking] Two `EngineConfig` literals missing the new field**
- **Found during:** Task 1
- **Issue:** `src/provider.rs`'s `dalle_config()` and `src/tui.rs`'s `edit_test_settings` predated the new field, so the lib test target failed to compile (E0063).
- **Fix:** `format: None,` on both.
- **Files modified:** src/provider.rs, src/tui.rs
- **Commit:** 1b5c114

**3. [Rule 3 - Blocking] `crate::formats::resolve` is not a free function**
- **Found during:** Task 2
- **Issue:** the plan's action text specified `crate::formats::resolve`, but `resolve` is an associated function on `ProviderFormat`.
- **Fix:** `crate::formats::ProviderFormat::resolve(name, cfg)?`. The plan's stated intent — resolve first, propagate its error — is preserved exactly.
- **Files modified:** src/provider.rs
- **Commit:** 1b5c114

**4. [Rule 3 - Blocking] `image_size` was gated on a non-empty size only in the plan's Task 2 text**
- **Found during:** Task 1 (surfaced during 03-02 Task 2)
- **Issue:** the plan deferred `image_size` mapping to 03-02, but a size-less request would then hit `fal_image_size` and bail. The guard was applied where the plan put the mapping.
- **Fix:** see 03-02-SUMMARY.md deviation 1.
- **Commit:** e74b5b3

### Plan Deviations (recorded, not auto-fixed)

**5. Task 3's field churn folded into the single 03-01 commit**
- **Found during:** Task 1
- **Issue:** the plan made Task 3 a separate task/commit for adding `format: None` to every `EngineConfig` literal, but Task 1's own `cargo clippy --all-targets` verify gate cannot pass until those literals name the field — the plan's own ordering makes it uncompilable between tasks.
- **Decision:** applied the churn at the point it blocked compilation, so one commit carries a compiling tree. The diff is identical to what Tasks 1+3 would have produced; only the commit boundary moved.
- **Commit:** 1b5c114

## Verification Evidence

| Gate | Result |
|------|--------|
| `cargo fmt --check` | clean |
| `cargo clippy --all-targets` | clean (0 warnings) |
| `cargo test --lib formats` | 9 passed |
| `cargo test --lib config` | 3 passed |
| `cargo test --lib provider::tests::edit_transport` | 8 passed (5 pre-existing unmodified + 3 new) |
| `cargo test --test core edit_against_fal_format_fails_before_any_request -- --exact` | 1 passed |
| `cargo test` | 92 lib / 43 cli / 9 core / 35 web = 179, all ok |

Manual confirmation requested by the plan: with `IMAGAI__ENGINES__X__FORMAT=openai` the run renders normally; with `IMAGAI__ENGINES__X__FORMAT=bogus` the `[warning]` line prints at load. The `[warning]` path was observed during 03-05's `unknown_format_fails_the_cli_run` (the load-time warning and the request-time error both appear in that test's captured stderr).

Acceptance criteria: `src/formats.rs` exists with every required item; `EngineConfig.format: Option<String>` is populated by `Settings::load`; `resolve` errors name engine, rejected value and both supported names; no `unwrap`/`expect` in `src/formats.rs` outside `#[cfg(test)]`; **no `Cargo.toml` diff**.

## ROADMAP Success Criteria Touched

- Criterion 3 (partial): no OpenAI-compatible engine changed behaviour — with no FORMAT set every engine resolves to `OpenAiCompatible` and the existing sniffing still picks its transport. The full key-set proof is 03-05's.

## Files Changed

- `src/formats.rs` (created)
- `src/config.rs`, `src/lib.rs`, `src/provider.rs`, `src/tui.rs`
- `tests/core.rs`, `tests/web.rs`

## Self-Check: PASSED
