---
phase: 02-branching-iteration-history
reviewed: 2026-10-09T22:45:22Z
depth: standard
files_reviewed: 9
files_reviewed_list:
  - src/history.rs
  - src/core.rs
  - src/cli.rs
  - src/tui.rs
  - src/web.rs
  - web_interface.html
  - tests/core.rs
  - tests/cli.rs
  - tests/web.rs
findings:
  critical: 6
  warning: 5
  info: 6
  total: 17
status: clean
fixed_by: .planning/phases/02-branching-iteration-history/02-REVIEW-FIX.md
findings_status:
  CR-01: fixed:7a3c82b
  CR-02: fixed:aa70667
  CR-03: fixed:7a3c82b
  CR-04: fixed:386dcbb
  CR-05: fixed:3c044eb
  CR-06: fixed:5f7750c
  WR-01: fixed:7e20e2a
  WR-02: fixed:d337700
  WR-03: fixed:45ee7c5
  WR-04: fixed:4a2a4e2
  WR-05: partial:conflict-D22
  IN-01: fixed:f1ec03d
  IN-02: skipped:not-a-defect-out-of-v1-perf-scope
  IN-03: fixed:3ed1423
  IN-04: fixed:9bb4a23
  IN-05: fixed:e24f937
  IN-06: skipped:false-positive
---

# Phase 2: Code Review Report

**Reviewed:** 2026-10-09T22:45:22Z
**Depth:** standard
**Files Reviewed:** 9
**Status:** clean — all 17 findings dispositioned in [02-REVIEW-FIX.md](02-REVIEW-FIX.md)
(15 fixed, 1 partial with a locked-decision conflict, 1 deferred as non-defect, 1 false positive)


## Summary

Phase 2 adds a new persistence layer — the `.imagai-history.json` sidecar — plus fork
resolution shared by three surfaces. The architecture is sound: one recording hook in
`generate_image_core`, one `HistoryView` serializer, one row grammar, one resolver. All 3
suites pass (`36 + 7 + 30` tests), `cargo clippy --all-targets` is clean, and
`cargo fmt --check` passes. The new CLI paths exit non-zero correctly (no repeat of ROB-01),
and there is no `unwrap`/`expect` in the new production code.

That is where the good news ends. The new store's read path is where the defects live, and
they are not hypothetical: every finding below was reproduced against the actual code.

**Verified behaviour that concerned me did *not* reproduce:** the `seen`-set cycle guard
does terminate (including a self-parent), `record_result`'s out-of-dir guard works, the TUI
tab dispatch is exhaustive, and the `… n more` scroll remainder is correct.

Every finding below is labelled **BLOCKER** or **WARNING**. Reproductions were run in a
throwaway copy of the crate (`/tmp/imagai-verify`) — no submitted source file was modified.

---

## Structural Findings (fallow)

No structural pre-pass was supplied with this review, so there is no fallow substrate to
report. The cross-module facts below were established by direct analysis and appear as
narrative findings (notably CR-01, CR-03 and WR-05, which are all cross-module facts about
the same resolver contract being broken differently in `web.rs` and `cli.rs`).

---

## Narrative Findings (AI reviewer)

## Critical Issues

### CR-01: `/api/fork` reads files resolved against the process CWD, not `output_dir`

> **FIXED** — commit `7a3c82b`

