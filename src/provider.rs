use std::collections::HashMap;
use std::path::Path;

use serde_json::{json, Value};

use crate::config::EngineConfig;
use crate::models::{
    ChatImage, ChatResponse, ImageGenerationRequest, ImageGenerationResponse, ImagesResponse,
    ModelsListResponse,
};

/// Build a shared HTTP client.
pub fn http_client() -> anyhow::Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(concat!("imagai/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| anyhow::anyhow!("failed to build HTTP client: {e}"))
}

/// Fetch the list of model ids exposed by an OpenAI-compatible `{base}/models`.
pub async fn fetch_models(base_url: Option<&str>, api_key: &str) -> anyhow::Result<Vec<String>> {
    let client = http_client()?;
    let url = match base_url {
        Some(b) => format!("{}/models", b.trim_end_matches('/')),
        None => "https://api.openai.com/v1/models".to_string(),
    };
    let resp = client.get(&url).bearer_auth(api_key).send().await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        anyhow::bail!("models fetch failed ({status}): {}", truncate(&text, 400));
    }

    // Structured parse first.
    if let Ok(list) = serde_json::from_str::<ModelsListResponse>(&text) {
        let mut ids: Vec<String> = list
            .data
            .into_iter()
            .filter_map(|m| m.id.or(m.name).or(m.model))
            .collect();
        ids.sort();
        ids.dedup();
        return Ok(ids);
    }

    // Lenient fallback: `{"data": [...]}` or a bare list of strings/objects.
    let v: Value = serde_json::from_str(&text)?;
    let mut ids: Vec<String> = Vec::new();
    let items: Option<&Vec<Value>> = v
        .get("data")
        .and_then(|d| d.as_array())
        .or_else(|| v.as_array());
    if let Some(arr) = items {
        for item in arr {
            if let Some(s) = item.as_str() {
                ids.push(s.to_string());
            } else {
                for key in ["id", "name", "model"] {
                    if let Some(s) = item.get(key).and_then(|i| i.as_str()) {
                        ids.push(s.to_string());
                        break;
                    }
                }
            }
        }
    }
    ids.sort();
    ids.dedup();
    Ok(ids)
}

fn resolve_url(base_url: &Option<String>, path: &str) -> String {
    match base_url {
        Some(b) => format!(
            "{}/{}",
            b.trim_end_matches('/'),
            path.trim_start_matches('/')
        ),
        None => format!("https://api.openai.com/v1/{}", path.trim_start_matches('/')),
    }
}

/// Generic OpenAI-compatible chat completion helper.
pub async fn chat_completion(
    base_url: &Option<String>,
    api_key: &str,
    model: &str,
    messages: &Value,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
) -> anyhow::Result<ChatResponse> {
    let client = http_client()?;
    let url = resolve_url(base_url, "chat/completions");
    let mut body = json!({ "model": model, "messages": messages });
    if let Some(mt) = max_tokens {
        body["max_tokens"] = json!(mt);
    }
    if let Some(t) = temperature {
        body["temperature"] = json!(t);
    }
    let resp = client
        .post(&url)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "chat completion failed ({status}): {}",
            truncate(&text, 500)
        ));
    }
    let parsed: ChatResponse = serde_json::from_str(&text)?;
    Ok(parsed)
}

/// Which wire shape an edit request must use for a given engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditTransport {
    /// `POST {base}/images/edits` as `multipart/form-data`.
    MultipartEdits,
    /// `POST {base}/chat/completions` with `image_url` content parts.
    ChatVision,
}

/// Decide how an edit request should be sent, before any socket is opened.
///
/// Takes the engine *name* as well as its config so a failure can identify the
/// engine from the message alone (D-05). Deliberately does not consult
/// `crate::cli::is_image_model`: that heuristic is display-only and `dall-e-3`
/// passes it while still being unable to take image input.
pub fn edit_transport(name: &str, cfg: &EngineConfig) -> anyhow::Result<EditTransport> {
    let model = cfg.model.clone().unwrap_or_else(|| "dall-e-3".to_string());
    let is_openrouter = cfg
        .base_url
        .as_deref()
        .map(|u| u.contains("openrouter.ai"))
        .unwrap_or(false);

    if is_openrouter {
        return Ok(EditTransport::ChatVision);
    }
    if model.to_lowercase().contains("gpt-image") {
        return Ok(EditTransport::MultipartEdits);
    }
    anyhow::bail!(
        "Engine '{name}' (model '{model}') cannot accept image input for edits. \
         Set IMAGAI__ENGINES__<NAME>__MODEL to a gpt-image model, or point BASE_URL at an \
         OpenRouter vision engine."
    )
}

