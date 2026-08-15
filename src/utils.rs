use std::path::{Path, PathBuf};

use crate::config::{EngineConfig, Settings};
use crate::provider::chat_completion;

/// Sanitize a string to be a valid, safe filename.
pub fn sanitize_filename(name: &str) -> String {
    let mut s: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if (c as u32) <= 0x1F => '_',
            c => c,
        })
        .collect();
    // Collapse whitespace runs into a single underscore.
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !prev_space {
                out.push('_');
            }
            prev_space = true;
        } else {
            out.push(c);
            prev_space = false;
        }
    }
    s = out;
    if s.len() > 100 {
        s.truncate(100);
    }
    s
}

/// Return the lower-case image extension for a file, defaulting to "png".
pub fn get_image_extension(path: &Path) -> String {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "jpg" | "jpeg" | "png" | "gif" | "webp" => ext,
        _ => "png".to_string(),
    }
}

/// Random filename: `image_<timestamp>_<uuid8>.<ext>`.
pub fn generate_random_filename(extension: &str) -> String {
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let random_str = &uuid::Uuid::new_v4().to_string()[..8];
    format!("image_{timestamp}_{random_str}.{extension}")
}

/// Default filename derived from the prompt (truncated) plus a timestamp.
pub fn generate_filename(prompt: Option<&str>, extension: &str) -> String {
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    match prompt {
        Some(p) => {
            let sane: String = p
                .chars()
                .take(30)
                .map(|c| {
                    if c.is_alphanumeric() || c == ' ' || c == '-' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect::<String>()
                .trim_end()
                .replace(' ', "_");
            format!("{sane}_{timestamp}.{extension}")
        }
        None => format!("image_{timestamp}.{extension}"),
    }
}

/// Generate a filename from the prompt using an LLM (chat completions).
///
/// Engine priority: `filename_generation` > `default_engine` > first engine
/// whose name contains "openai".
pub async fn generate_filename_from_prompt_llm(
    settings: &Settings,
    prompt: &str,
    extension: &str,
    verbose: bool,
) -> String {
    let chosen: Option<(&String, &EngineConfig)> = {
        let engines = &settings.engines;
        engines
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("filename_generation"))
            .or_else(|| {
                settings
                    .default_engine
                    .as_ref()
                    .and_then(|d| engines.iter().find(|(n, _)| n.eq_ignore_ascii_case(d)))
            })
            .or_else(|| {
                engines
                    .iter()
                    .find(|(name, _)| name.to_lowercase().contains("openai"))
            })
    };
    let Some((_name, cfg)) = chosen else {
        return generate_filename(Some(prompt), extension);
    };
    if !cfg.key_set() {
        return generate_filename(Some(prompt), extension);
    }

    let model = cfg
        .model
        .clone()
        .unwrap_or_else(|| "gpt-4.1-mini".to_string());
    let messages = serde_json::json!([
        {
            "role": "system",
            "content": "You are a helpful assistant that generates concise, descriptive, and filesystem-safe filenames based on user prompts. The filename should not include the file extension. Max 10 words.",
        },
        {
            "role": "user",
            "content": format!("Generate a best english filename for the pictures that will be generated from this prompt: `{prompt}`"),
        },
    ]);

    if verbose {
        eprintln!("--- LLM Filename Generation ---");
        eprintln!("Model: {model}");
        eprintln!(
            "{}",
            serde_json::to_string_pretty(&messages).unwrap_or_default()
        );
    }

    match chat_completion(
        &cfg.base_url,
        &cfg.api_key,
        &model,
        &messages,
        Some(20),
        Some(0.2),
    )
    .await
    {
        Ok(resp) => {
            let content = resp
                .choices
                .first()
                .and_then(|c| c.message.content.as_ref())
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            if verbose {
                eprintln!("LLM response: {content}");
            }
            if content.is_empty() {
                generate_filename(Some(prompt), extension)
            } else {
                let sanitized = sanitize_filename(&content);
                let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
                format!("{sanitized}_{timestamp}.{extension}")
            }
        }
        Err(e) => {
            eprintln!("Warning: LLM filename generation failed: {e}");
            generate_filename(Some(prompt), extension)
        }
    }
}

/// Download an image from a URL and save it to `output_path`.
pub async fn save_image_from_url(
    client: &reqwest::Client,
    url: &str,
    output_path: &Path,
    prompt: Option<&str>,
    model: Option<&str>,
) -> anyhow::Result<PathBuf> {
    let resp = client.get(url).send().await?;
    let status = resp.status();
    if !status.is_success() {
        anyhow::bail!("HTTP error downloading image {url}: {status}");
    }
    let bytes = resp.bytes().await?;
    save_image_bytes(&bytes, output_path, prompt, model)?;
    Ok(output_path.to_path_buf())
}

/// Decode a base64 image payload and save it to `output_path`.
pub async fn save_image_from_b64(
    b64: &str,
    output_path: &Path,
    prompt: Option<&str>,
    model: Option<&str>,
) -> anyhow::Result<PathBuf> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|e| anyhow::anyhow!("failed to decode base64 image: {e}"))?;
    save_image_bytes(&bytes, output_path, prompt, model)?;
    Ok(output_path.to_path_buf())
}

