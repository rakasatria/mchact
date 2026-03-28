use async_trait::async_trait;
use std::time::Duration;
use crate::video_gen::{VideoGenProvider, VideoGenParams, VideoGenOutput, poll_until_ready};
use crate::MediaError;

pub struct SoraProvider {
    api_key: String,
    http: reqwest::Client,
    timeout: Duration,
}

impl SoraProvider {
    pub fn new(api_key: &str, timeout_secs: u64) -> Self {
        Self {
            api_key: api_key.to_string(),
            http: reqwest::Client::new(),
            timeout: Duration::from_secs(timeout_secs),
        }
    }
}

#[async_trait]
impl VideoGenProvider for SoraProvider {
    async fn generate(&self, prompt: &str, params: VideoGenParams) -> Result<VideoGenOutput, MediaError> {
        let duration = params.duration_secs.unwrap_or(5);
        let body = serde_json::json!({
            "model": "sora-2",
            "prompt": prompt,
            "duration": duration,
            "size": params.resolution.as_deref().unwrap_or("1280x720"),
        });

        let response = self.http
            .post("https://api.openai.com/v1/videos/generations")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await?;

        if response.status().as_u16() == 404 || response.status().as_u16() == 410 {
            return Err(MediaError::ProviderError(
                "Sora 2 API is not available. Try 'fal' or 'minimax' provider.".into(),
            ));
        }

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(MediaError::ProviderError(format!("Sora {status}: {text}")));
        }

        let result: serde_json::Value = response.json().await?;
        let video_id = result.get("id").and_then(|i| i.as_str()).unwrap_or("");
        let status_url = format!("https://api.openai.com/v1/videos/{video_id}");

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Authorization",
            format!("Bearer {}", self.api_key).parse().unwrap(),
        );

        let completed = poll_until_ready(&self.http, &status_url, headers, self.timeout).await?;

        let video_url = completed
            .get("url")
            .and_then(|u| u.as_str())
            .ok_or_else(|| MediaError::ProviderError("No video URL in response".into()))?;

        let video_bytes = self.http.get(video_url).send().await?.bytes().await?.to_vec();

        Ok(VideoGenOutput {
            video_bytes,
            format: "mp4".into(),
            duration_secs: duration as f32,
        })
    }

    fn name(&self) -> &str {
        "sora"
    }
}
