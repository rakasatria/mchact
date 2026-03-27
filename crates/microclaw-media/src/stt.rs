use async_trait::async_trait;
use crate::MediaError;

#[async_trait]
pub trait SttProvider: Send + Sync {
    async fn transcribe(&self, audio_bytes: &[u8], mime_type: &str) -> Result<String, MediaError>;
    fn name(&self) -> &str;
}

pub struct SttRouter {
    provider: Box<dyn SttProvider>,
}

impl SttRouter {
    pub fn new(provider_name: &str, api_key: Option<&str>, model: &str) -> Result<Self, MediaError> {
        #[cfg(not(feature = "stt-local"))]
        let _ = model;
        let provider: Box<dyn SttProvider> = match provider_name {
            "openai" => {
                let key = api_key.ok_or_else(|| MediaError::NotConfigured("OpenAI STT requires api_key".into()))?;
                Box::new(crate::stt_openai::OpenAiSttProvider::new(key))
            }
            #[cfg(feature = "stt-local")]
            "whisper-local" => {
                Box::new(crate::stt_whisper::WhisperLocalProvider::new(model)?)
            }
            other => return Err(MediaError::NotConfigured(format!("Unknown STT provider: {other}"))),
        };
        Ok(Self { provider })
    }

    pub async fn transcribe(&self, audio_bytes: &[u8], mime_type: &str) -> Result<String, MediaError> {
        self.provider.transcribe(audio_bytes, mime_type).await
    }
}
