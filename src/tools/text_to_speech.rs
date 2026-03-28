use async_trait::async_trait;
use mchact_core::llm_types::ToolDefinition;
use mchact_media::{tts::TtsRouter, AudioFormat};
use serde_json::json;
use std::sync::Arc;

use super::{auth_context_from_input, schema_object, Tool, ToolResult};

pub struct TextToSpeechTool {
    tts_provider: String,
    tts_api_key: Option<String>,
    tts_voice: String,
    media_manager: Arc<crate::media_manager::MediaManager>,
}

impl TextToSpeechTool {
    pub fn new(
        provider: &str,
        api_key: Option<&str>,
        voice: &str,
        media_manager: Arc<crate::media_manager::MediaManager>,
    ) -> Self {
        Self {
            tts_provider: provider.to_string(),
            tts_api_key: api_key.map(String::from),
            tts_voice: voice.to_string(),
            media_manager,
        }
    }
}

fn audio_format_ext(format: AudioFormat) -> &'static str {
    match format {
        AudioFormat::Mp3 => "mp3",
        AudioFormat::Wav => "wav",
        AudioFormat::Opus => "opus",
        AudioFormat::Ogg => "ogg",
    }
}

#[async_trait]
impl Tool for TextToSpeechTool {
    fn name(&self) -> &str {
        "text_to_speech"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "text_to_speech".into(),
            description: "Convert text to speech audio using the configured TTS provider. Returns the file path of the generated audio file.".into(),
            input_schema: schema_object(
                json!({
                    "text": {
                        "type": "string",
                        "description": "The text to convert to speech"
                    },
                    "voice": {
                        "type": "string",
                        "description": "The voice ID or name to use (optional, uses default configured voice)"
                    }
                }),
                &["text"],
            ),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let text = match input.get("text").and_then(|v| v.as_str()) {
            Some(t) if !t.trim().is_empty() => t.to_string(),
            _ => return ToolResult::error("Missing required parameter: text".to_string()),
        };

        let voice = input
            .get("voice")
            .and_then(|v| v.as_str())
            .filter(|v| !v.trim().is_empty())
            .unwrap_or(&self.tts_voice)
            .to_string();

        let router = match TtsRouter::new(
            &self.tts_provider,
            self.tts_api_key.as_deref(),
            &voice,
        ) {
            Ok(r) => r,
            Err(e) => return ToolResult::error(format!("Failed to initialize TTS provider: {e}")),
        };

        let output = match router.synthesize(&text, &voice).await {
            Ok(o) => o,
            Err(e) => return ToolResult::error(format!("TTS synthesis failed: {e}")),
        };

        let ext = audio_format_ext(output.format);
        let filename = format!("tts_{}.{}", uuid::Uuid::new_v4(), ext);
        let mime = format!("audio/{ext}");
        let auth = auth_context_from_input(&input);
        let chat_id = auth.map(|a| a.caller_chat_id).unwrap_or(0);

        let media_id = match self
            .media_manager
            .store_file(output.audio_bytes, &filename, Some(&mime), chat_id, "tts")
            .await
        {
            Ok(id) => id,
            Err(e) => return ToolResult::error(format!("Failed to store: {e}")),
        };

        ToolResult::success(
            serde_json::json!({"media_object_id": media_id, "type": "audio"}).to_string(),
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
            .join(format!("mchact_tts_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let storage: Arc<dyn mchact_storage_backend::ObjectStorage> =
            Arc::new(LocalStorage::new(dir.to_str().unwrap()).await.unwrap());
        let db = Arc::new(Database::new(dir.to_str().unwrap()).unwrap());
        Arc::new(crate::media_manager::MediaManager::new(storage, db))
    }

    #[tokio::test]
    async fn test_definition() {
        let tool = TextToSpeechTool::new("openai", None, "alloy", make_media_manager().await);
        assert_eq!(tool.name(), "text_to_speech");
        let def = tool.definition();
        assert_eq!(def.name, "text_to_speech");
        assert!(def.input_schema["properties"]["text"].is_object());
        assert!(def.input_schema["properties"]["voice"].is_object());
        let required = def.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "text"));
        assert!(!required.iter().any(|v| v == "voice"));
    }

    #[tokio::test]
    async fn test_missing_text_returns_error() {
        let tool = TextToSpeechTool::new("openai", None, "alloy", make_media_manager().await);
        let result = tool.execute(json!({})).await;
        assert!(result.is_error);
        assert!(result.content.contains("Missing required parameter: text"));
    }

    #[tokio::test]
    async fn test_empty_text_returns_error() {
        let tool = TextToSpeechTool::new("openai", None, "alloy", make_media_manager().await);
        let result = tool.execute(json!({"text": "   "})).await;
        assert!(result.is_error);
        assert!(result.content.contains("Missing required parameter: text"));
    }

    #[tokio::test]
    async fn test_invalid_provider_returns_error() {
        let tool =
            TextToSpeechTool::new("nonexistent_provider", None, "alloy", make_media_manager().await);
        let result = tool.execute(json!({"text": "Hello world"})).await;
        assert!(result.is_error);
        assert!(result.content.contains("Failed to initialize TTS provider"));
    }

    #[test]
    fn test_audio_format_ext() {
        assert_eq!(audio_format_ext(AudioFormat::Mp3), "mp3");
        assert_eq!(audio_format_ext(AudioFormat::Wav), "wav");
        assert_eq!(audio_format_ext(AudioFormat::Opus), "opus");
        assert_eq!(audio_format_ext(AudioFormat::Ogg), "ogg");
    }
}
