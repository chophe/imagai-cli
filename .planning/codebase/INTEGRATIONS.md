---
last_mapped_commit: f37fbe5c1e16138cf0fbfaf77157b2e28b0f2a9c
last_mapped_at: 2026-10-03
---
# External Integrations

**Analysis Date:** 2026-10-03

## APIs & External Services

All outbound calls go through one shared helper: `provider::http_client()` in `src/provider.rs:12-17`, with URL resolution in `resolve_url()` (`src/provider.rs:71-80`) — **if an engine has no `BASE_URL`, the default is `https://api.openai.com/v1`**.

**Image generation (OpenAI-compatible images API):**

- Endpoint: `POST {base_url}/images/generations` — `src/provider.rs:154-172`
- Used for DALL-E 3, GPT-image, Imagen-style, and any OpenAI-compatible gateway
- Auth: `Authorization: Bearer <IMAGAI__ENGINES__<NAME>__API_KEY>`
- Request body built in `build_images_body()` (`src/provider.rs:243-291`):
  - `dall-e-3` models get `quality` + `style`
  - models containing `stability` drop `n`/`response_format` and pass Stability extras (`negative_prompt`, `seed`, `strength`, `output_format`, `aspect_ratio`, `mode`), inject `mode: text-to-image` for non-`sd3`, and drop `size` when `aspect_ratio` is set
- Response: `ImagesResponse` (`src/models.rs:57-73`) — accepts `url` or `b64_json`, plus optional `usage` and `estimated_cost` passthrough

**Chat completions (OpenAI-compatible):**

- Endpoint: `POST {base_url}/chat/completions` — generic helper `chat_completion()` at `src/provider.rs:83-116`
- Used for: LLM auto-filenames (`--auto-filename`, model fallback `gpt-4.1-mini`, max_tokens 20, temp 0.2 — `src/utils.rs:85-171`)

**OpenRouter (chat-based vision / image models):**

- Detected by base URL substring `openrouter.ai` + model containing `gemini` — `src/provider.rs:128-143`
- Endpoint: `POST {base_url}/chat/completions` with content array (`text` + optional `image_url`), `modalities: ["image","text"]` for image models — `src/provider.rs:293-335`
- Optional ranking headers from env: `HTTP-Referer` ← `OPENROUTER_HTTP_REFERER`, `X-Title` ← `OPENROUTER_X_TITLE` (`src/provider.rs:303-308`)

**Model discovery:**

- Endpoint: `GET {base_url}/models` (default `https://api.openai.com/v1/models`) — `src/provider.rs:20-69`
- Called by `imagai list-engines --all` (`src/cli.rs:328`) and the TUI engine browser (`src/tui.rs:768-770`)
- Tolerant parser: structured `ModelsListResponse` first, then lenient `{"data":[...]}` / bare-list fallback

**Image download:**

- `GET <image url>` when the provider returns `url` instead of `b64_json` — `save_image_from_url()` at `src/utils.rs:176-192`

**No other SaaS integrations:** no database, no object storage, no queue, no analytics, no telemetry, no payment, no third-party auth SDK.

## Data Storage

**Databases:**

- Not applicable — none. No SQL/NoSQL/ORM crate in `Cargo.toml`.

**File Storage:**

- Local filesystem only: `IMAGAI__OUTPUT_DIR` (default `generated_images/`), created on startup at `src/config.rs:89-91`
- PNG metadata written directly into the byte stream as `tEXt` chunks (`Prompt`, `Model`) — hand-rolled PNG chunk writer using `crc32fast` in `src/utils.rs:234-265`
- Image listing/serving reads that directory: `src/web.rs:327-398` (extension allowlist `png/jpg/jpeg/webp`, path-traversal-guarded)

**Caching:**

- None — no HTTP cache, no in-memory memoization across requests; a new `reqwest::Client` is built per operation in `provider.rs`.

## Authentication & Identity

**Auth Provider:**