**Files:** `src/history.rs:419-437`, `src/web.rs:489-502`, `src/web.rs:584-598`, `src/cli.rs:505-510`
**Issue:** `resolve_fork_source` step (3) checks `Path::new(from).is_file()` *before*
`dir.join(from)`. For a bare name (which `is_plain_filename` at `src/web.rs:266-270`
deliberately permits — it only rejects `/` and `\`), that resolution is relative to the
**process working directory**, not to `output_dir`. The web handler then accepts
`ForkSource::File` and uses it as `source_image` with no check that it lives in `dir`.

Reproduced end-to-end: a file created only in the CWD was read and its bytes uploaded to the
provider, and the response was `success: true`:

```
POST /api/fork {"from": "probe_cwd_only.png", "prompt": "x"}
-> 200 {"success": true, ...}
-> mock received 1 /v1/images/edits request
-> source bytes from CWD uploaded to provider: true
```

Unit-level confirmation of the resolver itself:
`resolve_fork_source(dir, "probe_cwd_file.png") -> Ok(File { path: "probe_cwd_file.png",
recorded: false })` where `probe_cwd_file.png` exists only in the CWD.

Consequences: (a) any browser origin (`.layer(CorsLayer::new().allow_origin(Any))`,
`src/web.rs:127-130`, server binds `0.0.0.0` by default) can make the tool read and upload
any CWD-resident file by bare name — the documented trust boundary at
`.planning/phases/02-branching-iteration-history/02-UI-SPEC.md:261` ("bare filename in
`output_dir`") is not what the code enforces; (b) the CLI is affected identically.

**Fix:** In `resolve_fork_source`, try the recorded-id/filename and `dir.join(name)` paths
first, and only fall back to a CWD-relative literal when `from` contains a path separator.
In `fork`, reject any `ForkSource::File` whose parent is not `dir`:

```rust
Ok(crate::history::ForkSource::File { path, .. }) => {
    if path.parent() != Some(dir.as_path()) {
        return Json(json!({
            "success": false,
            "error": format!("'{from}' is not inside output_dir")
        })).into_response();
    }
    if !path.is_file() { /* ... existing not-found envelope ... */ }
    path
}
```

### CR-02: An unreadable sidecar is read as an empty store, then silently overwritten

> **FIXED** — commit `aa70667`

**File:** `src/history.rs:96-104`
**Issue:** `read_records` maps *every* `read_to_string` failure to `Ok(Vec::new())`. Only a
missing file should mean "empty"; a permission error, an I/O error, or a non-UTF-8 sidecar
(a hand-edit, a truncated write from another tool) also silently reads as "no history".

The next `record_result` then calls `append_records` → `fs::write`, which **truncates and
replaces** the file. Reproduced:

```
write sidecar = b"\xff\xfe\x00{\""        (non-UTF-8, 5 bytes)
read_records  -> Ok(0)
record_result -> Ok("57302aee")
sidecar after -> 161 bytes of fresh JSON (the 5 original bytes are gone)
```

The operator's entire history was destroyed without a single warning, precisely to avoid the
"silent empty tree" the module doc (`src/history.rs:92-95`) and UI-SPEC promise never to
emit. `load_view` reports `nodes: 0`, so `imagai history` cheerfully prints
"🌳 No history yet".

**Fix:** Distinguish "absent" from "unreadable":

```rust
pub fn read_records(dir: &Path) -> anyhow::Result<Vec<HistoryRecord>> {
    let path = history_path(dir);
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(serde_json::from_str(&text)
            .map_err(|e| anyhow::anyhow!("Could not read history at {}: {e}", path.display()))?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(anyhow::anyhow!(
            "Could not read history at {}: {e}", path.display())),
    }
}
```

### CR-03: When the CWD is `output_dir`, a bare `--from` is never adopted, so the fork's child loses its parent

> **FIXED** — commit `7a3c82b` — Same code path as CR-01 — fixed together.

**Files:** `src/history.rs:424-436`, `src/cli.rs:505-510`
**Issue:** For a bare name, `literal.parent()` is `Some("")`, which never equals
`Some(dir)`. So when the CWD *is* the output dir, the `recorded: true` adoption branch is
unreachable, `recorded: false` is returned, no record is written, and the edit proceeds with
a source whose basename matches nothing in the store. `record_result`'s parent lookup then
finds no match and records the new node as a **root** — the lineage the whole phase exists
to preserve is silently dropped.

Reproduced:

```
(cwd == output_dir)
resolve_fork_source(dir, "cat.png") -> File { path: "cat.png", recorded: false }
records written = 0
```

**Fix:** Compare canonical candidates, not the literal's parent:

```rust
let candidate = dir.join(from);           // bare name, no separators
if candidate.is_file() { adopt_root(dir, &candidate)?; return Ok(ForkSource::File { path: candidate, recorded: true }); }
```
and only use the CWD-relative literal branch when `from` contains a separator (see CR-01).

### CR-04: `load_view` recursion is unbounded — a deep chain aborts the process

> **FIXED** — commit `386dcbb`

**Files:** `src/history.rs:198-249` (call sites at `src/history.rs:179-188`), guard only in `src/tui.rs:52-54`
**Issue:** `build_node` recurses once per tree level. The `seen` set bounds *work*, not
*stack depth*. `MAX_HISTORY_DEPTH = 64` exists but lives in the TUI flattener
(`src/tui.rs:1932-1945`), which runs **after** `load_view` has already recursed — so the cap
protects nothing.

Measured, by spawning `load_view` on threads of a chosen stack size:

| stack | depth that survives | depth that aborts |
|---|---|---|
| 2 MB (tokio worker — every web handler) | 600 | ~900 |
| 8 MB (main thread — CLI, TUI) | 3500 | ~6000 |

At ~900 chained nodes, `GET /api/history` and `POST /api/fork` kill the server process with
`fatal runtime error: stack overflow`. A long single branch (900 sequential edits of one
image) is ordinary use, not an exotic edit. The sidecar is also explicitly documented as
hand-editable, and its tests hand-write records directly.

**Fix:** Derive iteratively with an explicit work stack, or carry a depth counter into
`build_node` and stop descending (mirroring `MAX_HISTORY_DEPTH`), so the cap applies to the
shared read path rather than only to the flattener.

### CR-05: Parent resolution by basename is ambiguous — the oldest record silently wins

> **FIXED** — commit `3c044eb`

**Files:** `src/history.rs:281-285`, `src/history.rs:415`
**Issue:** Both `record_result`'s parent lookup and `resolve_fork_source`'s step (2) use
`records.iter().find(|r| r.filename == …)`, i.e. the *first* (oldest) match. Two records can
legitimately share a filename — see CR-06, which makes it routine — and then lineage is
attached to the wrong node with no diagnostic.

Reproduced with records `[aaaa1111 (old), bbbb22222 (new)]` both named `cat.png`:

```
record_result(source = cat.png) -> parent = Some("aaaa1111")   // the stale record
node_id_for("cat.png")           -> Some("aaaa1111")
resolve_fork_source("cat.png")   -> Node { id: "aaaa1111", .. }
```

The fork is now recorded under the wrong branch, and the tree shows a fork that did not
happen where the operator thinks it did.

**Fix:** Prefer the newest match (`records.iter().rev().find(...)`), or make duplicate
filenames impossible at write time by appending a counter — which also fixes CR-06.

### CR-06: Two images generated in the same second collide on filename; one silently overwrites the other

> **FIXED** — commit `5f7750c` — Touches a pre-Phase-2 line, as the review directs; no Phase 1 test depended on the collision.

**File:** `src/core.rs:102-104`
**Issue:** The three sibling branches of the filename selection all call `suffix_numbered`
(`src/core.rs:72`, `87`, `99`, `101`) — the default branch does not:

```rust
} else {
    generate_filename(Some(&request.prompt), &output_ext)   // <-- no _1/_2 suffix
};
```

`generate_filename` derives the name from the prompt plus a **second-resolution** timestamp
(`src/utils.rs:97-99`), so with `n > 1` every iteration in the same second produces the
identical path. Reproduced with `n: 2`:

```
sidecar filename order = ["a_cat_20261010_022837.png", "a_cat_20261010_022837.png"]
png files on disk     = ["a_cat_20261010_022837.png"]        // 2 images generated, 1 file
saved_paths           = [both report the same path]
```

The first image is destroyed, both results report "✅ generated successfully", and the sidecar
records two nodes claiming one filename — which then feeds CR-05. (The line predates this
phase — `git log -L 102,104:src/core.rs` shows the initial port — but Phase 2's new store is
what turns a quiet overwrite into corrupt lineage, and the phase's own test
`tests/core.rs:333` covers only the `--output` path, leaving this one dark.)

**Fix:** `suffix_numbered(&generate_filename(Some(&request.prompt), &output_ext), i, request.n)`
plus a regression test with `n: 2` and no `--output`.

## Warnings

### WR-01: Non-atomic sidecar write plus a discarded error — and the module doc claims a recovery that does not exist

> **FIXED** — commit `7e20e2a`

**Files:** `src/history.rs:108-114`, `src/core.rs:152-167`, claim at `src/history.rs:9-12`
**Issue:** `append_records` does `fs::write(path, …)` — truncate-then-write in place. An
interrupted write leaves a truncated sidecar that then fails to parse, and every surface
errors out until an operator hand-repairs the file. The doc comment states "a failed write is
never fatal — lazy adoption (D-10) recovers the node on the next read", which is false for
this case: adoption only triggers for files found by name, and `read_records` returns `Err`
for a malformed store, so `resolve_fork_source` never gets to step (3).

Separately, `core.rs` discards the `record_result` result with `let _ =`, so a failed write
is invisible to the operator — the image saves, the command reports success, and no lineage
is recorded. There is also no locking (a documented D-11 decision), so a concurrent
CLI + TUI + web run against one `output_dir` loses records.

**Fix:** Write to a tempfile in the same directory and `rename` over the sidecar (atomic on
the same filesystem), and at minimum log a warning on the `record_result` failure path instead
of `let _ =`.

### WR-02: Prompt text is embedded raw into the row grammar — newlines and terminal escapes pass through

> **FIXED** — commit `d337700`

**Files:** `src/history.rs:302-312`, `src/history.rs:327-344`
**Issue:** `prompt_field` clamps to 32 *characters* but performs no control-character
sanitisation, and `history_row_text` interpolates it straight into the row. A prompt can
reach the store unsanitised from every surface: `--prompt` arguments
(`src/cli.rs:149`, `175`), the web payloads (`src/web.rs:93`, `src/web.rs:517-523` only checks
emptiness), and the TUI textarea. Reproduced:

```
prompt "line one\nline two" -> row = "abc12345  line one\nline two                 cat.png  …"
row contains a newline: true
prompt "\u{1b}[2J\u{1b}[Hpwned" -> row = "abc12345  \u{1b}[2J\u{1b}[Hpwned   …"
```

A newline breaks the fixed-width, one-row-per-node contract (the whole tree misaligns and a
phantom row appears); an escape sequence is executed by the terminal running
`imagai history`. Only the stdin prompt path strips control characters
(`src/cli.rs:216-235`).

**Fix:** Strip control characters in `prompt_field` (or once, at record time in
`record_result`), so the row grammar can never be broken by prompt content.

### WR-03: `/api/history` discloses the server's absolute path, and `/api/images/{name}` serves the sidecar

> **FIXED** — commit `45ee7c5`

**Files:** `src/web.rs:464-478`, `src/web.rs:793-809`, CORS at `src/web.rs:127-130`
**Issue:** Two disclosures, both reachable from any origin:

1. `view.output_dir` is the server's absolute filesystem path, returned verbatim.
2. `serve_image` reduces to the final component but serves *any* file, including the sidecar.
   `content_type_for` has no branch for `.imagai-history.json`, so it is served as
   `application/octet-stream`. Reproduced:

```
GET /api/images/.imagai-history.json
-> 200 [{"created":"…","filename":"cat.png","id":"1a2b3c4d","model":"dall-e-3",
         "parent":null,"prompt":"SECRET-PROMPT"}]
