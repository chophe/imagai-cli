# API Coverage — OpenAI-compatible Images API (`/images/edits`) + OpenRouter chat vision

> Full coverage by default. Opt-outs are explicit, reasoned decisions.

Phase 1 adds two outbound wire shapes to the existing `src/provider.rs` transport:
multipart `POST {base}/images/edits` for OpenAI-compatible image engines, and
`POST {base}/chat/completions` with `image_url` content parts for OpenRouter vision engines.
Both endpoints were already reachable for text-to-image; the **edit** surface on them is new.
No provider SDK is vendored — the transport is the hand-rolled `reqwest` client in `src/provider.rs`.

## OpenAI-compatible `/images/edits`

| capability | decision | reason |
|---|---|---|
| `multipart image[]` file part (repeatable) | INTEGRATE | |
| `multipart prompt` text part | INTEGRATE | |
| `multipart model` text part | INTEGRATE | |
| 1..16 input images per request | INTEGRATE | |
| response parsed with the existing `ImagesResponse` DTO | INTEGRATE | the edit response shape matches `/images/generations`, so `b64_json` and `url` are both already handled |
| `response_format` | OPT-OUT | not a documented parameter of `/images/edits` for GPT Image models; forwarding the request-DTO default `b64_json` returns HTTP 400 |
| `quality` | OPT-OUT | the request-DTO default is `standard`, which is not in the GPT-image enum low/medium/high/xhigh/max/auto; forwarding returns HTTP 400 |
| `style` | OPT-OUT | `vivid` is a DALL-E 3 field; `/images/edits` rejects it |
| `n` | OPT-OUT | an edit returns one image per call; this phase edits once and saves one file |
| `size` | OPT-OUT | not needed yet — output dimensions follow the source image |
| `mask` (inpainting) | OPT-OUT | not needed yet — no inpainting requirement in EDIT-01..EDIT-03 |
| `background` / `output_format` / `output_compression` | OPT-OUT | explicitly out of scope — generation-side output knobs, not part of "combine this image with this change" |
| JSON `images:[{file_id or image_url}]` body | OPT-OUT | documented OpenAI fallback; multipart is the shape OpenAI-compatible gateways proxy and the shape the official curl example uses. Research assumption A3 covers a gateway that rejects multipart |
| Files API upload plus `file_id` reference | OPT-OUT | not needed yet — inline multipart covers the input sizes this tool accepts |

## OpenRouter chat vision (`POST {base}/chat/completions`)

| capability | decision | reason |
|---|---|---|
| N `image_url` content parts in one user message | INTEGRATE | |
| text part first, then the images | INTEGRATE | OpenRouter's documented ordering |
| local files as `data:{mime};base64,{b64}` URIs | INTEGRATE | a local file has no reachable URL |
| `modalities: ["image","text"]` for image models | INTEGRATE | existing behaviour at `src/provider.rs:324-326` |
| ranking headers `HTTP-Referer` / `X-Title` | INTEGRATE | existing behaviour at `src/provider.rs:302-308` |
| OpenRouter dedicated Image API (reference images) | OPT-OUT | a new vendor wire format — PRVD-01, Phase 3 |
| OpenRouter video endpoints | OPT-OUT | VID-01, v2 |

## Inherited capabilities (already integrated by the shipped text-to-image path — must not regress)

| capability | decision | reason |
|---|---|---|
| `POST /images/generations` JSON body (unchanged) | INTEGRATE | existing; D-01 locks `generate` untouched, so `n`, `size`, `response_format`, `quality` and `style` all keep working there |
| Stability extra params (unchanged) | INTEGRATE | existing `build_images_body` handling of `negative_prompt`, `seed`, `strength`, `output_format`, `aspect_ratio` and `mode` |
| `GET /models` engine/model listing | INTEGRATE | existing `fetch_models` |
| web `GET /api/images` and `GET /api/images/{filename}` | INTEGRATE | existing gallery; D-06 keeps edited results in the same flat `output_dir`, so both keep working without change |

## Provenance

- Endpoint and field semantics: `01-RESEARCH.md` § Architecture Patterns (Pattern 1, Pattern 2), § State of the Art, sources `developers.openai.com` and `openrouter.ai`.
- Feature-flag and dependency evidence: **measured, not quoted.** A throwaway probe project requesting `reqwest 0.13` with features `["json", "rustls", "webpki-roots", "multipart"]` was resolved with `cargo generate-lockfile`, and its lockfile was diffed against this repository's. Toggling `multipart` vendors two crates that are absent from `Cargo.lock` today:

  | package | version | checksum | yanked | source |
  |---------|---------|----------|--------|--------|
  | `mime_guess` | 2.0.5 | `f7c44f8e672c00fe5308fa235f821cb4198414e1c77935c1ab6948d3fd78550e` | no | `https://github.com/abonander/mime_guess`, MIT |
  | `unicase` | 2.10.0 | `357cc3acc6a036009fd6c973ed009037c732d60d0b4f6c673e9041497482a28f` | no | Unicode caseless-mapping crate (`unicode-rs`) |

  `futures-util` is already present in `Cargo.lock` and adds nothing. Both new entries are vetted at the `gate="blocking-human"` checkpoint that heads plan `01-01`, and `Cargo.lock` is declared in that plan's `files_modified`.
- This corrects `01-RESEARCH.md` § Package Legitimacy Audit, which asserts "No external packages are installed by this phase." That premise is refuted by the lockfile diff above. The research is an input artifact and is left unedited; the plans and this ledger carry the corrected account.