- None / custom — the app has no user accounts and no session auth.
- Outbound: per-engine API key sent as `Authorization: Bearer …` on every request (`src/provider.rs:26, 102, 168, 337`). Keys come from env (`IMAGAI__ENGINES__<NAME>__API_KEY`), never from a secrets manager.
- Placeholder detection: `YOUR_OPENAI_API_KEY` counts as unconfigured (`src/config.rs:5, 17-19`).
- Inbound (the `imagai web` server): **no authentication at all** — every `/api/*` route is open, and CORS is `allow_origin(Any).allow_methods(Any).allow_headers(Any)` (`src/web.rs:92-105`). Bind to localhost (`--host 127.0.0.1`) when exposing it.

## Monitoring & Observability

**Error Tracking:**

- None (no Sentry/OTel/metrics). Errors surface as `anyhow` messages returned to the CLI or as `{"error": …}` JSON from the API (`src/provider.rs:192-199`, `src/web.rs`).

**Logs:**

- Plain `println!`/`eprintln!` + `colored` styling. `--verbose` dumps the exact API request body (`src/provider.rs:157-164, 328-335`).
- Usage/cost reporting rendered from provider-supplied `usage` / `estimated_cost` (`print_usage_cost()` at `src/cli.rs:370`).

## CI/CD & Deployment

**Hosting:**

- Local/desktop use — `cargo build --release` produces one stripped binary. No Dockerfile, no deploy manifests.

**CI Pipeline:**

- Not detected — no `.github/`, no workflow files, no release automation.
- Remote: `https://github.com/chophe/imagai-cli` (`git remote -v`).

## Environment Configuration

**Required env vars:**

- `IMAGAI__DEFAULT_ENGINE` — default engine name
- `IMAGAI__ENGINES__<NAME>__API_KEY` — one per engine (required for real calls)
- `IMAGAI__ENGINES__<NAME>__BASE_URL` — optional; defaults to `https://api.openai.com/v1`
- `IMAGAI__ENGINES__<NAME>__MODEL` — optional; image fallback `dall-e-3`, filename LLM fallback `gpt-4.1-mini`
- `IMAGAI__OUTPUT_DIR` — optional, default `generated_images`
- `OPENROUTER_HTTP_REFERER`, `OPENROUTER_X_TITLE` — optional OpenRouter headers
- CLI flags (`--host`, `--port`) override the web bind address; defaults `0.0.0.0:5000` (`src/cli.rs:39-47`)

**Secrets location:**

- `.env` at the repo root (or cwd), loaded by `dotenvy` (`src/config.rs:46-55`); `.env` and `*.env` are gitignored (`.gitignore`).
- `.env.example` template exists at the repo root — present but not reproduced here.
- Keys live in process env (`std::env::vars()`), are held in `EngineConfig.api_key` (`src/config.rs:9-13`) and are only ever sent as Bearer headers — they are never logged except via `--verbose` request bodies (which contain no key).

## Webhooks & Callbacks

**Incoming:**

- None — the axum server exposes only request/response routes (`src/web.rs:96-104`):
  - `GET /` (embedded HTML UI), `GET /api/engines`, `POST /api/generate`, `POST /api/generate-cli`, `GET /api/images`, `GET /api/images/{filename}`

**Outgoing:**

- None — no webhook registration, no callbacks, no SSE/WebSocket push.

**Notable local integration surface:**

- `POST /api/generate-cli` shells out via `sh -c` with a prefix allowlist (`imagai`, `cargo run -- imagai`, `cargo run --quiet -- imagai`) and a 300 s timeout — `src/web.rs:237-320`. It is unauthenticated and CORS-open; treat the web server as a localhost-only tool.

## Developer Tooling Integrations

- `graft` MCP server registered in `.mcp.json` and `opencode.json` (local command `graft mcp`) — code-graph context for AI agents; graph sources in `graft/`, cache gitignored.
- Agent config directories (`.claude/`, `.cursor/`, `.gemini/`, `.grok/`, `.kiro/`, `.windsurf/`, `.adal/`) wire the same graft skill/hooks into various editors — no runtime effect.

---

*Integration audit: 2026-10-03*
