use async_trait::async_trait;
use crate::tts::{TtsOutput, TtsProvider, VoiceInfo};
use crate::{AudioFormat, MediaError};

pub struct ElevenLabsTtsProvider {
    api_key: String,
    http: reqwest::Client,
}

impl ElevenLabsTtsProvider {
    pub fn new(api_key: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            http: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl TtsProvider for ElevenLabsTtsProvider {
    async fn synthesize(&self, text: &str, voice: &str) -> Result<TtsOutput, MediaError> {
        let voice_id = if voice.is_empty() { "21m00Tcm4TlvDq8ikWAM" } else { voice };
        let body = serde_json::json!({
            "text": text,
            "model_id": "eleven_multilingual_v2"
        });
        let url = format!("https://api.elevenlabs.io/v1/text-to-speech/{voice_id}");
        let response = self.http
            .post(&url)
            .header("xi-api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(MediaError::ProviderError(format!("ElevenLabs {status}: {text}")));
        }

        let audio_bytes = response.bytes().await?.to_vec();
        Ok(TtsOutput { audio_bytes, format: AudioFormat::Mp3, duration_ms: None })
    }

    fn name(&self) -> &str {
        "elevenlabs"
    }

    fn voices(&self) -> Vec<VoiceInfo> {
        vec![
            VoiceInfo { id: "21m00Tcm4TlvDq8ikWAM".into(), name: "Rachel".into(), language: None },
        ]
    }
}