/// MIME type for an image path's extension, defaulting to PNG.
fn image_mime_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        _ => "image/png",
    }
}

/// The ordered image list an edit request carries: the source first, then the
/// user's references in the order given. Never sorted or deduplicated —
/// reordering changes what the model is being asked to blend, and silently
/// dropping a duplicate hides a user mistake.
fn edit_image_paths(request: &ImageGenerationRequest) -> Vec<&str> {
    std::iter::once(request.source_image.as_deref())
        .flatten()
        .chain(request.ref_images.iter().map(|p| p.as_str()))
        .collect()
}

/// Generate images using the OpenAI-compatible images API (with special-casing
/// for OpenRouter chat-based models and Stability-style extra params).
pub async fn generate_images(
    request: &ImageGenerationRequest,
    config: &EngineConfig,
) -> Vec<ImageGenerationResponse> {
    let model = config
        .model
        .clone()
        .unwrap_or_else(|| "dall-e-3".to_string());
    let is_openrouter = config
        .base_url
        .as_deref()
        .map(|u| u.contains("openrouter.ai"))
        .unwrap_or(false);

    // ---- OpenRouter chat-based generation (e.g. Gemini vision) ----
    if is_openrouter && model.to_lowercase().contains("gemini") {
        return match openrouter_chat_generate(request, config, &model).await {
            Ok(resp) => vec![resp],
            Err(e) => vec![ImageGenerationResponse {
                error: Some(e.to_string()),
                ..Default::default()
            }],
        };
    }

    let client = match http_client() {
        Ok(c) => c,
        Err(e) => {
            return vec![ImageGenerationResponse {
                error: Some(e.to_string()),
                ..Default::default()
            }]
        }
    };
    let url = resolve_url(&config.base_url, "images/generations");
    let body = build_images_body(request, config, &model);

    if request.verbose {
        eprintln!("--- API Request Body ---");
        eprintln!(
            "{}",
            serde_json::to_string_pretty(&body).unwrap_or_default()
        );
        eprintln!("------------------------");
    }

    let resp = match client
        .post(&url)
        .bearer_auth(&config.api_key)
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return vec![ImageGenerationResponse {
                error: Some(format!("request error: {e}")),
                ..Default::default()
            }]
        }
    };
    let status = resp.status();
    let text = match resp.text().await {
        Ok(t) => t,
        Err(e) => {
            return vec![ImageGenerationResponse {
                error: Some(format!("failed reading response: {e}")),
                ..Default::default()
            }]
        }
    };

    if !status.is_success() {
        let msg = extract_api_error(&text)
            .unwrap_or_else(|| format!("HTTP {status}: {}", truncate(&text, 400)));
        return vec![ImageGenerationResponse {
            error: Some(msg),
            ..Default::default()
        }];
    }

    let parsed: ImagesResponse = match serde_json::from_str(&text) {
        Ok(p) => p,
        Err(e) => {
            return vec![ImageGenerationResponse {
                error: Some(format!("failed to parse response: {e}")),
                ..Default::default()
            }]
        }
    };

    let mut responses = Vec::with_capacity(parsed.data.len());
    for img in parsed.data {
        let mut r = ImageGenerationResponse {
            usage: parsed.usage.clone(),
            estimated_cost: parsed.estimated_cost.clone(),
            ..Default::default()
        };
        if let Some(url) = img.url {
            r.image_url = Some(url);
        } else if let Some(b64) = img.b64_json {
            r.image_b64_json = Some(b64);
        } else {
            r.error = Some("No image data found in API response.".to_string());
        }
        responses.push(r);
    }
    if responses.is_empty() {
        responses.push(ImageGenerationResponse {
            error: Some("API returned no images.".to_string()),
            ..Default::default()
        });
    }
    responses
}

