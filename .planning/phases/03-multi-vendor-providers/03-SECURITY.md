---
phase: 03-multi-vendor-providers
padded_phase: "03"
verdict: SECURED
audited: 2026-10-12
auditor: gsd-security-auditor
asvs_level: 1
block_on: high
threats_open: 0
threats_total: 19
threats_closed: 19
baseline_tests:
  lib: 108
  cli: 52
  core: 26
  web: 40
  total: 226
---

# Phase 3 Security Audit — Multi-Vendor Providers

**Verdict:** SECURED — every declared mitigation is present in shipped code at HEAD (`ffddab8`).
**`threats_open`: 0** (`block_on: high`; ASVS level 1 — presence verified in the cited file).

Register extracted from the five plans' `<threat_model>` blocks: T-03-01…T-03-18 plus
T-03-SC (the accepted supply-chain threat, restated in all five plans — counted once).
Verification depth is ASVS L1: each mitigation is greppable at the cited file:line and
pinned by a named test.

**Suite state at audit time** (all four binaries run live, not cited):

| Suite | Passed | Failed | Ignored |
|-------|--------|--------|---------|
| lib | 108 | 0 | 0 |
| cli | 52 | 0 | 0 |
| core | 26 | 0 | 0 |
| web | 40 | 0 | 0 |

`cargo clippy --all-targets` clean, `cargo fmt --check` clean. Zero ignored tests across
all suites — the T-03-18 anti-weakening contract holds (204 baseline + 22 new = 226).

## Threat Verification

| Threat ID | Category | Severity | Disposition | Evidence |
|-----------|----------|----------|-------------|----------|
| T-03-01 | Tampering | medium | mitigate | CLOSED — `from_declared` closed set `src/formats.rs:46-52`; `resolve` bails naming engine + both supported names `src/formats.rs:112-123`; load-time `[warning]` `src/config.rs:96-104`. Tests: `an_unknown_declaration_names_the_engine_and_the_supported_names` (formats.rs:268), `unknown_format_fails_the_cli_run` |
| T-03-02 | Denial of Service | medium | mitigate | CLOSED — `resolve` is first statement of `edit_transport` and is pure `src/provider.rs:172`; capability gate `supports_image_input` at `src/provider.rs:178-185`, before any client build. Test: `edit_against_fal_format_fails_before_any_request` (`tests/core.rs:540`) asserts `mock.requests()` empty |
| T-03-03 | Spoofing | low | mitigate | CLOSED — `format_label` returns the declared value verbatim when rejected `src/formats.rs:132-140`. Test: `format_label_shows_the_resolved_name_or_the_raw_typo` (`src/formats.rs:287`) |
| T-03-04 | Spoofing | high | mitigate | CLOSED — `Authorization: Key {api_key}` set explicitly, not `bearer_auth` `src/provider.rs:663-666`; no auth header in the `--verbose` dump (`src/provider.rs:649-657`). Test: `fal_engine_saves_image_through_the_shared_pipeline` asserts `mock.auth_for(...) == ["Key test"]`. **Live:** capture log shows `authorization: Key fal-secret-key-123` |
| T-03-05 | Tampering | high | mitigate | CLOSED — 200-not-documented-shape bails naming engine, status, truncated body `src/provider.rs:701-710`; empty list → `API returned no images.` `src/provider.rs:730-735`; image with neither → `No image data found in API response.` `src/provider.rs:716-727`. Tests: `fal_unparseable_success_response_bails_naming_engine_status_and_body`, `fal_empty_images_list_reports_no_images` |
| T-03-06 | Information Disclosure | medium | mitigate | CLOSED — only `detail`'s `msg`/`type` mapped `src/provider.rs:953-985`; raw fallback through the existing `truncate(…, 400)` `src/provider.rs:692`; no header, key or base URL in any message. Tests: `fal_flat_error_body_is_mapped_into_the_envelope`, `fal_detail_array_error_body_is_mapped_into_the_envelope`, `fal_html_error_body_falls_back_to_the_truncated_raw_body`, `fal_empty_error_body_is_a_named_failure`, `fal_non_utf8_error_body_is_a_named_failure` |
| T-03-07 | Tampering | medium | mitigate | CLOSED — adapter keys assigned *after* the `extra_params` merge so they always win `src/provider.rs:597-607`. Tests: `fal_body_keeps_png_output_format_over_an_override`, `fal_body_merges_extra_params_unchanged`, `fal_body_merges_openai_shaped_keys_from_extra_params` (IN-05 scope stated) |
| T-03-08 | Elevation of Privilege | low | mitigate | CLOSED — only a `data:` prefix is split, remainder to the existing decoder `src/provider.rs:717-721`; a data URI performs no second fetch, so no SSRF surface. Test: `fal_data_uri_response_saves_without_a_fetch` |
| T-03-09 | Spoofing | medium | mitigate | CLOSED — CLI `src/cli.rs:761` and TUI `src/tui.rs:1704` both derive the cell from the single `format_label` helper. Tests: `list_engines_shows_configured_engines`, `list_engines_shows_a_broken_format_declaration_verbatim`, `engines_tab_rows_name_the_format_column` |
| T-03-10 | Information Disclosure | medium | mitigate | CLOSED — every new `.env.example` line is commented (`.env.example:15-18, 58-68`); only `[key]` placeholders and the public `https://fal.run` host. Test: `the_shipped_env_example_loads_every_engine_it_documents` |
| T-03-11 | Denial of Service | low | mitigate | CLOSED — **superseded by BLOCKER-2's fix, same observable contract:** per-engine `Err` arm prints one `[error]` line and the loop continues `src/cli.rs:810-816`; discovery is now format-gated `src/provider.rs:30-37`. Tests: `fal_engine_discovery_is_refused_even_with_a_reachable_host`, `list_engines_all_fetches_models`, `openai_compatible_engine_discovery_is_unchanged` |
| T-03-12 | Elevation of Privilege | medium | mitigate | CLOSED — all three option labels use `option.textContent` with `${engine.format}` (`web_interface.html:853, 873, 891`); no `innerHTML` on the label path. Test: `history_form_is_served_with_engine_format_labels` |
| T-03-13 | Information Disclosure | low | accept | CLOSED (accepted) — the added field discloses a member of a two-value closed set on a route that already exposes names, models and base URLs; localhost-only by INTEGRATIONS.md; no new route or data class |
| T-03-14 | Tampering | low | mitigate | CLOSED — single source: `"format": crate::formats::ProviderFormat::format_label(cfg)` `src/web.rs:174`; no resolved/declared pair. Tests: `engines_endpoint`, `engines_endpoint_reports_a_broken_format_declaration_verbatim` |
| T-03-15 | Tampering | critical | mitigate | CLOSED — the fal branch is entered only behind `matches!(format, ProviderFormat::Fal)` `src/provider.rs:260`; every other engine reaches the code below unchanged. Tests: `openai_default_engine_body_is_unchanged`, `stability_default_engine_body_is_unchanged` — full key-set equality, not substrings |
| T-03-16 | Spoofing | medium | mitigate | CLOSED — a declared `openai` resolves to `OpenAiCompatible` and the within-family sniffing is untouched `src/provider.rs:285-300`. Tests: `declared_openai_format_keeps_the_sniffed_transports`, `declared_openai_edit_engine_still_saves_with_lineage` |
| T-03-17 | Denial of Service | medium | mitigate | CLOSED — parity argument recorded in 03-05-SUMMARY; anchors verified: `App.engine` is the unchanged string field (`src/tui.rs:210`) and `generate_image_core` is exercised by both CLI and web fal tests (`fal_engine_generates_from_the_cli`, `fal_engine_generates_through_the_web_api`) |
| T-03-18 | Repudiation | high | mitigate | CLOSED — full suite run live: 108/52/26/40 = 226, **0 ignored, 0 failed**; per-suite counts match the 04-fix baseline exactly, so no test was removed, weakened or ignored |
| T-03-SC | Tampering | high | accept | CLOSED (accepted) — verified `git diff --stat 36ad9b9..ffddab8 -- Cargo.toml Cargo.lock` is **empty**: no dependency added, no new supply-chain surface |

