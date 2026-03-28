use async_trait::async_trait;
use mchact_core::llm_types::ToolDefinition;
use mchact_media::image_gen::{ImageGenParams, ImageGenRouter};
use serde_json::json;
use std::path::PathBuf;

use super::{schema_object, Tool, ToolResult};

pub struct ImageGenerateTool {
    image_gen_provider: String,
    image_gen_api_key: Option<String>,
    image_gen_fal_key: Option<String>,
    default_size: String,
    default_quality: String,
    data_dir: String,
}

impl ImageGenerateTool {
    pub fn new(
        provider: &str,
        api_key: Option<&str>,
        fal_key: Option<&str>,
        default_size: &str,
        default_quality: &str,
        data_dir: &str,
    ) -> Self {
        Self {
            image_gen_provider: provider.to_string(),
            image_gen_api_key: api_key.map(String::from),
            image_gen_fal_key: fal_key.map(String::from),
            default_size: default_size.to_string(),
            default_quality: default_quality.to_string(),
            data_dir: data_dir.to_string(),
        }
    }
}

#[async_trait]
impl Tool for ImageGenerateTool {
    fn name(&self) -> &str {
        "image_generate"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "image_generate".into(),
            description: "Generate an image from a text prompt using the configured image generation provider. Returns the file path of the generated image.".into(),
            input_schema: schema_object(
                json!({
                    "prompt": {
                        "type": "string",
                        "description": "The text prompt describing the image to generate"
                    },
                    "size": {
                        "type": "string",
                        "description": "Image dimensions (e.g. '1024x1024', '1792x1024'). Uses the configured default if not specified."
                    },
                    "quality": {
                        "type": "string",
                        "description": "Image quality level (e.g. 'standard', 'hd'). Uses the configured default if not specified."
                    }
                }),
                &["prompt"],
            ),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let prompt = match input.get("prompt").and_then(|v| v.as_str()) {
            Some(p) if !p.trim().is_empty() => p.to_string(),
            _ => return ToolResult::error("Missing required parameter: prompt".to_string()),
        };

        let size = input
            .get("size")
            .and_then(|v| v.as_str())
            .filter(|v| !v.trim().is_empty())
            .unwrap_or(&self.default_size)
            .to_string();

        let quality = input
            .get("quality")
            .and_then(|v| v.as_str())
            .filter(|v| !v.trim().is_empty())
            .unwrap_or(&self.default_quality)
            .to_string();

        let router = match ImageGenRouter::new(
            &self.image_gen_provider,
            self.image_gen_api_key.as_deref(),
            self.image_gen_fal_key.as_deref(),
        ) {
            Ok(r) => r,
            Err(e) => {
                return ToolResult::error(format!(
                    "Failed to initialize image generation provider: {e}"
                ))
            }
        };

        let params = ImageGenParams {
            size: Some(size),
            quality: Some(quality),
            n: Some(1),
        };

        let output = match router.generate(&prompt, params).await {
            Ok(o) => o,
            Err(e) => return ToolResult::error(format!("Image generation failed: {e}")),
        };

        let image = match output.images.into_iter().next() {
            Some(img) => img,
            None => return ToolResult::error("No images were generated".to_string()),
        };

        let format = if image.format.is_empty() {
            "png".to_string()
        } else {
            image.format.clone()
        };

        let filename = format!("img_{}.{}", uuid::Uuid::new_v4(), format);
        let media_dir = PathBuf::from(&self.data_dir).join("media");

        if let Err(e) = std::fs::create_dir_all(&media_dir) {
            return ToolResult::error(format!("Failed to create media directory: {e}"));
        }

        let file_path = media_dir.join(&filename);
        if let Err(e) = std::fs::write(&file_path, &image.data) {
            return ToolResult::error(format!("Failed to save image file: {e}"));
        }

        ToolResult::success(file_path.to_string_lossy().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_definition() {
        let tool =
            ImageGenerateTool::new("openai", None, None, "1024x1024", "standard", "/tmp/data");
        assert_eq!(tool.name(), "image_generate");
        let def = tool.definition();
        assert_eq!(def.name, "image_generate");
        assert!(def.input_schema["properties"]["prompt"].is_object());
        assert!(def.input_schema["properties"]["size"].is_object());
        assert!(def.input_schema["properties"]["quality"].is_object());
        let required = def.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "prompt"));
        assert!(!required.iter().any(|v| v == "size"));
        assert!(!required.iter().any(|v| v == "quality"));
    }

    #[tokio::test]
    async fn test_missing_prompt_returns_error() {
        let tool =
            ImageGenerateTool::new("openai", None, None, "1024x1024", "standard", "/tmp/data");
        let result = tool.execute(json!({})).await;
        assert!(result.is_error);
        assert!(result.content.contains("Missing required parameter: prompt"));
    }

    #[tokio::test]
    async fn test_empty_prompt_returns_error() {
        let tool =
            ImageGenerateTool::new("openai", None, None, "1024x1024", "standard", "/tmp/data");
        let result = tool.execute(json!({"prompt": "  "})).await;
        assert!(result.is_error);
        assert!(result.content.contains("Missing required parameter: prompt"));
    }

    #[tokio::test]
    async fn test_invalid_provider_returns_error() {
        let tool = ImageGenerateTool::new(
            "nonexistent_provider",
            None,
            None,
            "1024x1024",
            "standard",
            "/tmp/data",
        );
        let result = tool
            .execute(json!({"prompt": "A beautiful landscape"}))
            .await;
        assert!(result.is_error);
        assert!(
            result
                .content
                .contains("Failed to initialize image generation provider")
        );
    }
}
