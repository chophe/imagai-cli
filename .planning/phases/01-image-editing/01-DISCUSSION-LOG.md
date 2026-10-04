# Phase 1: Image Editing - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-10-04
**Phase:** 1-image-editing
**Areas discussed:** Edit command shape, Picking a past image, Multi-ref & unsupported engines, Where results land

---

## Gray Area Selection

| Option | Description | Selected |
|--------|-------------|----------|
| Edit command shape | New `imagai edit` subcommand vs `generate --image`; change-instruction vs re-prompt | |
| Picking a past image | How EDIT-02 resolves "past result" without a history index | |
| Multi-ref & unsupported | `--ref` flag shape; behavior when engine can't take image input | |
| Where results land | Same dir vs subdir; metadata/lineage in PNG | |

**User's choice:** "answer by yourself" — delegated all four areas to the agent
**Notes:** User was offered multi-select of the four gray areas and instead delegated the decisions.

---

## Edit command shape

| Option | Description | Selected |
|--------|-------------|----------|
| `imagai edit` subcommand | Parallel to `generate`, own `EditArgs` | ✓ (agent-chosen, user-approved) |
| `generate --image` flag | Overload the existing subcommand | |
| Prompt semantics | Change instruction vs full re-prompt | ✓ change instruction |

**User's choice:** Delegated to agent, then approved wholesale
**Notes:** Approved set recorded as D-01, D-02 in CONTEXT.md.

---

## Picking a past image

| Option | Description | Selected |
|--------|-------------|----------|
| Newest in output_dir | mtime heuristic, printed to user | ✓ (agent-chosen, user-approved) |
| Numbered index/picker only | requires an index (Phase 2 scope) | |
| Require explicit path always | fails EDIT-02's "without re-supplying path" | |

**User's choice:** Delegated to agent, then approved wholesale
**Notes:** Recorded as D-03. TUI/web get a newest-first picker over `output_dir`.

---

## Multi-ref & unsupported engines

| Option | Description | Selected |
|--------|-------------|----------|
| Repeatable `--ref` flag | clap `append`, web array, TUI list | ✓ (agent-chosen, user-approved) |
| Comma-separated single flag | less clap-idiomatic | |
| Fail fast on unsupported engine | `anyhow::bail!` naming the engine | ✓ (agent-chosen, user-approved) |
| Silent fallback to text-to-image | spends credits on the wrong request | rejected |

**User's choice:** Delegated to agent, then approved wholesale
**Notes:** Recorded as D-04, D-05.

---

## Where results land

| Option | Description | Selected |
|--------|-------------|----------|
| Same flat output_dir | keeps web gallery working | ✓ (agent-chosen, user-approved) |
| `edited/` subdir | splits gallery; needs web changes | |
| `-edit` filename suffix + metadata lineage | source stem + prompt/source in PNG tEXt | ✓ (agent-chosen, user-approved) |

**User's choice:** Delegated to agent, then approved wholesale
**Notes:** Recorded as D-06, D-07. D-06 rated costly (user-visible filename scheme).

---

## the agent's Discretion

- All four gray areas were delegated by the user ("answer by yourself")
- Remaining agent latitude: provider wire format for edits, DTO field naming, function decomposition, test strategy, TUI picker drawing

## Deferred Ideas

- Iteration history tree / fork / step-back — Phase 2
- Multi-vendor provider adapters — Phase 3
- Retries/backoff, cancellation, video — v2
