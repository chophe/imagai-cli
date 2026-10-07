use std::path::Path;

use crate::config::Settings;
use crate::models::{ImageGenerationRequest, ImageGenerationResponse};
use crate::provider;
use crate::utils::{
    generate_filename, generate_filename_from_prompt_llm, generate_random_filename,
    get_image_extension, save_image_from_b64, save_image_from_url,
};

/// Generate images for a request, saving them to the configured output dir.
pub async fn generate_image_core(
    request: &ImageGenerationRequest,
    settings: &Settings,
) -> Vec<ImageGenerationResponse> {
    let Some(engine_config) = settings.get_engine(&request.engine) else {
        return vec![ImageGenerationResponse {
            error: Some(format!(
                "Engine '{}' is not configured. Available engines: {}",
                request.engine,
                settings.engine_names().join(", ")
            )),
            ..Default::default()
        }];
    };

    // An edit request (`source_image` set) goes down the edits transport; every
    // other request takes the generation path exactly as before.
    let model = engine_config
        .model
        .clone()
        .unwrap_or_else(|| "dall-e-3".to_string());
    let api_responses = if request.source_image.is_none() {
        provider::generate_images(request, engine_config).await
    } else {
        // Capability gate first: an incapable engine must fail before a socket
        // is opened, never after credits are spent.
        match provider::edit_transport(&request.engine, engine_config) {
            Ok(_) => provider::generate_edits(request, engine_config, &model).await,
            Err(e) => vec![ImageGenerationResponse {
                error: Some(e.to_string()),
                ..Default::default()
            }],
        }
    };

    let mut final_responses = Vec::with_capacity(api_responses.len());
    for (i, mut api_response) in api_responses.into_iter().enumerate() {
        if api_response.error.is_some() {
            final_responses.push(api_response);
            continue;
        }

        // Text-only response (e.g. OpenRouter chat-based model): skip saving.
        if api_response.text_content.is_some()
            && api_response.image_url.is_none()
            && api_response.image_b64_json.is_none()
        {
            final_responses.push(api_response);
            continue;
        }

        let base_filename = request.output_filename.clone();
        let mut output_ext = "png".to_string();
        let current_filename = if let Some(base) = &base_filename {
            output_ext = get_image_extension(Path::new(base));
            if request.n > 1 {
                let stem = Path::new(base)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("image");
                format!("{stem}_{}.{output_ext}", i + 1)
            } else {
                base.clone()
            }
        } else if let Some(source) = request.source_image.as_deref() {
            // D-06: results land in the same flat output_dir, named after the
            // source rather than the prompt.
            let source_path = Path::new(source);
            output_ext = get_image_extension(source_path);
            let stem = source_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("image");
            let base = format!("{stem}-edit.{output_ext}");
            if request.n > 1 {
                suffix_numbered(&base, i, request.n)
            } else {
                base
            }
        } else if request.auto_filename {
            let f = generate_filename_from_prompt_llm(
                settings,
                &request.prompt,
                &output_ext,
                request.verbose,
            )
            .await;
            suffix_numbered(&f, i, request.n)
        } else if request.random_filename {
            suffix_numbered(&generate_random_filename(&output_ext), i, request.n)
        } else {
            generate_filename(Some(&request.prompt), &output_ext)
        };

        let output_file_path = settings.output_dir.join(&current_filename);
        let model_name = engine_config
            .model
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        // Lineage records the source's filename only — never the path it came from.
        let source_filename = request
            .source_image
            .as_deref()
            .and_then(|s| Path::new(s).file_name())
            .and_then(|n| n.to_str());
        let client = match provider::http_client() {
            Ok(c) => c,
            Err(e) => {
                api_response.error = Some(format!("failed to build HTTP client: {e}"));
                final_responses.push(api_response);
                continue;
            }
        };

        let saved_path = if let Some(url) = &api_response.image_url {
            save_image_from_url(
                &client,
                url,
                &output_file_path,
                Some(&request.prompt),
                Some(&model_name),
                source_filename,
            )
            .await
            .ok()
        } else if let Some(b64) = &api_response.image_b64_json {
            save_image_from_b64(
                b64,
                &output_file_path,
                Some(&request.prompt),
                Some(&model_name),
                source_filename,
            )
            .await
            .ok()
        } else {
            None
        };

        if let Some(path) = saved_path {
            api_response.saved_path = Some(path.to_string_lossy().to_string());
        } else if api_response.error.is_none() {
            api_response.error = Some(format!("Failed to save image to {output_file_path:?}"));
        }
        final_responses.push(api_response);
    }

    final_responses
}

/// Add a `_<i+1>` suffix before the extension when generating multiple images.
fn suffix_numbered(filename: &str, index: usize, total: u32) -> String {
    if total <= 1 {
        return filename.to_string();
    }
    let path = Path::new(filename);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("image");
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("png");
    format!("{stem}_{}.{ext}", index + 1)
}
