use async_trait::async_trait;
use crate::tts::{TtsOutput, TtsProvider, VoiceInfo};
use crate::{AudioFormat, MediaError};

pub struct OpenAiTtsProvider {
    api_key: String,
    http: reqwest::Client,
}

impl OpenAiTtsProvider {
    pub fn new(api_key: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            http: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl TtsProvider for OpenAiTtsProvider {
    async fn synthesize(&self, text: &str, voice: &str) -> Result<TtsOutput, MediaError> {
        let voice_name = if voice.is_empty() { "alloy" } else { voice };
        let body = serde_json::json!({
            "model": "tts-1",
            "input": text,
            "voice": voice_name,
            "response_format": "opus"
        });
        let response = self.http
            .post("https://api.openai.com/v1/audio/speech")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(MediaError::ProviderError(format!("OpenAI TTS {status}: {text}")));
        }

        let audio_bytes = response.bytes().await?.to_vec();
        Ok(TtsOutput { audio_bytes, format: AudioFormat::Opus, duration_ms: None })
    }

    fn name(&self) -> &str {
        "openai"
    }

    fn voices(&self) -> Vec<VoiceInfo> {
        vec![
            VoiceInfo { id: "alloy".into(), name: "Alloy".into(), language: None },
            VoiceInfo { id: "echo".into(), name: "Echo".into(), language: None },
            VoiceInfo { id: "fable".into(), name: "Fable".into(), language: None },
            VoiceInfo { id: "onyx".into(), name: "Onyx".into(), language: None },
            VoiceInfo { id: "nova".into(), name: "Nova".into(), language: None },
            VoiceInfo { id: "shimmer".into(), name: "Shimmer".into(), language: None },
        ]
    }
}
