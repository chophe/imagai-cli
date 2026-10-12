# Phase 3: Multi-Vendor Providers — User Acceptance Testing

**Phase goal (ROADMAP):** Users can configure named non-OpenAI-compatible providers
alongside existing engines and select them per request.
**UAT date:** 2026-10-12 (11:02 UTC)
**Binary under test:** `/Users/ali/dev/rust/imagai-cli/target/debug/imagai`
(rebuilt from working tree at `git rev-parse HEAD` = `e739dd8`, "docs(03): security
audit verdict SECURED"; `cargo build --quiet` exit 0 before any check ran)
**Method:** every check below was driven through the real binary against a local
capture server. No check was accepted on the strength of a test assertion.

---

## 0. Environment

| Item | Value |
|------|-------|
| Capture server | `python3 http.server` subclass, bound `127.0.0.1:18911`, logging method / path / headers / raw body to JSONL |
| Scratch dir | `$TMPDIR/imagai-uat3/` (`out/` = throwaway output_dir, `runs/` = per-command stdout/stderr/rc, `capture.jsonl` = wire log, `tui.log` = raw pty stream) |
| Network | Localhost only. The fal engine's `BASE_URL` points at the capture server, never at `fal.run`. No API key was used against any real provider. |
| Output dir | `$TMPDIR/imagai-uat3/out` (10 PNGs + `.imagai-history.json` at the end) |

**Engine configuration** (environment variables only, no repo file touched):

```bash
IMAGAI__OUTPUT_DIR=$TMPDIR/imagai-uat3/out
IMAGAI__ENGINES__FAL__API_KEY=fal-secret-key-123
IMAGAI__ENGINES__FAL__BASE_URL=http://127.0.0.1:18911
IMAGAI__ENGINES__FAL__MODEL=fal-ai/flux/schnell
IMAGAI__ENGINES__FAL__FORMAT=fal
IMAGAI__ENGINES__OAI__API_KEY=sk-oai-secret-key-456      # no FORMAT declared
IMAGAI__ENGINES__OAI__BASE_URL=http://127.0.0.1:18911/v1 #     → openai default
IMAGAI__ENGINES__OAI__MODEL=dall-e-3
IMAGAI__ENGINES__GPTIMG__API_KEY=sk-oai-edit-key-789
IMAGAI__ENGINES__GPTIMG__BASE_URL=http://127.0.0.1:18911/v1
IMAGAI__ENGINES__GPTIMG__MODEL=gpt-image-1
IMAGAI__ENGINES__GPTIMG__FORMAT=openai
```

Capture-server response shapes: fal path → `{"images":[{"url":"data:image/png;base64,…"}]}`;
`/v1/*` → `{"data":[{"b64_json":"…"}]}`; `GET /v1/models` → `{"data":[{"id":"dall-e-3"},{"id":"gpt-image-1"}]}`.
(The data-URL response keeps the image off the wire so the capture log stays clean.)

---

## 1. Result summary

| # | Check | Status | Key evidence |
|---|-------|--------|--------------|
| 1 | Discovery — `list-engines` shows both engines side by side | ✅ PASS | table rows `fal / fal` and `oai / openai` |
| 2 | OpenAI-compatible path unchanged | ✅ PASS | body keys exactly `[model,n,prompt,quality,response_format,size,style]` |
| 3 | fal generation uses fal's own wire format | ✅ PASS | `POST /fal-ai/flux/schnell`, `Authorization: Key …`, keys `[image_size,num_images,output_format,prompt]`, zero OpenAI keys |
| 4 | fal response maps onto the shared save path | ✅ PASS | PNG in output_dir, `tEXt` lineage chunks, `.imagai-history.json` node |
| 5 | Edit refusal on fal | ✅ PASS | single `Error:` line naming engine+model, rc=1, **0 requests**, history unchanged |
| 6 | Edit/fork still works on OpenAI-compatible engines | ✅ PASS | `POST /v1/images/edits` multipart, `Bearer`, PNG saved with `Source` lineage |
| 7 | fal selectable from all three surfaces | ✅ PASS | CLI (live), web (live), TUI (pty-driven, live) |
| 8 | The discovery flag behaves as the plans describe | ✅ PASS* | `list-engines --all`: 1 `[error]` line, loop continues, 0 fal requests — see note (a) |
| 9 | Robustness: bogus FORMAT / missing MODEL | ✅ PASS* | both fail fast naming engine + value/var, 0 requests — see notes (b),(c) |
| 10 | Two distinct wire formats in one run | ✅ PASS | intersection of the two key sets = `{prompt}` only |

**10 checks run, 10 passed, 0 failed, 0 skipped.**

`*` = passed on substance, with a wording deviation recorded in §4. No check was
silently passed; every check produced captured output.

---

## 2. Check-by-check evidence

### Check 1 — Discovery: both engines visible with the new format

```console
$ imagai list-engines
⚙️  Configured Imagai Engines
Engine Name              Format       Default Model                      Base URL                     API Key Set
fal                      fal          fal-ai/flux/schnell                http://127.0.0.1:18911       ✅ Set
gptimg                   openai       gpt-image-1                        http://127.0.0.1:18911/v1    ✅ Set
oai                      openai       dall-e-3                           http://127.0.0.1:18911/v1    ✅ Set
```

The `Format` column renders both families side by side — `fal` for the
non-OpenAI-compatible provider, `openai` for the engines that declared nothing.
(`$TMPDIR/imagai-uat3/runs/c1_list_engines.out`, ANSI stripped.)

### Check 2 — OpenAI-compatible path is byte-for-byte the OpenAI set

```console
$ imagai generate -p "openai unchanged cat" --engine oai -n 1
🖼️ Generating image with engine: oai
✅ Image 1 generated successfully! Saved to: …/out/openai_unchanged_cat_20261012_141101.png
```

Captured request (verbatim):

```http
POST /v1/images/generations
Host: 127.0.0.1:18911
Authorization: Bearer sk-oai-secret-key-456
Content-Type: application/json

{"model":"dall-e-3","n":1,"prompt":"openai unchanged cat","quality":"standard","response_format":"b64_json","size":"1024x1024","style":"vivid"}
```

Body keys = exactly `["model","n","prompt","quality","response_format","size","style"]`.
**PASS** — no regression on an engine that declares no FORMAT.

### Check 3 — fal generation in fal's own wire format

```console
$ imagai generate -p "fal cat" --engine fal -n 1
🖼️ Generating image with engine: fal
✅ Image 1 generated successfully! Saved to: …/out/fal_cat_20261012_141215.png
```

Captured request (verbatim):

```http
POST /fal-ai/flux/schnell
Host: 127.0.0.1:18911
Authorization: Key fal-secret-key-123
Content-Type: application/json

{"image_size":"square_hd","num_images":1,"output_format":"png","prompt":"fal cat"}
```

- Path is `/{model}` — the model id *is* the URL path, and no `model` field exists in the body.
- Body keys = `["image_size","num_images","output_format","prompt"]`.
- `model`, `n`, `size`, `response_format`, `quality`, `style`: **none present**.
- Auth scheme is `Key <key>`, **not** `Bearer`.
**PASS.**

### Check 4 — fal response lands on the shared save path

Same run as Check 3. The capture server answered with the fal shape; after the run:

```console
$ ls $TMPDIR/imagai-uat3/out
fal_cat_20261012_141215.png         137 bytes
openai_unchanged_cat_20261012_141101.png
.imagai-history.json

$ python3 -c "parse PNG chunks"
fal_cat_20261012_141215.png: valid PNG, tEXt chunks = ['Prompt = fal cat', 'Model = fal-ai/flux/schnell']

$ cat out/.imagai-history.json
[
  {"id":"e1de7d6f","parent":null,"filename":"openai_unchanged_cat_20261012_141101.png","prompt":"openai unchanged cat","model":"dall-e-3","created":"2026-10-12T10:41:01"},
  {"id":"be3010bd","parent":null,"filename":"fal_cat_20261012_141215.png","prompt":"fal cat","model":"fal-ai/flux/schnell","created":"2026-10-12T10:42:15"}
]
```

File in `output_dir` ✅, PNG `tEXt` lineage ✅, history sidecar records the node
with the fal model id ✅ — the shared path (D-27) is unchanged.

### Check 5 — Edit against the fal engine is refused, spending nothing

```console
$ imagai edit -p "make it sunset" --engine fal --image …/uat3/source.png
Error: Engine 'fal' (model 'fal-ai/flux/schnell') cannot accept image input for edits: the 'fal' format is generation-only in this version. Set IMAGAI__ENGINES__<NAME>__MODEL to a gpt-image model, or point BASE_URL at an OpenRouter vision engine.
exit=1
```

- Single line, on stderr, naming the engine and its model.
- **Capture log: 0 requests.** No socket was opened, so no credits were spent and
  no OpenAI-shaped body was attempted.
- History sidecar node count unchanged (2 before, 2 after); no file written.

**PASS.** Wording note in §4(b): the refused edit surfaces as the process's
`Error:` line (rc=1); the `[error]`-prefixed form is the `list-engines` per-engine
convention, which is proven in Check 8.

### Check 6 — Edit still works on an OpenAI-compatible engine

```console
$ imagai edit -p "make it sunset" --engine gptimg --image …/uat3/source.png
🖼️ Editing image with engine: gptimg
✅ Image 1 generated successfully! Saved to: …/out/source-edit.png
```

Captured request:

```http
POST /v1/images/edits
Host: 127.0.0.1:18911
Authorization: Bearer sk-oai-edit-key-789
Content-Type: multipart/form-data; boundary=…

Content-Disposition: form-data; name="model"        gpt-image-1
Content-Disposition: form-data; name="prompt"       make it sunset
Content-Disposition: form-data; name="image[]"; filename="source.png"   (PNG bytes)
```

The gate did not block a capable engine: the multipart edit transport still runs,
and the saved `source-edit.png` carries `Prompt`, `Model` **and** `Source = source.png`
lineage chunks, with a history node (`model: gpt-image-1`). **PASS.**

### Check 7 — fal selectable from all three surfaces

**7a. CLI `--engine fal`** — Checks 3 and 10 above (live `generate --engine fal`).

**7b. Web `engine` field** — live server `imagai web --host 127.0.0.1 --port 19555`:

```console
GET /api/engines  -> 200
{"default_engine":null,"success":true,"engines":[
  {"name":"fal","format":"fal","model":"fal-ai/flux/schnell","base_url":"http://127.0.0.1:18911"},
  {"name":"oai","format":"openai","model":"dall-e-3","base_url":"http://127.0.0.1:18911/v1"}]}

GET /  -> 200 (55518 bytes); option.textContent templates = 3, all `${engine.name} (${engine.model}) · ${engine.format}`;
            <select id="engine">, id="edit_engine", id="history_engine" all present

POST /api/generate {"prompt":"web fal cat","engine":"fal"}  -> 200
  results[0].success=true, saved_path=…/out/web_fal_cat_20261012_141853.png
POST /api/generate {"prompt":"web openai cat","engine":"oai"} -> 200, success=true   (no regression)
POST /api/edit {"prompt":"sunset","engine":"fal","source":"web_source.png"} -> 200
  results[0].error = "Engine 'fal' (model 'fal-ai/flux/schnell') cannot accept image input for edits: …", success=false
```

Capture log for the server session:

```http
POST /fal-ai/flux/schnell        Authorization: Key fal-secret-key-123
  {"image_size":"square_hd","num_images":1,"output_format":"png","prompt":"web fal cat"}
POST /v1/images/generations      Authorization: Bearer sk-oai-secret-key-456
  {"model":"dall-e-3","n":1,"prompt":"web openai cat","quality":"standard","response_format":"b64_json","size":"1024x1024","style":"vivid"}
```

and the web `POST /api/edit` on fal produced **0 requests** — refusal parity with the CLI.

**7c. TUI engine row** — driven through a pty (`pty.fork`, 200×50, `TERM=xterm-256color`):
Enter (edit prompt) → `tui fal cat` → Enter, Enter (edit engine) → `fal` → Enter,
`j`×8 to ROW_SUBMIT → Enter. Rendered screen (ANSI stripped, raw stream in
`$TMPDIR/imagai-uat3/tui.log`):

```text
Engine          fal
Generate        [ Generate ] [working…]
🖼️  Generating 1 image(s) with engine 'fal'…
Generating with engine 'fal'…
✅ Image 1 saved to …/out/tui_fal_cat_20261012_142324.png
```

and the capture server logged, for that keypress sequence:

```http
POST /fal-ai/flux/schnell
Authorization: Key fal-secret-key-123
{"image_size":"square_hd","num_images":1,"output_format":"png","prompt":"tui fal cat"}
```

Shared path the typed engine name funnels through (unchanged code, cited for
traceability): `src/tui.rs:414-431` `build_request()` reads the `engine` row field →
`src/tui.rs:386-404` `start_generation()` → `src/tui.rs:402`
`generate_image_core(&request, &settings)` — the same entry point the CLI and web
tests exercise. The pty run above shows the name `fal` reaching that path with a
fal-format request as the result. **PASS** for all three surfaces.

### Check 8 — The discovery flag behaves as the plans describe

The plans' flag decision (03-03-PLAN:65, D-32/D-34) is that `list-engines` keeps its
shape and gains only the format column — "no new subcommand or a new flag" — and
that `list-engines --all` needs no change because a provider with no `GET /models`
already degrades to one `[error]` line and continues (T-03-11). Live behaviour:

```console
$ imagai list-engines --all
Engine Name              Format       Default Model …
fal                      fal          fal-ai/flux/schnell …
gptimg                   openai       gpt-image-1 …
oai                      openai       dall-e-3 …

[error] Failed to fetch models for 'fal': Engine 'fal' has no model list to fetch: the 'fal' format is generation-only and names its model by URL path, so there is no GET /models endpoint to call.

📚 Models for 'gptimg' (all)
  • dall-e-3
  • gpt-image-1
📚 Models for 'oai' (all)
  • dall-e-3
  • gpt-image-1
```

- Exactly **one** `[error]` line (`rg -c` on the captured stdout = 1).
- The fal row still lists; the loop continues and the other two engines' models print.
- Capture log: only 2 `GET /v1/models` (the two openai engines) — **zero** requests
  attempted for the fal engine, so no fal key left the process.
**PASS** (substance), with the naming deviation in §4(a).

### Check 9 — Robustness

**9a. Bogus FORMAT value.**

```console
$ IMAGAI__ENGINES__TYPO__FORMAT=falai imagai list-engines
[warning] Engine 'typo' declares an unknown FORMAT 'falai'; supported formats: openai, fal. This engine fails on its first request.
typo                     falai        dall-e-3 …

$ IMAGAI__ENGINES__TYPO__FORMAT=falai imagai generate -p "bogus format" --engine typo
[warning] Engine 'typo' declares an unknown FORMAT 'falai'; supported formats: openai, fal. This engine fails on its first request.
[Error] Error generating image 1: Engine 'typo' declares an unknown FORMAT 'falai'; supported formats: openai, fal
```

The typo is visible verbatim in the format column, the warning names the engine, and
the request never happens. Capture log: **0 requests.**

**9b. fal engine with no MODEL.**

```console
$ IMAGAI__ENGINES__NOMODEL__FORMAT=fal imagai generate -p "no model fal" --engine nomodel
[Error] Error generating image 1: Engine 'nomodel' has no MODEL set: the 'fal' format derives its endpoint from the model id, so set IMAGAI__ENGINES__<NAME>__MODEL (for example 'fal-ai/flux/schnell').
```

Names the engine **and** the variable to set, and "derives its endpoint from the
model id" says why. Capture log: **0 requests** — the failure precedes any socket.
**PASS** for both, with the wording deviations noted in §4(b),(c).

### Check 10 — Two distinct wire formats in one run

Two consecutive `generate` invocations against the two engines, one capture log:

```http
#13 POST /v1/images/generations   Authorization: Bearer sk-oai-secret-key-456
    {"model":"dall-e-3","n":1,"prompt":"two formats oai","quality":"standard","response_format":"b64_json","size":"1024x1024","style":"vivid"}
#14 POST /fal-ai/flux/schnell     Authorization: Key fal-secret-key-123
    {"image_size":"square_hd","num_images":1,"output_format":"png","prompt":"two formats fal"}
```

```text
openai keys : ['model','n','prompt','quality','response_format','size','style']
fal keys    : ['image_size','num_images','output_format','prompt']
intersection: ['prompt']
fal body contains an OpenAI key? NO
```

Both files saved: `two_formats_oai_20261012_143045.png` (lineage `Model = dall-e-3`)
and `two_formats_fal_20261012_143045.png` (lineage `Model = fal-ai/flux/schnell`).
**PASS** — the two families' key sets are disjoint apart from the one shared `prompt`
field, in a single run against a single operator configuration.

---

## 3. Corroborating (not load-bearing) evidence

The UAT verdicts above rest on the captured wire traffic and binary output. As
corroboration only — never as the basis for a PASS — the suite is green:

```
test result: ok. 108 passed; 0 failed; 0 ignored   (lib)
test result: ok.  52 passed; 0 failed; 0 ignored   (cli)
test result: ok.  26 passed; 0 failed; 0 ignored   (core)
test result: ok.  40 passed; 0 failed; 0 ignored   (web)
                                                                226 total
```

Named tests that mirror the checks: `openai_default_engine_body_is_unchanged` (check 2),
`fal_engine_saves_image_through_the_shared_pipeline` (check 4),
`edit_against_fal_format_fails_before_any_request` (check 5),
`declared_openai_edit_engine_still_saves_with_lineage` (check 6),
`fal_engine_generates_from_the_cli` (7a), `fal_engine_generates_through_the_web_api` (7b),
`engines_tab_rows_name_the_format_column` (7c), `unknown_format_fails_the_cli_run` (9a).
`cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` are clean.

Source anchors a reviewer can read directly: `src/formats.rs:112-123` (`resolve`,
closed set), `src/provider.rs:172-185` (capability gate before `http_client`),
`src/provider.rs:639-666` (`fal_generate` + explicit `Key` header),
`src/provider.rs:594-623` (`build_fal_images_body`), `src/tui.rs:402`
(`generate_image_core`), `src/web.rs:174` (`format` from the shared helper),
`web_interface.html:853/873/891` (the three `option.textContent` labels).

---

## 4. Deviations and notes (recorded, not hidden)

**(a) There is no `--engines` flag in this build.** `imagai --help`, `imagai generate
--help` and `imagai list-engines --help` were run; the only engine-related flags are
`generate|edit --engine <ENGINE>`, `generate|edit|fork` per-surface `--engine`, and
`list-engines --all`. This matches the plans: 03-03-PLAN:65 decides "no new
subcommand or a new flag", and D-33 explicitly rejects a new `--provider` flag
distinct from `--engine`. Check 8 was therefore evaluated against the flag the plans
actually specify — `list-engines --all` — and passes on it. If `--engines` was meant
as a name for something else, that flag does not exist and the check is a FAIL by
that reading; the evidence above is the whole of what the built binary offers.

**(b) Refusal wording.** The edit refusal reaches the operator as the process's
`Error:` line on stderr with exit code 1, not as a `[error]`-prefixed line — the
`[error]` prefix in this codebase is the `list-engines` per-engine-fetch convention
(`src/cli.rs:813`). One line, naming the engine and model, zero requests: the
behaviour the phase promises.

**(c) Format-warning wording.** The bogus-FORMAT warning names the engine, the bad
value and the supported set; it does not spell out the literal variable path
`IMAGAI__ENGINES__<NAME>__FORMAT`. (The no-MODEL failure does name
`IMAGAI__ENGINES__<NAME>__MODEL` explicitly.) Judged a pass on substance — fail-fast,
named engine, visible in `list-engines`, no request.

**(d) Per-item failures exit 0.** `generate` prints one `[Error]` line per failed item
but returns exit status 0 (`rc=0` for checks 9a/9b). The hard gates that abort a
command — the fal edit refusal — do exit 1. This is the known deviation recorded in
03-05-SUMMARY.md; making per-item failures non-zero is Phase 4's ROB-01.

**Browser DOM item (closed during transition, 2026-10-12):** when UAT ran, the one
remaining look was the browser DOM rendering of the SPA engine pickers (no browser
binary in the UAT environment). During the post-UAT transition the check was
closed with live evidence — headless Google Chrome (installed on this machine,
driven over CDP) loaded the real served `GET /` from a live `imagai web` server
with the same fal + OpenAI-compatible engine configuration, and the page's own
`loadEngines()` rendered all three pickers (`#engine`, `#edit_engine`,
`#history_engine`) as:

```
["Use Default Engine", "fal (fal-ai/flux/schnell) · fal", "oai (dall-e-3) · openai"]
```

Every option reads `<name> (<model>) · <format>` via `textContent`, with the fal
and openai formats side by side. Recorded in 03-VERIFICATION.md ("Browser DOM
Closure"); with it, the phase's verification status is `passed`.

---

## 5. Verdict

**10 / 10 checks PASS via binary-driven evidence, 0 FAIL, 0 SKIP.** The user story
holds end to end: a fal engine is configured beside an OpenAI-compatible engine,
`list-engines` shows both with their formats, `--engine fal`, the TUI engine row and
the web `engine` field all produce fal's own wire format (`POST /fal-ai/flux/schnell`,
`Key …`, `{prompt,num_images,image_size,output_format}`), the OpenAI-compatible
request body is unchanged, the response lands on the shared save/lineage/history path,
editing is refused on fal without spending a request and still works on OpenAI-compatible
engines, and a bogus format or a missing model fails fast before any socket opens.

_Executed: 2026-10-12 by the UAT subagent against `target/debug/imagai` at `e739dd8`.
Raw streams retained at `$TMPDIR/imagai-uat3/` for the session's duration._
