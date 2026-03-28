use async_trait::async_trait;
use crate::{AudioFormat, MediaError};

#[derive(Debug, Clone)]
pub struct VoiceInfo {
    pub id: String,
    pub name: String,
    pub language: Option<String>,
}

#[derive(Debug)]
pub struct TtsOutput {
    pub audio_bytes: Vec<u8>,
    pub format: AudioFormat,
    pub duration_ms: Option<u64>,
}

#[async_trait]
pub trait TtsProvider: Send + Sync {
    async fn synthesize(&self, text: &str, voice: &str) -> Result<TtsOutput, MediaError>;
    fn name(&self) -> &str;
    fn voices(&self) -> Vec<VoiceInfo>;
}

pub struct TtsRouter {
    provider: Box<dyn TtsProvider>,
}

impl TtsRouter {
    pub fn new(provider_name: &str, api_key: Option<&str>, _voice: &str) -> Result<Self, MediaError> {
        let provider: Box<dyn TtsProvider> = match provider_name {
            #[cfg(feature = "tts")]
            "edge" => Box::new(crate::tts_edge::EdgeTtsProvider::new()),
            "openai" => {
                let key = api_key.ok_or_else(|| MediaError::NotConfigured("OpenAI TTS requires api_key".into()))?;
                Box::new(crate::tts_openai::OpenAiTtsProvider::new(key))
            }
            "elevenlabs" => {
                let key = api_key.ok_or_else(|| MediaError::NotConfigured("ElevenLabs requires api_key".into()))?;
                Box::new(crate::tts_elevenlabs::ElevenLabsTtsProvider::new(key))
            }
            other => return Err(MediaError::NotConfigured(format!("Unknown TTS provider: {other}"))),
        };
        Ok(Self { provider })
    }

    pub async fn synthesize(&self, text: &str, voice: &str) -> Result<TtsOutput, MediaError> {
        self.provider.synthesize(text, voice).await
    }

    pub fn name(&self) -> &str {
        self.provider.name()
    }
}
