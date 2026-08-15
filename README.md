# Imagai CLI 🎨

A fast, fully-featured **Rust** rewrite of the Imagai Python project — a CLI, interactive **TUI**, and **web interface** to generate images using OpenAI-compatible AI APIs (DALL-E 3, GPT-image, Gemini/Imagen, Stable Diffusion / Stability, OpenRouter vision, and any OpenAI-compatible gateway).

## Features

- 🖥️ **Three frontends in one binary**
  - **CLI** — scriptable image generation (`imagai generate`)
  - **TUI** — full-screen interactive terminal UI (`imagai tui`)
  - **Web** — REST API + browser UI (`imagai web`)
- 🔌 **Multiple engines** via OpenAI-compatible endpoints:
  - OpenAI images API (`images/generations`) for DALL-E 3, GPT-image, Imagen
  - Stability-style extra params (`negative_prompt`, `seed`, `strength`, `aspect_ratio`, `mode`, …)
  - OpenRouter chat completions for Gemini text / vision / image models
- 📄 **Smart filenames**: manual, LLM-generated (`--auto-filename`), random, or prompt-derived
- 🏷️ **Metadata injection**: prompt + model written into saved PNGs (`tEXt` chunks)
- 📊 Usage & cost reporting, `--verbose` request dumps
- 🔍 `list-engines --all` to fetch and browse available models per engine
- 🔒 Path-traversal-safe image serving in the web UI

## Install

Requires [Rust](https://rustup.rs/) (edition 2021).

```bash
git clone https://github.com/chophe/imagai-cli.git
cd imagai-cli
cargo build --release
# binary: ./target/release/imagai
```

## Quick start

```bash
# 1) Configure your engines
cp .env.example .env   # then fill in API keys

# 2) See help
imagai --help
imagai generate --help

# 3) Generate an image
imagai generate -p "A beautiful sunset over a mountain range"

# 4) Launch the interactive TUI
imagai tui

# 5) Launch the web interface
imagai web --port 5000   # http://localhost:5000
```

## CLI usage

```bash
imagai generate -p "A cat wearing a hat" -o cat_with_hat.png
imagai generate -p "Futuristic city skyline at night" --auto-filename
imagai generate -p "Abstract art" --random-filename --size 1792x1024
imagai generate -p "A dog" --engine openai_dalle3 -n 2
imagai generate -p "dreamy portrait" --negative-prompt "blurry, text" --seed 42
imagai list-engines
imagai list-engines --all
```

### Options

| Flag | Description |
| --- | --- |
| `-p, --prompt` | Text prompt (reads from stdin if omitted) |
| `--engine` | Engine name from `.env` (falls back to `IMAGAI__DEFAULT_ENGINE`) |
| `-o, --output` | Output filename (multi-image gets `_1`, `_2`, … suffixes) |
| `-n, --num-images` | Number of images (default 1) |
| `--size` | e.g. `1024x1024`, `1792x1024` |
| `--quality`, `--style` | DALL-E 3: `standard`/`hd`, `vivid`/`natural` |
| `--auto-filename` | Ask an LLM for a descriptive filename |
| `--random-filename` | Random timestamped filename |
| `--negative-prompt`, `--seed`, `--strength`, `--output-format`, `--aspect-ratio`, `--mode` | Stability-style extra params |
| `--image-url` | Vision input for OpenRouter Gemini models |
| `--verbose` | Print the API request body |

Filename precedence: `--output` > `--auto-filename` > `--random-filename` > prompt-derived default.

## Configuration

Configuration lives in a `.env` file or environment variables:

```
IMAGAI__DEFAULT_ENGINE=openai_dalle3
IMAGAI__OUTPUT_DIR=generated_images
IMAGAI__ENGINES__OPENAI_DALLE3__API_KEY=sk-...
IMAGAI__ENGINES__OPENAI_DALLE3__MODEL=dall-e-3
IMAGAI__ENGINES__IMAGEN4__API_KEY=...
IMAGAI__ENGINES__IMAGEN4__BASE_URL=https://your-gateway/v1
```

## Web API

Start with `imagai web`, then:

| Endpoint | Method | Description |
| --- | --- | --- |
| `/` | GET | Web interface |
| `/api/engines` | GET | Configured engines |
| `/api/generate` | POST | `{prompt, engine?, n?, size?, quality?, style?, ...}` → images + base64 previews |
| `/api/generate-cli` | POST | Run a `imagai …` command server-side |
| `/api/images` | GET | List generated images |
| `/api/images/<file>` | GET | Serve a generated image |

## Testing

The suite includes unit tests plus end-to-end tests that run the real binary
against an in-process mock OpenAI-compatible server.

```bash
cargo test            # all tests
cargo test --lib      # unit tests only
cargo test --test cli # end-to-end binary tests (generate, list-engines, errors)
cargo test --test web # REST API tests (in-process)
cargo test --test core # generation pipeline tests
```

The CLI tests spawn the compiled `imagai` binary as a subprocess, so the
binary is always rebuilt automatically by `cargo test`.

## Notes

- This is a from-scratch Rust port; behavior mirrors the original Python CLI
  (`https://github.com/chophe/imagai`-style project), but no Python runtime is required.
- Engine names are matched case-insensitively.
- The web `/api/generate-cli` endpoint only accepts commands that begin with `imagai`.

## License

MIT — see [LICENSE](LICENSE).
