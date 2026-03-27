use async_trait::async_trait;
use std::time::Duration;
use crate::video_gen::{VideoGenProvider, VideoGenParams, VideoGenOutput, poll_until_ready};
use crate::MediaError;

pub struct MiniMaxProvider {
    api_key: String,
    http: reqwest::Client,
    timeout: Duration,
}

impl MiniMaxProvider {
    pub fn new(api_key: &str, timeout_secs: u64) -> Self {
        Self {
            api_key: api_key.to_string(),
            http: reqwest::Client::new(),
            timeout: Duration::from_secs(timeout_secs),
        }
    }
}

#[async_trait]
impl VideoGenProvider for MiniMaxProvider {
    async fn generate(&self, prompt: &str, params: VideoGenParams) -> Result<VideoGenOutput, MediaError> {
        let body = serde_json::json!({
            "model": "MiniMax-Hailuo-2.3",
            "prompt": prompt,
        });

        let response = self.http
            .post("https://api.minimax.io/v1/video_generation")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(MediaError::ProviderError(format!("MiniMax {status}: {text}")));
        }

        let result: serde_json::Value = response.json().await?;
        let task_id = result
            .get("task_id")
            .and_then(|t| t.as_str())
            .ok_or_else(|| MediaError::ProviderError("No task_id in MiniMax response".into()))?;

        let status_url = format!("https://api.minimax.io/v1/query/video_generation?task_id={task_id}");
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Authorization",
            format!("Bearer {}", self.api_key).parse().unwrap(),
        );

        let completed = poll_until_ready(&self.http, &status_url, headers, self.timeout).await?;

        let file_id = completed
            .get("file_id")
            .and_then(|f| f.as_str())
            .ok_or_else(|| MediaError::ProviderError("No file_id in completed response".into()))?;

        let file_url = format!("https://api.minimax.io/v1/files/retrieve?file_id={file_id}");
        let video_bytes = self.http
            .get(&file_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await?
            .bytes()
            .await?
            .to_vec();

        Ok(VideoGenOutput {
            video_bytes,
            format: "mp4".into(),
            duration_secs: params.duration_secs.unwrap_or(5) as f32,
        })
    }

    fn name(&self) -> &str {
        "minimax"
    }
}
