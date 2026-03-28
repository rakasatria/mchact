use async_trait::async_trait;
use crate::image_gen::{ImageGenProvider, ImageGenParams, ImageGenOutput, GeneratedImage};
use crate::MediaError;

pub struct DalleProvider {
    api_key: String,
    http: reqwest::Client,
}

impl DalleProvider {
    pub fn new(api_key: &str) -> Self {
        Self { api_key: api_key.to_string(), http: reqwest::Client::new() }
    }
}

#[async_trait]
impl ImageGenProvider for DalleProvider {
    async fn generate(&self, prompt: &str, params: ImageGenParams) -> Result<ImageGenOutput, MediaError> {
        let size = params.size.as_deref().unwrap_or("1024x1024");
        let quality = params.quality.as_deref().unwrap_or("standard");
        let n = params.n.unwrap_or(1);

        let body = serde_json::json!({
            "model": "gpt-image-1",
            "prompt": prompt,
            "n": n,
            "size": size,
            "quality": quality,
        });

        let response = self.http
            .post("https://api.openai.com/v1/images/generations")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(MediaError::ProviderError(format!("DALL-E {status}: {text}")));
        }

        let result: serde_json::Value = response.json().await?;
        let data = result.get("data").and_then(|d| d.as_array()).cloned().unwrap_or_default();

        let mut images = Vec::new();
        for item in &data {
            if let Some(b64) = item.get("b64_json").and_then(|b| b.as_str()) {
                use base64::Engine;
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(b64)
                    .map_err(|e| MediaError::ProviderError(format!("Base64 decode failed: {e}")))?;
                let revised = item.get("revised_prompt").and_then(|p| p.as_str()).map(String::from);
                images.push(GeneratedImage { data: bytes, format: "png".into(), revised_prompt: revised });
            }
        }

        Ok(ImageGenOutput { images })
    }

    fn name(&self) -> &str { "openai" }
}
