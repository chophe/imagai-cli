---
phase: 02-branching-iteration-history
plan: 05
subsystem: history-store
tags: [rust, filename-collision, fork, lineage, g1, gap-closure]

requires:
  - phase: 02-branching-iteration-history
    provides: the `.imagai-history.json` store, the shared `generate_image_core` pipeline and the CR-06 collision-free default-filename branch this fix mirrors
provides:
  - `history::filename_taken` — the "is this filename already claimed" probe honouring both the disk half and the sidecar-record half of a claim
  - a collision-free `n == 1` arm of the `source_image` naming branch in `generate_image_core`
  - deterministic fork filename regression coverage (e2e + pinned assertion)
affects: [branching-iteration-history, history-store naming]

actuals:
  tokens: 2429
  tasks: 2
  commits: 2
plan_head_before: 150239af56a4bc2b533e013b079f7f6e833dda0f
plan_head_after: f5cf9c24f99f809859a924e250d2d9b86d486e49

tech-stack:
  added: []
  patterns:
    - "Claim-probe-then-bump: a candidate filename is accepted only after `history::filename_taken` clears both halves of a claim (file on disk, sidecar record); bumps reuse the shared `suffix_numbered` `_N` scheme so all branches name siblings identically"

key-files:
  created: []
  modified:
    - src/history.rs
    - src/core.rs
    - tests/cli.rs

key-decisions:
  - "A filename claim has two halves — a file on disk and a sidecar record naming it — and both must be probed: D-20 keeps a node whose file was deleted externally, so a disk-only probe would let the next fork land on a name a kept-but-missing node owns"
  - "An unreadable sidecar reads as taken (always disambiguate, never overwrite), and the bump loop is bounded so an always-taken probe cannot spin"
  - "Deterministic assertion over permissive: `fork_keeps_both_branches_visible` accepts exactly `cat-edit_1.png`, so a filename collision can never pass silently again"

patterns-established:
  - "Collision-safe naming arm: probe `crate::history::filename_taken` before committing to a name, bump through `suffix_numbered`, keep the canonical name when free — the CR-06 shape, now covering the `source_image` branch"

requirements-completed: [BRCH-02, BRCH-03]

coverage:
  - id: D1
    description: "`history::filename_taken` refuses a name claimed by a file on disk or by a sidecar record, treating an unreadable sidecar as taken"
    requirement: BRCH-03
    verification:
      - kind: unit
        ref: "src/history.rs#filename_taken_is_false_for_a_name_nothing_claims / _sees_a_file_the_store_does_not_know / _honours_a_record_whose_file_is_gone / _treats_an_unreadable_sidecar_as_taken"
        status: pass
    human_judgment: false
  - id: D2
    description: "Two forks from one source save to distinct paths, keep both images on disk, and leave a truthful 3-record store with a marker-free tree"
    requirement: BRCH-02
    verification:
      - kind: e2e
        ref: "tests/cli.rs#two_forks_from_one_source_keep_both_images"
        status: pass
      - kind: e2e
        ref: "tests/cli.rs#fork_keeps_both_branches_visible (now asserts exactly cat-edit_1.png)"
        status: pass
    human_judgment: false

duration: 15min
completed: 2026-10-10
status: complete
---

# Phase 2 Plan 5: G-1 Gap Closure Summary

**Fork filenames are collision-safe: `history::filename_taken` clears a candidate against both the file on disk and the sidecar record naming it, and the `n == 1` `source_image` arm bumps through the shared `suffix_numbered` `_N` scheme — two forks from one source now keep both images with distinct names and a truthful store.**

## Performance

- **Duration:** 15 min
- **Started:** 2026-10-10T06:56:56Z
- **Completed:** 2026-10-10T07:12:37Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- `history::filename_taken(dir, filename)` (`src/history.rs`) — the claim probe both halves of the store honour: a file on disk (real overwrite hazard) or a sidecar record naming it (D-20 keeps a node whose file was deleted externally, so the name is not free again). An unreadable sidecar (CR-02) reads as taken — disambiguating is always the safe direction. Four unit tests cover free / file-on-disk / record-claim / unreadable-sidecar.
- The `n == 1` arm of the `source_image` branch in `generate_image_core` (`src/core.rs:88-108`) now probes the store and bumps `cat-edit.png → cat-edit_1.png → cat-edit_2.png …` via the shared `suffix_numbered` scheme the sibling branches use (first bump `_1`, identical to what `n > 1` produces), keeping the canonical name when free — single-edit naming is unchanged. The bump loop is bounded (`MAX_NAME_BUMPS`) so an always-taken probe cannot spin.
- `tests/cli.rs#fork_keeps_both_branches_visible` accepts exactly one deterministic filename (`cat-edit_1.png`) — the `||` escape that let a collision pass silently is gone.
- `tests/cli.rs#two_forks_from_one_source_keep_both_images` — the G-1 live repro from `02-VERIFICATION.md`, inverted deterministically: two forks from one root leave `cat-edit.png` and `cat-edit_1.png` both on disk, a 3-record sidecar with three distinct filenames both parented to the root, and a `history` tree with the root once, both children listed and no missing marker.

## Task Commits

1. **Task 1: Make the n==1 source_image filename arm collision-safe** — `5264a55` (fix)
2. **Task 2: Pin the regression in tests (drop the || escape, add the two-fork e2e)** — `f5cf9c2` (test)

**Plan metadata:** final docs commit via `gsd_run query commit` (see completion notes).

## Files Created/Modified

