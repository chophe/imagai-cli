use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

/// A request to generate one or more images.
#[derive(Debug, Clone)]
pub struct ImageGenerationRequest {
    pub prompt: String,
    pub engine: String,
    pub output_filename: Option<String>,
    pub size: String,
    pub quality: String,
    pub n: u32,
    pub style: String,
    pub response_format: String,
    pub extra_params: HashMap<String, Value>,
    pub verbose: bool,
    pub auto_filename: bool,
    pub random_filename: bool,
}

impl Default for ImageGenerationRequest {
    fn default() -> Self {
        ImageGenerationRequest {
            prompt: String::new(),
            engine: String::new(),
            output_filename: None,
            size: "1024x1024".to_string(),
            quality: "standard".to_string(),
            n: 1,
            style: "vivid".to_string(),
            response_format: "b64_json".to_string(),
            extra_params: HashMap::new(),
            verbose: false,
            auto_filename: false,
            random_filename: false,
        }
    }
}

/// The result of generating a single image.
#[derive(Debug, Clone, Default)]
pub struct ImageGenerationResponse {
    pub image_url: Option<String>,
    pub image_b64_json: Option<String>,
    pub saved_path: Option<String>,
    pub error: Option<String>,
    pub usage: Option<Value>,
    pub estimated_cost: Option<Value>,
    /// Text-only responses (e.g. OpenRouter chat-based models).
    pub text_content: Option<String>,
}

/// Minimal structs for parsing OpenAI-compatible API responses.

#[derive(Debug, Deserialize)]
pub struct ImagesResponse {
    #[serde(default)]
    pub data: Vec<ImageData>,
    #[serde(default)]
    pub usage: Option<Value>,
    #[serde(default)]
    pub estimated_cost: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct ImageData {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub b64_json: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ChatResponse {
    #[serde(default)]
    pub choices: Vec<ChatChoice>,
    #[serde(default)]
    pub usage: Option<Value>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ChatChoice {
    pub message: ChatMessage,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ChatMessage {
    #[serde(default)]
    pub content: Option<Value>,
    #[serde(default)]
    pub images: Option<Vec<ChatImage>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ChatImage {
    #[serde(default)]
    pub image_url: Option<ChatImageUrl>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ChatImageUrl {
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ModelsListResponse {
    #[serde(default)]
    pub data: Vec<ModelInfo>,
}

#[derive(Debug, Deserialize)]
pub struct ModelInfo {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}
