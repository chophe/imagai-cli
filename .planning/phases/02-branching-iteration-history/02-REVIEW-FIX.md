---
phase: 02-branching-iteration-history
fixed_at: 2026-10-10T05:20:00Z
review_path: .planning/phases/02-branching-iteration-history/02-REVIEW.md
iteration: 1
findings_in_scope: 17
fixed: 15
skipped: 2
status: partial
---

# Phase 2: Code Review Fix Report

**Fixed at:** 2026-10-10T05:20:00Z
**Source review:** `.planning/phases/02-branching-iteration-history/02-REVIEW.md`
**Iteration:** 1

**Summary:**
- Findings in scope: 17 (6 Critical, 5 Warning, 6 Info)
- Fixed: 15
- Skipped/dismissed: 2 (IN-02, IN-06) — WR-05 recorded as a partial fix with a locked-decision conflict

All work ran in an isolated git worktree (`gsd-reviewfix/02-…`, removed on completion). The
crate has no `node_modules`, so `cargo` gates ran inside the worktree against its own
`target/` directory; `cargo fmt --check`, `cargo clippy --all-targets` and `cargo test` all
pass there and the results below are reproducible from the merged tree.

## Fixed Issues

### CR-01: `/api/fork` reads files resolved against the process CWD, not `output_dir`

**Files modified:** `src/history.rs`, `src/web.rs`, `tests/cli.rs`, `tests/web.rs`
**Commit:** `7a3c82b`
**Applied fix:** `resolve_fork_source`'s step (3) checked `Path::new(from).is_file()` before
`dir.join(from)`, so a bare filename resolved against the process CWD — and the web surface
binds `0.0.0.0` with `allow_origin(Any)`. Step (3) is now two branches: a value carrying a
separator is the literal path the operator named (left untracked, D-22), and a bare name is
joined onto `dir` and adopted as a root — never resolved relative to the CWD. The web `fork`
handler additionally refuses any `ForkSource::File` whose parent is not `dir`, so a
separator-carrying path cannot reach the provider even if `is_plain_filename` is ever relaxed.

### CR-02: An unreadable sidecar is read as an empty store, then silently overwritten

**Files modified:** `src/history.rs`, `tests/cli.rs`
**Commit:** `aa70667`
**Applied fix:** `read_records` mapped *every* `read_to_string` failure to `Ok(Vec::new())`.
Only `ErrorKind::NotFound` now means "no history yet"; a non-UTF-8 sidecar (which fails with
`InvalidData`, not `NotFound`) and any other I/O error bail with the Copywriting Contract's
`Could not read history at {path}: {why}` sentence. A failed *write* stays non-fatal, as
required: the image is real and lazy adoption recovers the node.

### CR-03: When the CWD is `output_dir`, a bare `--from` is never adopted

**Files modified:** `src/history.rs`, `src/web.rs`, `tests/cli.rs`, `tests/web.rs`
**Commit:** `7a3c82b`
**Applied fix:** Same root cause as CR-01 — the old step (3) compared `literal.parent()`
(`Some("")` for a bare name) against `Some(dir)`, so when the CWD *was* `output_dir` the
`recorded: true` adoption branch was unreachable and the child recorded as a root. Bare names
now always resolve to `dir.join(from)`, which makes adoption reachable and keeps the lineage.
Fixed together with CR-01 because they are one code path.

### CR-04: `load_view` recursion is unbounded — a deep chain aborts the process

**Files modified:** `src/history.rs`, `src/tui.rs`
**Commit:** `386dcbb`
**Applied fix:** `build_node` recursed once per tree level with no bound; `MAX_HISTORY_DEPTH`
lived only in the TUI flattener, which runs *after* `load_view` had already recursed. The cap
now lives in the shared read path as `history::MAX_TREE_DEPTH` and the TUI flattener uses
that same constant instead of its private copy. Traversal stops descending at the cap, and the
existing stranded pass re-roots what was skipped, so every record stays visible (D-20).

### CR-05: Parent resolution by basename is ambiguous; the oldest record wins

**Files modified:** `src/history.rs`
**Commit:** `3c044eb`
**Applied fix:** One helper, `record_for_filename`, now owns the lookup and returns the
**newest** match — the last record in sidecar (append) order, which is deterministic and the
branch an edit of that file most plausibly came from. `record_result`'s parent lookup,
`resolve_fork_source`'s filename step and `node_id_for` all route through it. Resolution is
now documented on the helper rather than implied by `records.iter().find(...)`.

### CR-06: Two images generated in the same second collide on filename

**Files modified:** `src/core.rs`, `tests/core.rs`
**Commit:** `5f7750c`
**Applied fix:** The default branch of the filename selection was the only one missing
`suffix_numbered`. It now calls `suffix_numbered(&generate_filename(...), i, request.n)`,
exactly like its three siblings.
**Note — this touches a line the reviewer flagged as predating Phase 2.** The fix was applied
because it is the only way Phase 2's new store stops recording two nodes against one filename
(which then feeds CR-05), and because the review explicitly directs it. It changes no Phase 1
code path *other than* removing the silent overwrite: with `n > 1` and no `--output`, two
images generated in the same second now land at `…_1.png` and `…_2.png` instead of one path.
No Phase 1 test depended on the collision, and the behaviour is now collision-safe rather
than lossy.