### Open threats

None. Every threat resolves CLOSED or documented-accepted.

## Live Proofs (derived by this audit, not cited from 03-VERIFICATION.md)

### 1. BLOCKER-2 key isolation — a fal key never reaches `api.openai.com`

A raw capture server listened on `127.0.0.1:18947` and the real binary was run with
`HTTPS_PROXY`/`ALL_PROXY` pointed at it, so **any** outbound request this process
attempted would have been captured. Engine: `API_KEY=fal-secret-key-123`,
`MODEL=fal-ai/flux/schnell`, `FORMAT=fal`, **`BASE_URL` unset** — exactly the minimal fal
configuration `.env.example` documents.

```
$ HTTPS_PROXY=http://127.0.0.1:18947 ./target/debug/imagai list-engines
Engine Name              Format       Default Model                      Base URL                     API Key Set
fal                      fal          fal-ai/flux/schnell                N/A (fal.run)                ✅ Set

[error] Failed to fetch models for 'fal': Engine 'fal' has no model list to fetch: the 'fal' format is generation-only and names its model by URL path, so there is no GET /models endpoint to call.

# CAPTURE LOG
(EMPTY — zero bytes ever sent to any host, api.openai.com included)
```

Control, same proxy, OpenAI-compatible engine with `BASE_URL` unset — proving the capture
server works and that the gate is precisely what protects the fal case:

```
$ HTTPS_PROXY=http://127.0.0.1:18947 ./target/debug/imagai list-engines   # format unset, model dall-e-3
[error] Failed to fetch models for 'oai': error sending request for url (https://api.openai.com/v1/models)

# CAPTURE LOG
=== connection from 127.0.0.1 at 14:13:44 ===
CONNECT api.openai.com:443 HTTP/1.1
Host: api.openai.com:443
user-agent: imagai/0.1.0
```