- `src/history.rs` — `pub fn filename_taken` beside `node_id_for`/`record_for_filename` (G-1 doc comment naming why the record half matters); four `history::tests` unit tests
- `src/core.rs` — collision-probe loop replacing the `else { base }` arm of the `source_image` branch (`// G-1:` comment); `MAX_NAME_BUMPS` bound beside `suffix_numbered`
- `tests/cli.rs` — deterministic filename assertion in `fork_keeps_both_branches_visible`; new `two_forks_from_one_source_keep_both_images` e2e after it

## Decisions Made

- A filename claim covers both halves of the store. A disk-only probe would still let a new fork land on a name a kept-but-missing node owns — the same two-nodes-one-file condition G-1 created — so `filename_taken` consults `read_records` too (via the existing `record_for_filename` helper).
- Unreadable sidecar ⇒ taken. Reading a corrupt store as "free" would reuse a name whose records are unreadable, and re-derive the collision; the safe direction is always to disambiguate.
- Bounded bump loop (`MAX_NAME_BUMPS = 1000`). The plan's unbounded `while` would spin forever in the always-taken case an unreadable sidecar produces; past the bound the last candidate is used and the save proceeds exactly as before.
- The `total: 2` argument passed to `suffix_numbered` in the probe loop is the smallest value that activates its `_N` scheme (`total <= 1` returns the input unchanged), so bumped names are byte-identical to what `n > 1` produces — one naming scheme, not a second one.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Plan-text error: the expected filename was specified with a hyphen**
- **Found during:** Task 2 (`fork_keeps_both_branches_visible` assertion)
- **Issue:** Plan step 1 said to assert `stdout.contains("cat-edit-1.png")` and called it "the real name under the fix", but `suffix_numbered` produces `cat-edit_1.png` (underscore). The hyphen form is precisely the name the old permissive `||` admitted the code never produces (02-VERIFICATION.md § G-1 calls this out), and the plan's own must_haves and step 2 both use `edit_1`.
- **Fix:** Assert `cat-edit_1.png`; the deterministic assertion is otherwise exactly as planned. The first run of the tightened test failed on the hyphen, which is how the error surfaced.
- **Files modified:** tests/cli.rs
- **Verification:** `cargo test --test cli fork` → 9 passed; full suite green.
- **Committed in:** `f5cf9c2` (Task 2 commit)

**2. [Rule 2 - Missing Critical] Bounded the collision-probe bump loop**
- **Found during:** Task 1 (implementing the probe loop in `src/core.rs`)
- **Issue:** The plan specified an unbounded `while filename_taken(...)` loop. When the sidecar is unreadable the probe returns true for every candidate, so the loop would never terminate — a hang in the save path, reachable from any edit/fork with a corrupt store.
- **Fix:** `for bump in 0..MAX_NAME_BUMPS` with `MAX_NAME_BUMPS = 1000`; past the bound the last candidate is used and the save proceeds as before.
- **Files modified:** src/core.rs
- **Verification:** clippy clean; full suite green; a `✗`-free two-fork run confirms the loop breaks on the first free name.
- **Committed in:** `5264a55` (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (1 bug, 1 missing critical)
**Impact on plan:** Both confined to the plan's own tasks. The first corrects a typo-class plan-text error the implementation immediately exposed; the second closes a hang the plan's unbounded loop would have shipped. No scope creep — Phase 1 naming behavior is untouched when there is no collision.

## Issues Encountered

- Two intermediate `Edit` calls in this session truncated source mid-flight (one dropped the body of `suffix_numbered`, one dropped the `fork_rejects_unknown_source` header). Both were caught by reading the file back before any verification run or commit, and restored in the following edit; the committed trees are correct — verified by the green full suite and by reading the final state of both regions.
- No authentication gates, no package installs, no blockers.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- G-1 (the phase's only BLOCKER) is closed: the live repro in `02-VERIFICATION.md` § G-1 now inverts — two forks from one root produce `cat.png`, `cat-edit.png`, `cat-edit_1.png` on disk and a 3-record sidecar with three distinct filenames.
- The verifier should re-run truth #9 ("Forking never deletes or detaches anything") and the BRCH-02/BRCH-03 requirement rows against HEAD `f5cf9c2`.
- Full suite: **163 passed, 0 failed** (lib 80 / cli 40 / core 8 / web 35 — the pre-existing 158 plus 5 new tests, zero removed). `cargo fmt --check` exit 0. `cargo clippy --all-targets` prints no `warning:`/`error:` lines.

---

*Phase: 02-branching-iteration-history*
*Completed: 2026-10-10*

## Self-Check: PASSED

| Claim | Check | Result |
|-------|-------|--------|
| `02-05-SUMMARY.md` exists at the phase dir | `[ -f … ]` | FOUND |
| Task 1 commit `5264a55` is in HEAD's history | `git merge-base --is-ancestor` | FOUND |
| Task 2 commit `f5cf9c2` is in HEAD's history | `git merge-base --is-ancestor` | FOUND |
| `filename_taken` exists in both files | `rg -c` | 9 hits in `src/history.rs`, 1 in `src/core.rs` |
| Tightened assertion uses the deterministic `cat-edit_1.png` | `rg` | 5 hits in `tests/cli.rs`, no `cat-edit-1.png` hyphen form remains |
| Bump loop bounded | `rg MAX_NAME_BUMPS` | `src/core.rs:98`, `:211` |
| Full suite green | `cargo test` | 163 passed, 0 failed (lib 80 / cli 40 / core 8 / web 35) |
| fmt clean | `cargo fmt --check` | exit 0 |
| clippy clean | `cargo clippy --all-targets` | no `warning:`/`error:` lines |