### WR-01: Non-atomic sidecar write plus a discarded error

**Files modified:** `src/history.rs`, `src/core.rs`
**Commit:** `7e20e2a`
**Applied fix:** `append_records` wrote the sidecar in place with `fs::write` (truncate then
write). It now writes `HISTORY_TEMP_FILENAME` beside the sidecar and `rename`s it over, which
is atomic within one filesystem, so an interrupted or failed write leaves the previous content
intact. This matters more after CR-02: an unreadable store is now an error, so a truncated
sidecar would have bricked every surface until hand-repair. The module doc's false
"lazy adoption recovers it" claim is corrected. `core.rs` replaced `let _ = record_result(...)`
with a `[warning] Lineage was not recorded for …` on stderr; the command still succeeds,
because the image is already on disk.

### WR-02: Prompt text is embedded raw into the row grammar

**Files modified:** `src/history.rs`
**Commit:** `d337700`
**Applied fix:** `sanitize_prompt` strips ASCII control characters in `record_result` — the
one place every surface's prompt funnels through — so the store never holds a newline or an
escape sequence. A hand-edited sidecar remains the only possible source, and that file is
already fully trusted.

### WR-03: `/api/history` discloses the server's absolute path, and `/api/images/{name}` serves the sidecar

**Files modified:** `src/history.rs`, `src/web.rs`, `tests/web.rs`
**Commit:** `45ee7c5`
**Applied fix:** `GET /api/history` now reports the bare directory name rather than
`view.output_dir`. The UI-SPEC's own example payload carries
`"output_dir": "generated_images"`, and the SPA never reads the field; the CLI keeps rendering
the configured path the operator typed (D-19 pins one *serializer*, not one value).
`serve_image` now refuses `HISTORY_FILENAME` and `HISTORY_TEMP_FILENAME`. The directory
listing was already filtered to image extensions, so it never disclosed the sidecar.

### WR-04: Same-second records fall back to uuid ordering, so sibling order is arbitrary

**Files modified:** `src/history.rs`
**Commit:** `4a2a4e2`
**Applied fix:** The sort key is now sidecar position (append order) instead of the record id.
**The review's own suggested snippet was not applied**: it kept `id` ahead of the index
(`b.1.id.as_str(), b.0`), so the index could only break a tie between two records with
*both* the same `created` and the same id — which never happens, leaving the tie unresolved.
The index replaces the id rather than following it. Children sort `(created, index)` ascending
and roots `(created, index)` descending.

### IN-01: `history_tree_prints_connectors` asserts coverage it does not have

**Files modified:** `tests/cli.rs`
**Commit:** `f1ec03d`
**Applied fix:** The comment "Three nodes are missing, so the plural warning line prints" now
backs a real assertion of the plural warning line, and the `let _ = out_path;` filler is gone.

### IN-03: `ForkPayload.prompt` is required, so a missing field bypasses the JSON error envelope

**Files modified:** `src/web.rs`, `tests/web.rs`
**Commit:** `3ed1423`
**Applied fix:** `#[serde(default)] prompt: String`, so a body with no `prompt` reaches the
emptiness check that already follows and returns the `{ "success": false, "error": … }`
envelope instead of axum's plain-text 400.

### IN-04: Two implementations of the same filename reduction, with different rules

**Files modified:** `src/web.rs`, `src/history.rs`, `tests/web.rs`
**Commit:** `9bb4a23`
**Applied fix:** `output_dir_file` is now `history::node_path`, so the `HistoryNode` doc's
claim that every consumer goes through this reduction is true. **This is a deduplication, not
a behavioural fix** — no reachable web input produced a different result from the two
implementations (every web entry point is gated by `is_plain_filename` first, and both
reductions agree on any value with a final component). The tests pin the shared contract so a
future divergence fails loudly rather than silently re-forking the two rules.

### IN-05: Dead branch in the session clause

**Files modified:** `src/cli.rs`
**Commit:** `e24f937`
**Applied fix:** `if sessions == 1 { "" } else { "s" }` sat inside the `else` arm of an outer
`if sessions == 1`, so the `""` arm was unreachable. The inner branch is now the plain plural.
The rendered string is identical — the outer guard already drops the clause for one session.

## Skipped Issues

### WR-05: A fork from an unrecorded file records its child as a root

**File:** `src/cli.rs:505-510`, `src/history.rs:281-285`
**Reason:** partially resolved by CR-01/CR-03 and CR-05; the residual is a conflict with locked
decision D-22, recorded rather than worked around.
**Original issue:** `record_result` re-derives the parent from a basename, so a source that was
never adopted leaves the child with `parent: None`.