That CONNECT is the pre-fix path BLOCKER-2 described: unpatched, it carries
`Authorization: Bearer fal-secret-key-123` to `api.openai.com`. The fal engine makes no
connection at all. Mechanism: `models_url` returns `None` for a non-listing format
(`src/provider.rs:106-114`, unit test `a_format_without_a_listing_endpoint_has_no_discovery_url_at_all`)
and `fetch_models` bails before building any client (`src/provider.rs:32-37`).

### 2. The `.env` load path — malformed warns, good loads (BLOCKER-1 / WR-06)

```
# (a) .env copied from .env.example per the file's own line-2 instruction
$ cp .env.example $D/.env && (cd $D && imagai list-engines)
Engine Name              Format       Default Model                      Base URL                     API Key Set
openai_dalle3            openai       dall-e-3                           N/A (Official OpenAI)        ⚠️ Not Set / Default
openai_gpt               openai       gpt-image-1                       N/A (Official OpenAI)        ⚠️ Not Set / Default
[warning] Skipping model fetch for 'openai_dalle3': missing API key.
[warning] Skipping model fetch for 'openai_gpt': missing API key.
# every documented engine loaded; no "could not parse" line

# (b) malformed .env (the exact CR-01 line)
$ printf 'IMAGAI__ENGINES__<NAME>__FORMAT=openai\n…' > $D2/.env && (cd $D2 && imagai list-engines)
[warning] could not parse /…/envbad.geY1/.env: Error parsing line: 'IMAGAI__ENGINES__<NAME>__FORMAT=openai', error at line index: 17
[warning] No engines configured. Check your .env file or environment variables.
```

A malformed file is now named, never silent (`src/config.rs:62-67`); a good copy yields
every documented engine rather than zero. Tests: `the_shipped_env_example_loads_every_engine_it_documents`,
`a_malformed_env_file_is_reported_not_swallowed`.

### 3. The fal gateway URL construction — no user-controlled string reaches a second host

Join site: `fal_base_url` (`src/provider.rs:579-584`, default from
`ProviderFormat::Fal.default_host()` = `https://fal.run`) → `resolve_url(&fal_base_url(config), model)`
(`src/provider.rs:646`), where `resolve_url` (`src/provider.rs:88-97`) is
`format!("{}/{}", base.trim_end_matches('/'), path.trim_start_matches('/'))`.

Live: `BASE_URL=http://127.0.0.1:18963`, `FORMAT=fal`, hostile model ids:

```
capture log — every request landed on the configured host, model as a path segment
POST /https://evil.example.com/steal-the-key HTTP/1.1
authorization: Key fal-secret-key-123
host: 127.0.0.1:18963
POST /evil.example.com/x HTTP/1.1        host: 127.0.0.1:18963     (leading slashes trimmed)
POST /overt/path HTTP/1.1                host: 127.0.0.1:18963     (`../` normalized away by the URL crate)
{"image_size":"square_hd","num_images":1,"output_format":"png","prompt":"url probe"}
```

Because the model id is always concatenated *after* the base as a path segment, a scheme,
`//authority` form, or `..` traversal in the model id cannot move the request — or the API
key — to a second host. The same join with the default base yields `host=fal.run` for every
hostile id tested (`https://evil.example.com/steal`, `//evil.example.com/x`, `../../overt/path`,
`x?y=z`, `#frag`). The captured body also confirms the D-36 wire shape live: no `model`, `n`,
`size`, `response_format`, `quality` or `style`; auth is `Key`, not `Bearer`.

## Unregistered Flags

None. No SUMMARY carries a `## Threat Flags` section. The one piece of new attack surface
that appeared during implementation — the fal credential reaching `api.openai.com` through
`fetch_models` — was caught by code review as BLOCKER-2 (`de54540`), fixed, and its fix is
registered against T-03-04/T-03-11/T-03-13 in this audit.

## Accepted Residuals (recorded, not open)

- **ROB-01** — `imagai generate` exits 0 on a per-item provider error. Phase 4's success
  criterion ("Non-zero exit on failure"); recorded at `.planning/STATE.md:108` and
  03-05-SUMMARY deviation 1. Out of this phase's scope by instruction.
- **IN-05** — `extra_params` may carry OpenAI-shaped keys into a fal body from the library
  API. Guard dismissed against locked decision D-31 (a per-format allowlist would invent the
  mechanism D-31 rejects); scope is documented in the test name and no CLI/web surface can
  reach those keys. Recorded in 03-REVIEW.md.
- **T-03-13** — `/api/engines` exposes per-engine configuration on an unauthenticated,
  CORS-open route. Accepted: the added `format` field discloses a two-value closed set on a
  route that already exposes names, models and base URLs; localhost-only per INTEGRATIONS.md.

## Audit Method

Read-only. No source file was modified (`git status` clean apart from this file). Every
mitigation was verified by locating the code at the cited file:line and confirming a named
test exercises it; the three proofs above were produced by running the shipped binary at
`ffddab8` against local capture servers under `$TMPDIR/opencode/audit/`.