/// Edit an existing image by combining it with a change instruction.
///
/// Dispatches on [`edit_transport`]: multipart `/images/edits` for gpt-image
/// engines, chat vision for OpenRouter engines. The multipart form carries
/// only `model`, `prompt` and the `image[]` parts — `quality`, `style`,
/// `response_format` and `n` belong to `/images/generations` and are rejected
/// by the edits endpoint.
pub async fn generate_edits(
    request: &ImageGenerationRequest,
    config: &EngineConfig,
    model: &str,
) -> Vec<ImageGenerationResponse> {
    let transport = match edit_transport(&request.engine, config) {
        Ok(t) => t,
        Err(e) => {
            return vec![ImageGenerationResponse {
                error: Some(e.to_string()),
                ..Default::default()
            }]
        }
    };

    if matches!(transport, EditTransport::ChatVision) {
        return match openrouter_chat_generate(request, config, model).await {
            Ok(resp) => vec![resp],
            Err(e) => vec![ImageGenerationResponse {
                error: Some(e.to_string()),
                ..Default::default()
            }],
        };
    }

    // A missing source is an error, never a refs-only request.
    let Some(source) = request.source_image.as_deref() else {
        return vec![ImageGenerationResponse {
            error: Some("An edit request requires a source image.".to_string()),
            ..Default::default()
        }];
    };

    let paths = edit_image_paths(request);

    // Hard failure rather than truncation: dropping references without saying
    // so would hand the model a different request than the user asked for.
    const MAX_IMAGES: usize = 16;
    if paths.len() > MAX_IMAGES {
        return vec![ImageGenerationResponse {
            error: Some(format!(
                "edit request carries {} images but the provider accepts at most {MAX_IMAGES} \
                 (1 source + {} references); 1 source and {} references were given",
                paths.len(),
                MAX_IMAGES - 1,
                request.ref_images.len()
            )),
            ..Default::default()
        }];
    }

    let client = match http_client() {
        Ok(c) => c,
        Err(e) => {
            return vec![ImageGenerationResponse {
                error: Some(e.to_string()),
                ..Default::default()
            }]
        }
    };

    let mut form = reqwest::multipart::Form::new()
        .text("model", model.to_string())
        .text("prompt", request.prompt.clone());

    for path in &paths {
        // `{path:?}` — Debug for Path is byte-safe, unlike `truncate`.
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                return vec![ImageGenerationResponse {
                    error: Some(format!("failed to read source image {path:?}: {e}")),
                    ..Default::default()
                }]
            }
        };
        let p = Path::new(path);
        let name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("image.png");
        let part = match reqwest::multipart::Part::bytes(bytes)
            .file_name(name.to_string())
            .mime_str(image_mime_for(p))
        {
            Ok(part) => part,
            Err(e) => {
                return vec![ImageGenerationResponse {
                    error: Some(format!("failed to build image part for {path:?}: {e}")),
                    ..Default::default()
                }]
            }
        };
        form = form.part("image[]".to_string(), part);
    }

    if request.verbose {
        // Never the image bytes — filenames only.
        eprintln!("--- Edit Request ---");
        eprintln!("model: {model}");
        eprintln!("source: {source}");
        eprintln!("references: {}", request.ref_images.len());
        eprintln!("--------------------");
    }

    let url = resolve_url(&config.base_url, "images/edits");
    let resp = match client
        .post(&url)
        .bearer_auth(&config.api_key)
        .multipart(form)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return vec![ImageGenerationResponse {
                error: Some(format!("request error: {e}")),
                ..Default::default()
            }]
        }
    };
    let status = resp.status();
    let text = match resp.text().await {
        Ok(t) => t,
        Err(e) => {
            return vec![ImageGenerationResponse {
                error: Some(format!("failed reading response: {e}")),
                ..Default::default()
            }]
        }
    };

    if !status.is_success() {
        let msg = extract_api_error(&text)
            .unwrap_or_else(|| format!("HTTP {status}: {}", truncate(&text, 400)));
        return vec![ImageGenerationResponse {
            error: Some(msg),
            ..Default::default()
        }];
    }

    let parsed: ImagesResponse = match serde_json::from_str(&text) {
        Ok(p) => p,
        Err(e) => {
            return vec![ImageGenerationResponse {
                error: Some(format!("failed to parse response: {e}")),
                ..Default::default()
            }]
        }
    };

    let mut responses = Vec::with_capacity(parsed.data.len());
    for img in parsed.data {
        let mut r = ImageGenerationResponse {
            usage: parsed.usage.clone(),
            estimated_cost: parsed.estimated_cost.clone(),
            ..Default::default()
        };
        if let Some(url) = img.url {
            r.image_url = Some(url);
        } else if let Some(b64) = img.b64_json {
            r.image_b64_json = Some(b64);
        } else {
            r.error = Some("No image data found in API response.".to_string());
        }
        responses.push(r);
    }
    if responses.is_empty() {
        responses.push(ImageGenerationResponse {
            error: Some("API returned no images.".to_string()),
            ..Default::default()
        });
    }
    responses
}

