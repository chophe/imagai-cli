# Walking Skeleton — imagai (image editing)

**Phase:** 1
**Generated:** 2026-10-06

## Capability Proven End-to-End

> A user names an image on disk and a change instruction on the command line, runs one
> command, and gets a new edited image written into `output_dir` with its source recorded
> in the file — exercising the full stack: clap routing → shared request DTO → application
> core → provider transport → persistence.

Concretely, the skeleton command is:

```
imagai edit --image generated_images/sunset_20261004_120000.png -p "make it sunset"
```

which must (1) resolve the engine, (2) gate on that engine's ability to take image input,
(3) build a `multipart/form-data` body against the configured engine, (4) save the returned
image as `<source-stem>-edit.<ext>` in the same flat `output_dir`, and (5) record
`Prompt`, `Model` and `Source` in the PNG's `tEXt` chunks.

## Architectural Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Framework / language | Rust 2021, single crate `imagai`, one binary + one lib | Already shipped and validated; `PROJECT.md` Constraints forbid restructuring |
| Frontend topology | Three thin frontends (`src/cli.rs`, `src/tui.rs`, `src/web.rs`) converging on `src/core.rs::generate_image_core` | Existing modular-monolith shape; guarantees surface parity by construction (`PROJECT.md` Key Decisions) |
| Data layer (request) | One normalized DTO `ImageGenerationRequest` in `src/models.rs`, with `source_image: Option<String>` and `ref_images: Vec<String>` added as **typed fields** | Fixes the documented `extra_params` duplication anti-pattern: edit fields are not re-assembled per frontend |
| Data layer (persistence) | Flat `output_dir`; no new store, no database, no sidecar index | D-06 keeps the web gallery and `list_images` working unchanged; iteration history is Phase 2's job |
| Auth | None — provider keys arrive as bearer tokens from `IMAGAI__ENGINES__<NAME>__API_KEY`; web stays localhost-only | `PROJECT.md` Out of Scope: no web authentication |
| Deployment target | Single local binary. No hosted service. | `PROJECT.md` Out of Scope: hosted/multi-user web service excluded |
| Provider transport | Hand-rolled `reqwest` + `rustls` in `src/provider.rs`, one function per wire shape (`generate_images`, `generate_edits`, `openrouter_chat_generate`, `chat_completion`, `fetch_models`) | Existing; Phase 3's vendor adapters swap in at exactly this seam |
| Engine capability model | One gate function `provider::edit_transport(name, cfg) -> anyhow::Result<EditTransport>` that decides the wire shape before any network call and **bails naming the engine** | D-05: spending provider credits on a request the engine cannot serve is worse than an error |
| Directory layout | Flat sibling modules in `src/`; no `src/` subdirectories, no logging framework, no config-file parser | `src/lib.rs` structure + `STRUCTURE.md` "Do not create" |
| Web UI delivery | Single embedded `web_interface.html` via `include_str!`; no bundler, no build step | `PROJECT.md` Constraints |
| Test strategy | In-process axum `MockServer` (`tests/common/mod.rs`) capturing raw request `Bytes`, driven by `assert_cmd` (CLI), in-process library calls (`tests/core.rs`) and `tower::oneshot` (`tests/web.rs`) | Existing harness; raw-bytes capture is what makes multipart wire assertions possible without a multipart parser |
| TUI state model | Per-tab row constants and per-tab focus/editing state inside the single `App` struct | The four lockstep `match` blocks in `src/tui.rs` end in `unreachable!()`; extending the Generate tab's row range is the documented panic risk |

## Stack Touched in Phase 1