fn save_image_bytes(
    bytes: &[u8],
    output_path: &Path,
    prompt: Option<&str>,
    model: Option<&str>,
) -> anyhow::Result<()> {
    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let ext = get_image_extension(output_path);
    let final_bytes = if ext == "png" && bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        if let (Some(p), Some(m)) = (prompt, model) {
            inject_png_metadata(bytes, p, m)
        } else {
            bytes.to_vec()
        }
    } else {
        bytes.to_vec()
    };
    std::fs::write(output_path, final_bytes)?;
    Ok(())
}

/// Inject `Prompt` and `Model` as tEXt chunks into a PNG byte stream.
pub fn inject_png_metadata(bytes: &[u8], prompt: &str, model: &str) -> Vec<u8> {
    if bytes.len() < 8 {
        return bytes.to_vec();
    }
    let mut pos = 8usize;
    while pos + 8 <= bytes.len() {
        let len = u32::from_be_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
        let ctype = &bytes[pos + 4..pos + 8];
        if ctype == b"IEND" {
            let mut out = Vec::with_capacity(bytes.len() + 256);
            out.extend_from_slice(&bytes[..pos]);
            out.extend_from_slice(&png_text_chunk("Prompt", prompt));
            out.extend_from_slice(&png_text_chunk("Model", model));
            out.extend_from_slice(&bytes[pos..]);
            return out;
        }
        pos += 12 + len;
    }
    bytes.to_vec()
}