/// Build the request body for the OpenAI-compatible images API.
///
/// Special cases:
/// - DALL-E 3 models get `quality` and `style`.
/// - Stability-style models drop `n`/`response_format` and accept
///   Stability-specific extra params; `size` is dropped when `aspect_ratio`
///   is present, and a default `mode` is injected for non-sd3 models.
pub fn build_images_body(
    request: &ImageGenerationRequest,
    config: &EngineConfig,
    model: &str,
) -> serde_json::Value {
    let mut body = json!({
        "model": model,
        "prompt": request.prompt,
        "n": request.n.max(1),
        "size": request.size,
        "response_format": request.response_format,
    });

    if model.contains("dall-e-3") {
        body["quality"] = json!(request.quality);
        body["style"] = json!(request.style);
    }

    if model.to_lowercase().contains("stability") {
        if let Some(obj) = body.as_object_mut() {
            obj.remove("n");
            obj.remove("response_format");
        }
        const STABILITY_KEYS: [&str; 6] = [
            "negative_prompt",
            "seed",
            "strength",
            "output_format",
            "aspect_ratio",
            "mode",
        ];
        for key in STABILITY_KEYS {
            if let Some(v) = request.extra_params.get(key) {
                body[key] = v.clone();
            }
        }
        if body.get("mode").is_none() && !model.to_lowercase().contains("sd3") {
            body["mode"] = json!("text-to-image");
        }
        if body.get("aspect_ratio").is_some() {
            if let Some(obj) = body.as_object_mut() {
                obj.remove("size");
            }
        }
    }

    let _ = config;
    body
}