- [x] **Project scaffold** — Cargo single crate, `[[bin]] imagai`. The only scaffold change is `Cargo.toml`: the `multipart` feature added to the existing locked `reqwest` dependency. That toggle vendors two new transitive crates (`mime_guess` 2.0.5, `unicase` 2.10.0) and rewrites `Cargo.lock`; both are decided on at the wave-0 gate (plan `01-00`) before the toggle runs. No *direct* dependency is added.
- [x] **Routing** — one real new entry point: the `Edit` variant of the clap `Commands` enum in `src/cli.rs`, wired through `cli::run`. `generate`, `list-engines`, `tui` and `web` are untouched.
- [x] **Data layer** — one real read (`std::fs::read` of the source image bytes plus a `read_dir` scan of `output_dir`) and one real write (`output_dir/<source-stem>-edit.<ext>` with PNG `tEXt` lineage appended before `IEND`).
- [x] **UI** — one interactive element wired end-to-end: the `imagai edit` flag set (`--image`, `--ref`, `--engine`, `-p`, `-o`, `--verbose`) driven from a terminal into the same core the other two surfaces use.
- [x] **Deployment** — local full-stack run command, documented in `01-01-PLAN.md` § Verification: `cargo run -- edit --image <path> -p "<change>"` against a configured engine, and `cargo test` against the in-process mock for the automated path.

## Out of Scope (Deferred to Later Slices)

> Anything that is *not* in the skeleton. This list prevents future phases from re-litigating
> the skeleton's minimalism.

- Multiple reference images on the CLI and both transports — plan `01-01` Task 3 (same phase)
- Newest-image auto-pick when `--image` is omitted — plan `01-02` (same phase)
- Web `POST /api/edit` + browser edit form and source picker — plan `01-03` (same phase)
- TUI Edit tab + source picker overlay — plan `01-04` (same phase)
- Iteration history tree, fork, step-back, cross-surface parity for history — Phase 2 (`BRCH-01..04`)
- Named non-OpenAI-compatible provider adapters — Phase 3 (`PRVD-01`)
- Non-zero exit on generation failure, single-binary install docs — Phase 4 (`ROB-01`, `DIST-01`)
- Retry/backoff, cancellation, progress reporting — v2 (`ROB-02`, `ROB-03`)
- Video generation — v2 (`VID-01`)
- Fixing the pre-existing `truncate()` byte-slice panic, `sanitize_filename` non-ASCII panic, exit-0-on-failure, `/api/generate-cli` shell allow-list, and the dead `input_image` upload widget — all recorded in `.planning/codebase/CONCERNS.md` and out of this phase. New edit code must not route user-supplied paths through the known-buggy helpers, but it must not fix them either.

## Subsequent Slice Plan

Each later slice adds one vertical capability on top of this skeleton without altering its
architectural decisions:

- **Plan 01-00 (Wave 0)** — the phase's supply-chain gate: a `checkpoint:decision` carrying `gate="blocking-human"`, where a human decides whether Phase 1 may vendor `mime_guess` 2.0.5 and `unicase` 2.10.0. Every plan below depends on it transitively, so nothing resolves until the decision is made. `01-01` Task 1 additionally carries a `<precondition>` asserting the decision was recorded.
- **Plan 01-01 (Wave 1)** — CLI edit from an explicit path: DTO fields, capability gate, multipart
  `/images/edits`, chat-vision edit path, `-edit` filename, PNG lineage, mock raw-bytes capture.
- **Plan 01-01 Task 3** — multiple reference images end-to-end on the CLI and both transports.
- **Plan 01-02 (Wave 2)** — `list_output_images` as the single "which image?" helper; CLI newest-mtime
  auto-pick; `src/web.rs::list_images` refactored onto the shared helper.
- **Plan 01-03 (Wave 3)** — `POST /api/edit` + `EditPayload` + filename-only path-traversal guard +
  browser edit form with a source/refs picker fed by `GET /api/images`.
- **Plan 01-04 (Wave 3)** — TUI Edit tab with its own row constants, tab-scoped state, and a source
  picker overlay that consumes arrow keys before focus movement.
- **Phase 2** — history tree over the `Source` lineage already recorded in the PNGs.
- **Phase 3** — vendor adapters replacing the substring routing inside `provider::edit_transport`.
- **Phase 4** — exit codes and distribution docs.