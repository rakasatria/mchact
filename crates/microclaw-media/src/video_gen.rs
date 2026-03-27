use async_trait::async_trait;
use std::time::{Duration, Instant};
use crate::MediaError;

#[derive(Debug, Clone, Default)]
pub struct VideoGenParams {
    pub duration_secs: Option<u32>,
    pub resolution: Option<String>,
}

#[derive(Debug)]
pub struct VideoGenOutput {
    pub video_bytes: Vec<u8>,
    pub format: String,
    pub duration_secs: f32,
}

#[async_trait]
pub trait VideoGenProvider: Send + Sync {
    async fn generate(&self, prompt: &str, params: VideoGenParams) -> Result<VideoGenOutput, MediaError>;
    fn name(&self) -> &str;
}

pub async fn poll_until_ready(
    client: &reqwest::Client,
    status_url: &str,
    headers: reqwest::header::HeaderMap,
    timeout: Duration,
) -> Result<serde_json::Value, MediaError> {
    let deadline = Instant::now() + timeout;
    loop {
        if Instant::now() > deadline {
            return Err(MediaError::Timeout);
        }
        let resp = client.get(status_url).headers(headers.clone()).send().await?;
        let body: serde_json::Value = resp.json().await?;
        let status = body.get("status").and_then(|s| s.as_str()).unwrap_or("");
        match status {
            "completed" | "succeeded" | "Completed" | "Success" => return Ok(body),
            "failed" | "error" | "Failed" | "Error" => {
                return Err(MediaError::ProviderError(format!("Video generation failed: {}", body)));
            }
            _ => {
                tracing::debug!("Video gen status: {status}, polling again...");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}

pub struct VideoGenRouter {
    provider: Box<dyn VideoGenProvider>,
}

impl VideoGenRouter {
    pub fn new(
        provider_name: &str,
        api_key: Option<&str>,
        fal_model: Option<&str>,
        minimax_key: Option<&str>,
        timeout_secs: u64,
    ) -> Result<Self, MediaError> {
        let provider: Box<dyn VideoGenProvider> = match provider_name {
            "sora" => {
                let key = api_key.ok_or_else(|| MediaError::NotConfigured("Sora requires api_key".into()))?;
                Box::new(crate::video_gen_sora::SoraProvider::new(key, timeout_secs))
            }
            "fal" => {
                let key = api_key.ok_or_else(|| MediaError::NotConfigured("FAL video requires api_key".into()))?;
                let model = fal_model.unwrap_or("cogvideox");
                Box::new(crate::video_gen_fal::FalVideoProvider::new(key, model, timeout_secs))
            }
            "minimax" => {
                let key = minimax_key.ok_or_else(|| MediaError::NotConfigured("MiniMax requires minimax_key".into()))?;
                Box::new(crate::video_gen_minimax::MiniMaxProvider::new(key, timeout_secs))
            }
            other => return Err(MediaError::NotConfigured(format!("Unknown video gen provider: {other}"))),
        };
        Ok(Self { provider })
    }

    pub async fn generate(&self, prompt: &str, params: VideoGenParams) -> Result<VideoGenOutput, MediaError> {
        self.provider.generate(prompt, params).await
    }
}