async fn openrouter_chat_generate(
    request: &ImageGenerationRequest,
    config: &EngineConfig,
    model: &str,
) -> anyhow::Result<ImageGenerationResponse> {
    let client = http_client()?;
    let url = resolve_url(&config.base_url, "chat/completions");

    // Optional OpenRouter ranking headers from env.
    let mut extra_headers = HashMap::new();
    if let Ok(v) = std::env::var("OPENROUTER_HTTP_REFERER") {
        extra_headers.insert("HTTP-Referer".to_string(), v);
    }
    if let Ok(v) = std::env::var("OPENROUTER_X_TITLE") {
        extra_headers.insert("X-Title".to_string(), v);
    }

    let mut content_items: Vec<Value> = vec![json!({"type": "text", "text": request.prompt})];
    // Edit path: source first, then references, as data URIs. A local file has
    // no reachable URL, and a raw filesystem path must never enter a content part.
    for path in edit_image_paths(request) {
        match std::fs::read(path) {
            Ok(bytes) => {
                use base64::Engine;
                let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
                let uri = format!("data:{};base64,{b64}", image_mime_for(Path::new(path)));
                content_items.push(json!({"type": "image_url", "image_url": {"url": uri}}));
            }
            Err(e) => {
                return Err(anyhow::anyhow!("failed to read source image {path:?}: {e}"));
            }
        }
    }
    if let Some(img_url) = request
        .extra_params
        .get("image_url")
        .and_then(|v| v.as_str())
    {
        content_items.push(json!({"type": "image_url", "image_url": {"url": img_url}}));
    }

    let is_image_model = model.to_lowercase().contains("image");
    let mut body = json!({
        "model": model,
        "messages": [{"role": "user", "content": content_items}],
    });
    if is_image_model {
        body["modalities"] = json!(["image", "text"]);
    }

    if request.verbose {
        eprintln!("--- OpenRouter Chat.Completions Request ---");
        eprintln!(
            "{}",
            serde_json::to_string_pretty(&body).unwrap_or_default()
        );
        eprintln!("-------------------------------------------");
    }

    let mut builder = client.post(&url).bearer_auth(&config.api_key).json(&body);
    for (k, v) in extra_headers {
        builder = builder.header(k, v);
    }

    let resp = builder.send().await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "OpenRouter chat failed ({status}): {}",
            truncate(&text, 500)
        ));
    }
    let parsed: ChatResponse = serde_json::from_str(&text)?;

    let mut out = ImageGenerationResponse {
        usage: parsed.usage.clone(),
        ..Default::default()
    };

    let Some(choice) = parsed.choices.first() else {
        out.error = Some("No choices returned from OpenRouter chat completion.".to_string());
        return Ok(out);
    };

    let content_value = choice.message.content.clone();
    let content = content_value
        .as_ref()
        .and_then(extract_text_content)
        .unwrap_or_default();
    let images = choice.message.images.clone().unwrap_or_default();

    if is_image_model && content.starts_with("data:image/") {
        match content.split_once(',') {
            Some((_, b64)) => {
                out.image_b64_json = Some(b64.to_string());
            }
            None => {
                out.error = Some("Failed to extract image data from content.".to_string());
                out.text_content = Some(content);
            }
        }
    } else if is_image_model && !images.is_empty() {
        let img: Option<&ChatImage> = images.first();
        if let Some(url) = img
            .and_then(|i| i.image_url.as_ref())
            .and_then(|u| u.url.clone())
        {
            if url.starts_with("data:image/") {
                out.image_b64_json = url.split_once(',').map(|(_, b)| b.to_string());
            } else {
                out.image_url = Some(url);
            }
        } else {
            out.error = Some("Failed to extract image data.".to_string());
        }
    } else if !content.is_empty() {
        out.text_content = Some(content);
    }

    if out.image_b64_json.is_none()
        && out.image_url.is_none()
        && out.text_content.is_none()
        && !is_image_model
    {
        out.error = Some("No content returned from OpenRouter chat completion.".to_string());
    }

    Ok(out)
}

/// Extract a text string from a chat content value (string or array of parts).
fn extract_text_content(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Array(items) => {
            let mut parts = Vec::new();
            for item in items {
                if let Some(t) = item.get("text").and_then(|t| t.as_str()) {
                    parts.push(t.to_string());
                }
            }
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n"))
            }
        }
        _ => None,
    }
}

