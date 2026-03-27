use async_trait::async_trait;
use microclaw_core::llm_types::ToolDefinition;
use microclaw_media::video_gen::{VideoGenParams, VideoGenRouter};
use serde_json::json;
use std::path::PathBuf;

use super::{schema_object, Tool, ToolResult};

pub struct VideoGenerateTool {
    video_gen_provider: String,
    video_gen_api_key: Option<String>,
    video_gen_fal_model: Option<String>,
    video_gen_minimax_key: Option<String>,
    video_gen_timeout_secs: u64,
    data_dir: String,
}

impl VideoGenerateTool {
    pub fn new(
        provider: &str,
        api_key: Option<&str>,
        fal_model: Option<&str>,
        minimax_key: Option<&str>,
        timeout_secs: u64,
        data_dir: &str,
    ) -> Self {
        Self {
            video_gen_provider: provider.to_string(),
            video_gen_api_key: api_key.map(String::from),
            video_gen_fal_model: fal_model.map(String::from),
            video_gen_minimax_key: minimax_key.map(String::from),
            video_gen_timeout_secs: timeout_secs,
            data_dir: data_dir.to_string(),
        }
    }
}

#[async_trait]
impl Tool for VideoGenerateTool {
    fn name(&self) -> &str {
        "video_generate"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "video_generate".into(),
            description: "Generate a video from a text prompt using the configured video generation provider. Returns the file path of the generated video.".into(),
            input_schema: schema_object(
                json!({
                    "prompt": {
                        "type": "string",
                        "description": "The text prompt describing the video to generate"
                    },
                    "duration": {
                        "type": "integer",
                        "description": "Desired video duration in seconds (optional, provider may have limits)"
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

        let duration_secs = input
            .get("duration")
            .and_then(|v| v.as_u64())
            .map(|d| d as u32);

        let router = match VideoGenRouter::new(
            &self.video_gen_provider,
            self.video_gen_api_key.as_deref(),
            self.video_gen_fal_model.as_deref(),
            self.video_gen_minimax_key.as_deref(),
            self.video_gen_timeout_secs,
        ) {
            Ok(r) => r,
            Err(e) => {
                return ToolResult::error(format!(
                    "Failed to initialize video generation provider: {e}"
                ))
            }
        };

        let params = VideoGenParams {
            duration_secs,
            resolution: None,
        };

        let output = match router.generate(&prompt, params).await {
            Ok(o) => o,
            Err(e) => return ToolResult::error(format!("Video generation failed: {e}")),
        };

        let format = if output.format.is_empty() {
            "mp4".to_string()
        } else {
            output.format.clone()
        };

        let filename = format!("vid_{}.{}", uuid::Uuid::new_v4(), format);
        let media_dir = PathBuf::from(&self.data_dir).join("media");

        if let Err(e) = std::fs::create_dir_all(&media_dir) {
            return ToolResult::error(format!("Failed to create media directory: {e}"));
        }

        let file_path = media_dir.join(&filename);
        if let Err(e) = std::fs::write(&file_path, &output.video_bytes) {
            return ToolResult::error(format!("Failed to save video file: {e}"));
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
        let tool = VideoGenerateTool::new("sora", None, None, None, 300, "/tmp/data");
        assert_eq!(tool.name(), "video_generate");
        let def = tool.definition();
        assert_eq!(def.name, "video_generate");
        assert!(def.input_schema["properties"]["prompt"].is_object());
        assert!(def.input_schema["properties"]["duration"].is_object());
        let required = def.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "prompt"));
        assert!(!required.iter().any(|v| v == "duration"));
    }

    #[tokio::test]
    async fn test_missing_prompt_returns_error() {
        let tool = VideoGenerateTool::new("sora", None, None, None, 300, "/tmp/data");
        let result = tool.execute(json!({})).await;
        assert!(result.is_error);
        assert!(result.content.contains("Missing required parameter: prompt"));
    }

    #[tokio::test]
    async fn test_empty_prompt_returns_error() {
        let tool = VideoGenerateTool::new("sora", None, None, None, 300, "/tmp/data");
        let result = tool.execute(json!({"prompt": "  "})).await;
        assert!(result.is_error);
        assert!(result.content.contains("Missing required parameter: prompt"));
    }

    #[tokio::test]
    async fn test_invalid_provider_returns_error() {
        let tool =
            VideoGenerateTool::new("nonexistent_provider", None, None, None, 300, "/tmp/data");
        let result = tool
            .execute(json!({"prompt": "A cinematic short film"}))
            .await;
        assert!(result.is_error);
        assert!(
            result
                .content
                .contains("Failed to initialize video generation provider")
        );
    }
}
