---
phase: 01-image-editing
verdict: OPEN_THREATS
threats_open: 0
threats_closed: 16
threats_total: 21
asvs_level: 1
audited: 2026-10-07T19:25:00+03:30
---

# Security Review: Phase 01 — Image Editing

**Verdict:** OPEN_THREATS (5 open, all non-blocking `low` accepts below the `high` block threshold)
**Blocking threats open:** 0 — all three `high` threats (T-01-SC, T-01-10, T-01-11) are CLOSED.

## Closed (16)

| Threat ID | Category | Severity | Evidence |
|-----------|----------|----------|----------|
| T-01-SC | Tampering (supply chain) | high | Cargo.lock pins mime_guess 2.0.5 + unicase 2.10.0 with approved checksums; human `approve` recorded in 01-00-SUMMARY.md; no `[patch]` overrides |
| T-01-16 | Spoofing | medium | Same checksum pins, crates.io registry source, no patch section |
| T-01-01 | Tampering | medium | `edit_transport` capability gate bails naming engine+model before any request (`src/provider.rs`, `src/cli.rs:388`, `src/core.rs:38-39`) |
| T-01-15 | Tampering | medium | Edit failure exits 1 in `cmd_edit` only (`src/cli.rs:416-417`) |
| T-01-02 | Information Disclosure | medium | `Path::file_name()` reduction; Source chunk holds filename only |
| T-01-03 | Tampering | medium | Byte-safe `{path:?}` formatting; `truncate()` never sees user paths |
| T-01-04 | DoS | medium | MAX_IMAGES=16 guard before any read/request, both counts in message |
| T-01-07 | Spoofing | medium | Auto-pick printed via `Using most recent image:`; explicit path short-circuits |
| T-01-09 | Information Disclosure | low | Gallery shape/ordering pinned by regression tests |
| T-01-10 | Tampering | high | `output_dir_file` + `is_plain_filename` guard on web edit path; traversal refused |
| T-01-11 | Tampering | high | `output` routed through same guard; no writes outside output dir |
| T-01-12 | Tampering | medium | Same `edit_transport` gate via shared core; incapable engine fails naming model |
| T-01-13 | Denial of Service | medium | Count guard before read/network on web path |
| T-01-19 | Tampering | medium | TUI refs resolved against output_dir; missing ref aborts with naming log |
| T-01-20 | Denial of Service | medium | `EDIT_ROWS` namespace; tab indices consistent |
| T-01-21 | Spoofing | low | TUI engine membership check with not-configured message |

## Accepted risks (5, all `low`)

- **T-01-05** (Information Disclosure, low) — accept. Rationale in 01-01-PLAN.md.
- **T-01-06** (Spoofing, low) — accept. TLS/DTO trust rationale in 01-01-PLAN.md.
- **T-01-08** (Tampering, low) — accept. Symlink-in-own-output-dir is same-user action on a localhost single-user tool; revisit if `imagai web` is ever exposed beyond localhost. Rationale in 01-02-PLAN.md.
- **T-01-14** (Information Disclosure, low) — accept. Engine-name-in-error rationale in 01-03-PLAN.md.
- **T-01-22** (Information Disclosure, low) — accept. Same-local-user rationale in 01-04-PLAN.md.

## Unregistered flags

None — no SUMMARY contains a `## Threat Flags` section; no new network/auth/file-access surface beyond the planned `POST /api/edit`.