/// Build a single PNG tEXt chunk (keyword, null byte, text, CRC32).
fn png_text_chunk(keyword: &str, text: &str) -> Vec<u8> {
    let mut data = Vec::with_capacity(keyword.len() + 1 + text.len());
    data.extend_from_slice(keyword.as_bytes());
    data.push(0);
    data.extend_from_slice(text.as_bytes());

    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(b"tEXt");
    crc_input.extend_from_slice(&data);
    let crc = crc32fast::hash(&crc_input);

    let mut chunk = Vec::with_capacity(12 + data.len());
    chunk.extend_from_slice(&(data.len() as u32).to_be_bytes());
    chunk.extend_from_slice(b"tEXt");
    chunk.extend_from_slice(&data);
    chunk.extend_from_slice(&crc.to_be_bytes());
    chunk
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    const PIXEL_PNG_B64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

    fn pixel_png() -> Vec<u8> {
        base64::engine::general_purpose::STANDARD
            .decode(PIXEL_PNG_B64)
            .unwrap()
    }

    #[test]
    fn sanitize_replaces_special_chars_and_collapses_whitespace() {
        assert_eq!(sanitize_filename("a b  c"), "a_b_c");
        assert_eq!(
            sanitize_filename("a<b>c:d\"e/f\\g|h?i*j"),
            "a_b_c_d_e_f_g_h_i_j"
        );
        assert_eq!(
            sanitize_filename(" tab\tand\nnewline "),
            "_tab_and_newline_"
        );
        assert_eq!(sanitize_filename("ok-name_1"), "ok-name_1");
        assert_eq!(sanitize_filename(&"x".repeat(150)).len(), 100);
    }

    #[test]
    fn generate_filename_from_prompt() {
        let name = generate_filename(Some("A beautiful Sunset!"), "png");
        assert!(name.starts_with("A_beautiful_Sunset__"));
        assert!(name.ends_with(".png"));
        assert!(name.contains(&chrono::Local::now().format("%Y%m%d_%H%M%S").to_string()));

        let no_prompt = generate_filename(None, "png");
        assert!(no_prompt.starts_with("image_"));
        assert!(no_prompt.ends_with(".png"));
    }

    #[test]
    fn random_filename_shape() {
        let a = generate_random_filename("png");
        let b = generate_random_filename("png");
        assert_ne!(a, b);
        assert!(a.starts_with("image_"));
        assert!(a.ends_with(".png"));
        // image_<YYYYMMDD>_<HHMMSS>_<uuid8>.png
        let stem = a.trim_end_matches(".png");
        let parts: Vec<&str> = stem.split('_').collect();
        assert_eq!(parts.len(), 4, "parts: {parts:?}");
        assert_eq!(parts[0], "image");
        assert_eq!(parts[1].len(), 8, "date part");
        assert_eq!(parts[2].len(), 6, "time part");
        assert_eq!(parts[3].len(), 8, "uuid part");
        assert!(parts[3].chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn image_extension_detection() {
        assert_eq!(get_image_extension(Path::new("x.PNG")), "png");
        assert_eq!(get_image_extension(Path::new("x.jpeg")), "jpeg");
        assert_eq!(get_image_extension(Path::new("x.webp")), "webp");
        assert_eq!(get_image_extension(Path::new("x.gif")), "gif");
        assert_eq!(get_image_extension(Path::new("x.txt")), "png");
        assert_eq!(get_image_extension(Path::new("noext")), "png");
    }

    /// Walk PNG chunks and collect (type, data, crc-ok).
    fn parse_png_chunks(bytes: &[u8]) -> Vec<(String, Vec<u8>, bool)> {
        let mut out = Vec::new();
        assert!(
            bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
            "valid PNG signature"
        );
        let mut pos = 8usize;
        while pos + 8 <= bytes.len() {
            let len = u32::from_be_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
            let ctype = String::from_utf8(bytes[pos + 4..pos + 8].to_vec()).unwrap();
            let data = bytes[pos + 8..pos + 8 + len].to_vec();
            let stored_crc =
                u32::from_be_bytes(bytes[pos + 8 + len..pos + 12 + len].try_into().unwrap());
            let mut crc_input = bytes[pos + 4..pos + 8].to_vec();
            crc_input.extend_from_slice(&data);
            let crc_ok = crc32fast::hash(&crc_input) == stored_crc;
            out.push((ctype, data, crc_ok));
            pos += 12 + len;
        }
        out
    }

    #[test]
    fn inject_png_metadata_adds_text_chunks() {
        let png = pixel_png();
        let out = inject_png_metadata(&png, "my prompt here", "dall-e-3");
        assert!(out.len() > png.len());

        let chunks = parse_png_chunks(&out);
        let texts: Vec<&(String, Vec<u8>, bool)> =
            chunks.iter().filter(|(t, _, _)| t == "tEXt").collect();
        assert_eq!(texts.len(), 2, "Prompt + Model tEXt chunks");

        // Prompt chunk.
        let prompt_chunk = &texts[0].1;
        let idx = prompt_chunk.iter().position(|&b| b == 0).unwrap();
        assert_eq!(&prompt_chunk[..idx], b"Prompt");
        assert_eq!(&prompt_chunk[idx + 1..], b"my prompt here");

        // Model chunk.
        let model_chunk = &texts[1].1;
        let idx = model_chunk.iter().position(|&b| b == 0).unwrap();
        assert_eq!(&model_chunk[..idx], b"Model");
        assert_eq!(&model_chunk[idx + 1..], b"dall-e-3");

        // All chunk CRCs valid.
        assert!(chunks.iter().all(|(_, _, ok)| *ok));

        // IEND still last.
        assert_eq!(chunks.last().unwrap().0, "IEND");
    }

    #[test]
    fn inject_png_metadata_leaves_non_png_untouched() {
        let junk = b"not a png at all".to_vec();
        assert_eq!(inject_png_metadata(&junk, "p", "m"), junk);
    }

    #[test]
    fn png_text_chunk_is_well_formed() {
        let chunk = png_text_chunk("Keyword", "value");
        // keyword "Keyword" (7) + null + "value" (5) = 13 bytes of data.
        assert_eq!(u32::from_be_bytes(chunk[..4].try_into().unwrap()), 13);
        assert_eq!(&chunk[4..8], b"tEXt");
        assert_eq!(&chunk[8..21], b"Keyword\x00value");

        let stored_crc = u32::from_be_bytes(chunk[21..].try_into().unwrap());
        assert_eq!(
            crc32fast::hash(&chunk[4..21]),
            stored_crc,
            "CRC covers type+data"
        );
    }
}