Disposition: the reproduced half of this finding was CR-03's `cwd == output_dir` case, fixed in
`7a3c82b` — adoption now happens, so the parent is found. With CR-05 the re-derivation is
deterministic (newest match), so a fork no longer attaches to a stale node. The remaining
residual is an **out-of-dir file named explicitly**, which D-22 mandates stays untracked — so
there is genuinely no parent to record and `parent: None` is the correct, documented outcome
(`print_fork_line` already omits the node clause for it rather than inventing one).

What was *not* done: the review's suggested fix of threading the resolved node id through
`resolve_fork_source → cmd_fork → generate_image_core`. That changes the signature of
`generate_image_core`, the shared Phase 1 pipeline, which the task forbids. The residual
mis-link it would prevent needs two records sharing a filename, which CR-06/CR-05 now make
rare and resolve deterministically to the newest node rather than the oldest — a valid edge,
not corruption. **Recommend a follow-up phase** if exact node-id fidelity across a fork matters.

### IN-02: The TUI clones the full row list on every frame

**File:** `src/tui.rs:1136-1142`
**Reason:** not a defect — a performance cost the review itself labels "out of v1 perf scope".
**Original issue:** `draw_history_tree` clones `state.rows` (each row holds a `Line<'static>`
plus a full `HistoryNode` clone) on every draw.

Disposition: deferred. There is no behavioural bug, and the two-pass borrow fix sits in the
TUI draw path, which has no terminal test harness — a regression there would not be caught by
any existing test. The cost is real but bounded by history length and only paid while the
History tab is open. **Recommend a dedicated perf phase** with a render test first.

### IN-06: TUI selection is not clamped after a reload shrinks the row list

**File:** `src/tui.rs:1058-1067`, `src/tui.rs:1196-1201`
**Reason:** false positive — the premise contradicts the conclusion.
**Original issue:** "`load_history` resets `selected` to 0, but a reload triggered from
`drain_messages` … can still leave `selected` past the end of a shorter row list."

Disposition: dismissed, no code change. `load_history` (`src/tui.rs:1004`) is the *only* writer
of `rows` — `rg '\.rows\b' src/tui.rs` shows three readers and zero other writers — and every
reload route funnels through it: `drain_messages` (:661), `open_history_pick` (:1093), the Tab
key (:2020) and the tests (:2256, :2275). It sets `selected: 0` and `scroll: 0` on every call,
so `rows.get(h.selected)` is in range whenever `rows` is non-empty, and `rows.get(_)` returns
`None` safely when it is empty. `history_move` is the only other mutator and it clamps through
`scroll_selection`. The suggested extra clamp at the end of `load_history` would be equivalent
to the `selected: 0` already there. No reachable out-of-range state was found.

---

## Verification

All three gates ran inside the isolated worktree (crate has no `node_modules`; `cargo`
used the worktree's own `target/`).

```
$ cargo fmt --check
(no output — clean)

$ cargo clippy --all-targets
    Checking imagai v0.1.0 (/Users/ali/dev/rust/imagai-cli/.claude/worktrees/rf-02-91979-1791588376)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.02s

$ cargo test
running 76 tests   (lib)     test result: ok. 76 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
running 39 tests   (cli)     test result: ok. 39 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
running 8 tests    (core)    test result: ok. 8 passed;  0 failed; 0 ignored; 0 measured; 0 filtered out
running 35 tests   (web)     test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**No regressions.** Diffing the set of test-function names between the parent commit
(`45a0ecc`) and HEAD shows **zero removed tests** and 18 added. Clippy is clean with no
warnings on `--all-targets`.

**Failing-first evidence.** Each BLOCKER (CR-01, CR-02, CR-03, CR-04, CR-06) and each of
WR-02, WR-03, WR-04 was verified to fail against the pre-fix source before the fix was
applied, by temporarily reverting only the production hunk and re-running the new test:

| Finding | Pre-fix result of the new test |
|---|---|
| CR-01 | `Unexpected success … code=0` (CLI) / `assertion left == right failed` (web) |
| CR-03 | `Unexpected success … code=0`, `records.len() == 2` mismatch |
| CR-02 | `expect_err("unreadable is an error")` panic; sidecar clobbered |
| CR-04 | `fatal runtime error: stack overflow, aborting` (SIGABRT) |
| CR-06 | `assertion left != right failed: two distinct paths` |
| WR-02 | control-character assertion panic |
| WR-03 | `panicked … absolute path` / sidecar contents served |
| WR-04 | `left: ["aaa11111","zzz99999"] right: ["zzz99999","aaa11111"]` |

**Phase 1 boundary.** No Phase 1 behaviour changed except the CR-06 collision fix, called out
above and directed by the review. `generate_image_core`'s signature is untouched, the D-22
out-of-dir path is untouched, the Copywriting Contract sentences are unchanged, and the
`HistoryView` JSON shape is unchanged (only the web's `output_dir` *value*, not the field set).

_Fixed: 2026-10-10T05:20:00Z_
_Fixer: the agent (gsd-code-fixer)_
_Iteration: 1_
