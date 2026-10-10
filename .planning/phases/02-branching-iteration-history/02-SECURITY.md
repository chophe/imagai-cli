---
phase: 02-branching-iteration-history
verdict: SECURED
threats_open: 0
threats_closed: 14
threats_total: 14
asvs_level: 1
audited: 2026-10-10T12:57:36.233Z
---

# Security Review: Phase 02 — Branching & Iteration History

**Verdict:** SECURED — every registered Phase 02 threat is CLOSED.
**Blocking threats open:** 0 — all three `high`-tier threats (T-02-01, T-02-05, T-02-10) are CLOSED.

## Closed (14)

| Threat ID | Category | Severity | Evidence |
|-----------|----------|----------|----------|
| T-02-01 | Tampering | high | Every consumer that joins a record `filename` under `output_dir` reduces it through `history::node_path` (`src/history.rs:210-216`) first: `load_view` (`src/history.rs:255`), the missing-file probe (`src/history.rs:332`) and the CLI's last join before the read (`src/cli.rs:510-532`, added in `56984f8`); the web fork arm reduces the same way (`src/web.rs:585`). Unit tests `load_view_reduces_a_filename_that_carries_a_parent_component` and `node_path_rejects_a_value_with_no_final_component` pin the reduction |
| T-02-02 | Tampering | medium | `load_view` re-roots a record whose `parent` is absent or names an unknown id (`src/history.rs:236-239`); derivation is index-based with a visited set, so a cyclic `parent` chain cannot loop, and `MAX_TREE_DEPTH` stops a deep chain from aborting the process (`src/history.rs:220-228`) |
| T-02-03 | Information Disclosure | low | accept. The `Could not read history at {path}` sentence names the operator's own configured `output_dir`, not another user's file — rationale in 02-01-PLAN.md |
| T-02-04 | Denial of Service | low | accept. Whole-sidecar read-modify-write per save is D-11's locked cost for a single-user local tool; the write is a temp-file `rename`, so an interrupted write never destroys the store (`src/history.rs:183-191`) — rationale in 02-01-PLAN.md |
| T-02-05 | Tampering | medium | `--from` resolves through `history::resolve_fork_source`, which matches node ids and recorded filenames against records only; a bare filename is resolved inside `output_dir` and never against the CWD (`src/history.rs:513-567`). A node's `filename` is reduced with `node_path` and refused when the reduction is not a no-op before the last join (`src/cli.rs:510-532`, `56984f8`), so `--from <id8>` cannot name an injected `../` record. CLI regressions `fork_rejects_a_record_filename_that_escapes_output_dir` and `edit_from_refuses_a_separator_carrying_record_filename` assert exit 1 and zero provider requests |
| T-02-06 | Repudiation | medium | Nothing on the fork path writes to an existing file (D-13): forks append a record rather than mutating the source node's row, so the original branch stays in the tree. Pinned by `two_forks_from_one_source_keep_both_images` and `fork_keeps_both_branches_visible` |
| T-02-07 | Tampering | medium | `src/tui.rs::history_select` maps the re-indexed tab dispatch through the same `EDIT_ROWS` namespace used by the renderer, so a missing or out-of-range row cannot route to the About renderer |
| T-02-08 | Elevation of Privilege | low | The tab indices are derived from one list and consumed by both dispatch and render, so the two cannot drift; a future tab cannot silently reach the About renderer |
| T-02-09 | Tampering | high | Every path-shaped request field (`from`, `refs`, `output`) passes `is_plain_filename` (`src/web.rs:271-275`) before resolution, and `output_dir_file` (`src/web.rs:263-264`) reduces to a final component inside `output_dir` with `parent() != Some(dir)` refused — traversal outside the directory is answered, never served |
| T-02-10 | Tampering | medium | The resolved node's `filename` is reduced with `output_dir_file` before it is joined (`src/web.rs:581-603`), the same reduction `history::node_path` performs; combined with T-02-01's read-side guard, both halves of the store-to-handler path reduce. `fork_endpoint_reduces_a_traversal_shaped_record_filename` passes |
| T-02-11 | Denial of Service | medium | A reference count above 16 is rejected before any read or request on both the CLI and web paths, and the message names both counts |
| T-02-12 | Information Disclosure | low | accept. The fork-failure sentence names the operator's own `from` value and `output_dir`, both client-supplied — rationale in 02-04-PLAN.md |
| T-02-13 | Tampering | medium | The `n == 1` `source_image` naming arm (`src/core.rs:134`) probes for a free filename before writing, so a fork can never land on the name a kept-but-missing node owns |
| T-02-SC | Tampering (supply chain) | low | accept. No packages are installed by this plan; Cargo.lock remains the only source of truth — rationale in 02-05-PLAN.md |

## Unregistered flags

None — no SUMMARY contains a `## Threat Flags` section.

## Phase 02 audit note

T-02-01 / T-02-05 / T-02-10 were CLOSED by `56984f8` (`fix(02): T-02-01/T-02-05/T-02-10 reduce the CLI fork record filename before the join`), which closed the unreduced `settings.output_dir.join(filename)` in `src/cli.rs` — one guard, at the last join before the read, covering both `fork` and `edit` through `resolve_from`. The web surface was already closed by T-02-09/T-02-10's mitigation and was left unchanged. `threats_open` is 0 and the verdict is SECURED. Verification: `cargo fmt --check` clean, `cargo clippy --all-targets` clean, `cargo test` green (lib 80 / cli 43 / core 8 / web 35 — 166 total, three tests added, none removed). Gates ran in an isolated git worktree that was fast-forwarded into `main` and removed on completion.
