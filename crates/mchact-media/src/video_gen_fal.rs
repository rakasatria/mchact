use async_trait::async_trait;
use std::time::Duration;
use crate::video_gen::{VideoGenProvider, VideoGenParams, VideoGenOutput, poll_until_ready};
use crate::MediaError;

pub struct FalVideoProvider {
    api_key: String,
    model: String,
    http: reqwest::Client,
    timeout: Duration,
}

impl FalVideoProvider {
    pub fn new(api_key: &str, model: &str, timeout_secs: u64) -> Self {
        Self {
            api_key: api_key.to_string(),
            model: model.to_string(),
            http: reqwest::Client::new(),
            timeout: Duration::from_secs(timeout_secs),
        }
    }
}

#[async_trait]
impl VideoGenProvider for FalVideoProvider {
    async fn generate(&self, prompt: &str, params: VideoGenParams) -> Result<VideoGenOutput, MediaError> {
        let body = serde_json::json!({
            "prompt": prompt,
            "duration": params.duration_secs.unwrap_or(5),
        });

        let url = format!("https://queue.fal.run/fal-ai/{}", self.model);
        let response = self.http
            .post(&url)
            .header("Authorization", format!("Key {}", self.api_key))
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(MediaError::ProviderError(format!("FAL video {status}: {text}")));
        }

        let result: serde_json::Value = response.json().await?;

        if let Some(video_url) = result
            .get("video")
            .and_then(|v| v.get("url"))
            .and_then(|u| u.as_str())
        {
            let video_bytes = self.http.get(video_url).send().await?.bytes().await?.to_vec();
            return Ok(VideoGenOutput {
                video_bytes,
                format: "mp4".into(),
                duration_secs: params.duration_secs.unwrap_or(5) as f32,
            });
        }

        let request_id = result
            .get("request_id")
            .and_then(|r| r.as_str())
            .ok_or_else(|| MediaError::ProviderError("No request_id or video in FAL response".into()))?;

        let status_url = format!("{}/requests/{}", url, request_id);
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Authorization",
            format!("Key {}", self.api_key).parse().unwrap(),
        );

        let completed = poll_until_ready(&self.http, &status_url, headers, self.timeout).await?;

        let video_url = completed
            .get("video")
            .and_then(|v| v.get("url"))
            .and_then(|u| u.as_str())
            .ok_or_else(|| MediaError::ProviderError("No video URL in completed response".into()))?;

        let video_bytes = self.http.get(video_url).send().await?.bytes().await?.to_vec();

        Ok(VideoGenOutput {
            video_bytes,
            format: "mp4".into(),
            duration_secs: params.duration_secs.unwrap_or(5) as f32,
        })
    }

    fn name(&self) -> &str {
        "fal"
    }
}
