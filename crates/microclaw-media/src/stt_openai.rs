use async_trait::async_trait;
use crate::stt::SttProvider;
use crate::MediaError;

pub struct OpenAiSttProvider {
    api_key: String,
    http: reqwest::Client,
}

impl OpenAiSttProvider {
    pub fn new(api_key: &str) -> Self {
        Self { api_key: api_key.to_string(), http: reqwest::Client::new() }
    }
}

#[async_trait]
impl SttProvider for OpenAiSttProvider {
    async fn transcribe(&self, audio_bytes: &[u8], mime_type: &str) -> Result<String, MediaError> {
        let ext = match mime_type {
            "audio/ogg" | "audio/opus" => "ogg",
            "audio/webm" => "webm",
            "audio/wav" | "audio/x-wav" => "wav",
            "audio/mpeg" | "audio/mp3" => "mp3",
            _ => "ogg",
        };
        let filename = format!("audio.{ext}");
        let part = reqwest::multipart::Part::bytes(audio_bytes.to_vec())
            .file_name(filename)
            .mime_str(mime_type)
            .map_err(|e| MediaError::ProviderError(format!("MIME error: {e}")))?;

        let form = reqwest::multipart::Form::new()
            .text("model", "whisper-1")
            .part("file", part);

        let response = self.http
            .post("https://api.openai.com/v1/audio/transcriptions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(MediaError::ProviderError(format!("OpenAI STT {status}: {text}")));
        }

        let body: serde_json::Value = response.json().await?;
        let text = body.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string();
        Ok(text)
    }

    fn name(&self) -> &str { "openai" }
}