```

The CORS layer (`allow_origin(Any)`) is pre-existing, but these are new response surfaces
introduced by this phase.

**Fix:** Return `output_dir` as a relative name (or omit it); in `serve_image`, refuse names
that are not in the image extension set, or explicitly refuse `HISTORY_FILENAME`.

### WR-04: Same-second records fall back to uuid ordering, so sibling order is arbitrary

> **FIXED** — commit `4a2a4e2` — The suggested snippet was NOT applied — it left `id` ahead of the index, which cannot break the tie. See 02-REVIEW-FIX.md.

**File:** `src/history.rs:163-167`
**Issue:** Children sort ascending and roots descending by `(created, id)`. `created` has
second resolution, so any two records written in the same second — which CR-06 makes
routine — tie and are then ordered by a random uuid, contradicting the documented
"children oldest-first / roots newest-first". Reproduced with three records sharing one
timestamp written in sidecar order `aaaa, bbbb, cccc`:

```
roots order = ["cccc3333", "bbbb2222", "aaaa1111"]
```

**Fix:** Break ties on sidecar position (the array index), which *is* insertion order:

```rust
let mut indexed: Vec<(usize, &HistoryRecord)> = records.iter().enumerate().collect();
indexed.sort_by(|a, b| (b.1.created.as_str(), b.1.id.as_str(), b.0).cmp(&(a.1.created.as_str(), a.1.id.as_str(), a.0)));
```

### WR-05: A fork from an unrecorded file records its child as a root

> **PARTIAL** — Reproduced half fixed by CR-03 (`7a3c82b`) plus CR-05. Residual out-of-dir case is D-22-mandated (`parent: None` is correct there); the suggested `generate_image_core` signature change is a Phase 1 boundary — deferred to a follow-up phase.

**Files:** `src/cli.rs:505-510`, `src/history.rs:281-285`
**Issue:** `resolve_from` maps `ForkSource::File` to a bare path and `record_result` recovers
the parent only by matching the source *basename* against recorded filenames. For any source
that was not adopted — CR-03's `cwd == output_dir` case, or an out-of-dir file named
explicitly (D-22) — the lookup fails and the new node is written with `parent: None`. The
tree then shows a fresh root instead of a branch, and `print_fork_line` cannot name a node.

**Fix:** Have `record_result` accept the resolved node id directly (pass
`ForkSource::Node { id }` through `resolve_fork_source` → `cmd_fork` → `generate_image_core`),
rather than re-deriving it from a basename.

## Info

### IN-01: `history_tree_prints_connectors` asserts coverage it does not have

> **FIXED** — commit `f1ec03d`

**File:** `tests/cli.rs:753-755`
**Issue:** The comment reads "Three nodes are missing, so the plural warning line prints",
but no plural-warning assertion follows — only the singular one is exercised (in
`history_tree_marks_missing_nodes`). A reader concludes the plural wording is pinned here.
**Fix:** Add the plural assertion or delete the comment and the `let _ = out_path;` filler.

### IN-02: The TUI clones the full row list on every frame

> **SKIPPED** — Not a defect — a perf cost the review itself labels out of v1 scope, in a draw path with no test harness. Deferred.

**File:** `src/tui.rs:1136-1142`
**Issue:** `draw_history_tree` clones `state.rows` (each row holds a `Line<'static>` plus a
full `HistoryNode` clone) on every draw — O(history) allocation at ~10 fps. Out of v1 perf
scope, but it is a new cost this phase introduces.
**Fix:** Render in two passes like the rest of the file already does (borrow for the read,
`as_mut()` only for the height write-back).

