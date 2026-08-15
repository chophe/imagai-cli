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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_images_response_with_b64() {
        let json = r#"{
            "data": [{"b64_json": "QUJD"}, {"url": "https://x/y.png"}],
            "usage": {"total_tokens": 10},
            "estimated_cost": {"total": "0.04"}
        }"#;
        let parsed: ImagesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.data.len(), 2);
        assert_eq!(parsed.data[0].b64_json.as_deref(), Some("QUJD"));
        assert_eq!(parsed.data[0].url, None);
        assert_eq!(parsed.data[1].url.as_deref(), Some("https://x/y.png"));
        assert_eq!(parsed.usage.unwrap()["total_tokens"], 10);
        assert_eq!(parsed.estimated_cost.unwrap()["total"], "0.04");
    }

    #[test]
    fn images_response_defaults_missing_fields() {
        let parsed: ImagesResponse = serde_json::from_str(r#"{"data": []}"#).unwrap();
        assert!(parsed.data.is_empty());
        assert_eq!(parsed.usage, None);
        assert_eq!(parsed.estimated_cost, None);
    }

    #[test]
    fn parses_chat_response_text() {
        let json =
            r#"{"choices": [{"message": {"content": "hello"}}], "usage": {"total_tokens": 3}}"#;
        let parsed: ChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(
            parsed.choices[0].message.content.as_ref().unwrap().as_str(),
            Some("hello")
        );
        assert_eq!(parsed.usage.unwrap()["total_tokens"], 3);
    }

    #[test]
    fn parses_chat_response_images() {
        let json = r#"{"choices": [{"message": {
            "content": [{"type": "text", "text": "here"}],
            "images": [{"image_url": {"url": "https://x/img.png"}}]
        }}]}"#;
        let parsed: ChatResponse = serde_json::from_str(json).unwrap();
        let msg = &parsed.choices[0].message;
        assert_eq!(msg.content.as_ref().unwrap()[0]["text"], "here");
        let img = msg.images.as_ref().unwrap()[0].image_url.as_ref().unwrap();
        assert_eq!(img.url.as_deref(), Some("https://x/img.png"));
    }

    #[test]
    fn parses_models_list() {
        let json = r#"{"data": [{"id": "dall-e-3"}, {"id": "gpt-4o"}]}"#;
        let parsed: ModelsListResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.data[0].id.as_deref(), Some("dall-e-3"));
        assert_eq!(parsed.data[1].id.as_deref(), Some("gpt-4o"));
    }

    #[test]
    fn request_defaults() {
        let req = ImageGenerationRequest::default();
        assert_eq!(req.size, "1024x1024");
        assert_eq!(req.quality, "standard");
        assert_eq!(req.style, "vivid");
        assert_eq!(req.n, 1);
        assert_eq!(req.response_format, "b64_json");
    }
}