/// Parse `{"error": {"message": "..."}}` from an API error body.
fn extract_api_error(text: &str) -> Option<String> {
    let v: Value = serde_json::from_str(text).ok()?;
    let err = v.get("error")?;
    let msg = err
        .get("message")
        .and_then(|m| m.as_str())
        .unwrap_or("unknown error");
    let etype = err.get("type").and_then(|t| t.as_str());
    Some(match etype {
        Some(t) => format!("{msg} (type: {t})"),
        None => msg.to_string(),
    })
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn request_with_extra(extra: HashMap<String, Value>) -> ImageGenerationRequest {
        ImageGenerationRequest {
            prompt: "a cat".to_string(),
            engine: "mock".to_string(),
            size: "1024x1024".to_string(),
            quality: "standard".to_string(),
            n: 2,
            style: "vivid".to_string(),
            response_format: "b64_json".to_string(),
            extra_params: extra,
            verbose: false,
            auto_filename: false,
            random_filename: false,
            output_filename: None,
            source_image: None,
            ref_images: Vec::new(),
        }
    }

    fn dalle_config() -> EngineConfig {
        EngineConfig {
            api_key: "sk".to_string(),
            base_url: Some("http://mock/v1".to_string()),
            model: Some("dall-e-3".to_string()),
        }
    }

    #[test]
    fn dalle3_body_has_quality_and_style() {
        let req = request_with_extra(HashMap::new());
        let body = build_images_body(&req, &dalle_config(), "dall-e-3");
        assert_eq!(body["model"], "dall-e-3");
        assert_eq!(body["prompt"], "a cat");
        assert_eq!(body["n"], 2);
        assert_eq!(body["size"], "1024x1024");
        assert_eq!(body["response_format"], "b64_json");
        assert_eq!(body["quality"], "standard");
        assert_eq!(body["style"], "vivid");
    }

    #[test]
    fn gpt_image_body_has_no_quality_style() {
        let cfg = EngineConfig {
            model: Some("gpt-image-1".to_string()),
            ..dalle_config()
        };
        let body = build_images_body(&request_with_extra(HashMap::new()), &cfg, "gpt-image-1");
        assert_eq!(body["model"], "gpt-image-1");
        assert!(body.get("quality").is_none());
        assert!(body.get("style").is_none());
    }

    #[test]
    fn stability_body_merges_extra_params() {
        let mut extra = HashMap::new();
        extra.insert("negative_prompt".into(), json!("blurry, text"));
        extra.insert("seed".into(), json!(42));
        extra.insert("aspect_ratio".into(), json!("16:9"));
        let cfg = EngineConfig {
            model: Some("stability.stable-image-core-v1:1".to_string()),
            ..dalle_config()
        };
        let body = build_images_body(
            &request_with_extra(extra),
            &cfg,
            "stability.stable-image-core-v1:1",
        );

        // n / response_format dropped for stability.
        assert!(body.get("n").is_none());
        assert!(body.get("response_format").is_none());
        // size dropped because aspect_ratio present.
        assert!(body.get("size").is_none());

        assert_eq!(body["negative_prompt"], "blurry, text");
        assert_eq!(body["seed"], 42);
        assert_eq!(body["aspect_ratio"], "16:9");
        // default mode injected for non-sd3 models.
        assert_eq!(body["mode"], "text-to-image");
    }

    #[test]
    fn stability_sd3_gets_no_default_mode() {
        let cfg = EngineConfig {
            model: Some("stability.sd3-large".to_string()),
            ..dalle_config()
        };
        let body = build_images_body(
            &request_with_extra(HashMap::new()),
            &cfg,
            "stability.sd3-large",
        );
        assert!(body.get("mode").is_none());
        assert!(body.get("n").is_none());
        // No size drop unless aspect_ratio given.
        assert_eq!(body["size"], "1024x1024");
    }

    #[test]
    fn edit_transport_allows_gpt_image_engine() {
        let cfg = EngineConfig {
            model: Some("gpt-image-1".to_string()),
            ..dalle_config()
        };
        assert_eq!(
            edit_transport("openai_gpt", &cfg).unwrap(),
            EditTransport::MultipartEdits
        );
    }

    #[test]
    fn edit_transport_allows_openrouter_base_url() {
        let cfg = EngineConfig {
            base_url: Some("https://openrouter.ai/api/v1".to_string()),
            model: Some("google/gemini-2.0-flash-image".to_string()),
            ..dalle_config()
        };
        assert_eq!(
            edit_transport("openrouter_vision", &cfg).unwrap(),
            EditTransport::ChatVision
        );
    }

    #[test]
    fn edit_transport_rejects_dall_e() {
        assert!(edit_transport("openai_dalle3", &dalle_config()).is_err());
    }

    #[test]
    fn edit_transport_rejects_stability() {
        let cfg = EngineConfig {
            model: Some("stability.sd3-large".to_string()),
            ..dalle_config()
        };
        assert!(edit_transport("stability", &cfg).is_err());
    }

    #[test]
    fn edit_transport_error_names_engine_and_model() {
        let cfg = EngineConfig {
            model: Some("stability.sd3-large".to_string()),
            ..dalle_config()
        };
        let err = edit_transport("my_stability_engine", &cfg)
            .unwrap_err()
            .to_string();
        assert!(err.contains("my_stability_engine"), "{err}");
        assert!(err.contains("stability.sd3-large"), "{err}");
        assert!(err.contains("cannot accept image input for edits"), "{err}");
    }

    #[test]
    fn resolve_url_defaults_to_openai() {
        assert_eq!(
            resolve_url(&None, "images/generations"),
            "https://api.openai.com/v1/images/generations"
        );
        assert_eq!(
            resolve_url(&Some("http://x/v1/".to_string()), "models"),
            "http://x/v1/models"
        );
    }
}