### IN-03: `ForkPayload.prompt` is required, so a missing field bypasses the JSON error envelope

> **FIXED** — commit `3ed1423`

**File:** `src/web.rs:92-104`
**Issue:** `prompt: String` is a required field, so a body without it gets axum's plain-text
400 rather than the `{ "success": false, "error": … }` envelope every other handler in the
file produces (and that `02-04-PLAN.md` requires).
**Fix:** `#[serde(default)] prompt: String` plus the same empty check already at
`src/web.rs:517-523`.

### IN-04: Two implementations of the same filename reduction, with different rules

> **FIXED** — commit `9bb4a23` — Deduplication only — no reachable web input differed between the two implementations.

**Files:** `src/history.rs:133-139` (`node_path`), `src/web.rs:250-260` (`output_dir_file`)
**Issue:** Both reduce a client-supplied filename to its final component and join onto `dir`.
`node_path` also rejects an empty final component; `output_dir_file` does not. The
`HistoryNode` doc comment claims "every consumer goes through this reduction", which is not
true of `web.rs`.
**Fix:** Export `node_path` and have `output_dir_file` call it, keeping one guard.

### IN-05: Dead branch in the session clause

> **FIXED** — commit `e24f937`

**File:** `src/cli.rs:664-672`
**Issue:** `if sessions == 1 { "" } else { "s" }` sits inside the `else` arm of an outer
`if sessions == 1`, so it can never take the `""` arm.
**Fix:** `format!(", {sessions} session{s}", s = if sessions == 1 { "" } else { "s" })` at one
level only.

### IN-06: TUI selection is not clamped after a reload shrinks the row list

> **SKIPPED** — False positive. `load_history` is the only writer of `rows` and every reload route funnels through it; it already sets `selected: 0`. No reachable out-of-range state.

**File:** `src/tui.rs:1058-1067`, `src/tui.rs:1196-1201`
**Issue:** `load_history` resets `selected` to 0, but a reload triggered from
`drain_messages` after a generation can still leave `selected` past the end of a shorter row
list (e.g. the view's roots reorder). `rows.get(h.selected)` then returns `None`, so Enter
and the detail pane silently do nothing until the user moves.
**Fix:** `self.history.as_mut().map(|h| h.selected = h.selected.min(h.rows.len().saturating_sub(1)))`
at the end of `load_history`.

---

_Reviewed: 2026-10-09T22:45:22Z_
_Reviewer: the agent (gsd-code-reviewer)_
_Depth: standard_

---

_All 17 findings dispositioned 2026-10-10 — see [02-REVIEW-FIX.md](02-REVIEW-FIX.md)._
