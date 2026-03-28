use async_trait::async_trait;
use mchact_core::llm_types::ToolDefinition;
use mchact_media::video_gen::{VideoGenParams, VideoGenRouter};
use serde_json::json;
use std::sync::Arc;

use super::{auth_context_from_input, schema_object, Tool, ToolResult};

pub struct VideoGenerateTool {
    video_gen_provider: String,
    video_gen_api_key: Option<String>,
    video_gen_fal_model: Option<String>,
    video_gen_minimax_key: Option<String>,
    video_gen_timeout_secs: u64,
    media_manager: Arc<crate::media_manager::MediaManager>,
}

impl VideoGenerateTool {
    pub fn new(
        provider: &str,
        api_key: Option<&str>,
        fal_model: Option<&str>,
        minimax_key: Option<&str>,
        timeout_secs: u64,
        media_manager: Arc<crate::media_manager::MediaManager>,
    ) -> Self {
        Self {
            video_gen_provider: provider.to_string(),
            video_gen_api_key: api_key.map(String::from),
            video_gen_fal_model: fal_model.map(String::from),
            video_gen_minimax_key: minimax_key.map(String::from),
            video_gen_timeout_secs: timeout_secs,
            media_manager,
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
        let mime = format!("video/{format}");
        let auth = auth_context_from_input(&input);
        let chat_id = auth.map(|a| a.caller_chat_id).unwrap_or(0);

        let media_id = match self
            .media_manager
            .store_file(output.video_bytes, &filename, Some(&mime), chat_id, "video_gen")
            .await
        {
            Ok(id) => id,
            Err(e) => return ToolResult::error(format!("Failed to store: {e}")),
        };

        ToolResult::success(
            serde_json::json!({"media_object_id": media_id, "type": "video"}).to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mchact_storage::db::Database;
    use mchact_storage_backend::local::LocalStorage;
    use serde_json::json;

    async fn make_media_manager() -> Arc<crate::media_manager::MediaManager> {
        let dir = std::env::temp_dir()
            .join(format!("mchact_vid_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let storage: Arc<dyn mchact_storage_backend::ObjectStorage> =
            Arc::new(LocalStorage::new(dir.to_str().unwrap()).await.unwrap());
        let db = Arc::new(Database::new(dir.to_str().unwrap()).unwrap());
        Arc::new(crate::media_manager::MediaManager::new(storage, db))
    }

    #[tokio::test]
    async fn test_definition() {
        let tool = VideoGenerateTool::new("sora", None, None, None, 300, make_media_manager().await);
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
        let tool = VideoGenerateTool::new("sora", None, None, None, 300, make_media_manager().await);
        let result = tool.execute(json!({})).await;
        assert!(result.is_error);
        assert!(result.content.contains("Missing required parameter: prompt"));
    }

    #[tokio::test]
    async fn test_empty_prompt_returns_error() {
        let tool = VideoGenerateTool::new("sora", None, None, None, 300, make_media_manager().await);
        let result = tool.execute(json!({"prompt": "  "})).await;
        assert!(result.is_error);
        assert!(result.content.contains("Missing required parameter: prompt"));
    }

    #[tokio::test]
    async fn test_invalid_provider_returns_error() {
        let tool = VideoGenerateTool::new(
            "nonexistent_provider",
            None,
            None,
            None,
            300,
            make_media_manager().await,
        );
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
