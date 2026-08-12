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

    let api_responses = provider::generate_images(request, engine_config).await;

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
            )
            .await
            .ok()
        } else if let Some(b64) = &api_response.image_b64_json {
            save_image_from_b64(
                b64,
                &output_file_path,
                Some(&request.prompt),
                Some(&model_name),
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
