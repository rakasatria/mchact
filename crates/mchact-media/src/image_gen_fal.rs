use async_trait::async_trait;
use crate::image_gen::{ImageGenProvider, ImageGenParams, ImageGenOutput, GeneratedImage};
use crate::MediaError;

pub struct FalFluxProvider {
    api_key: String,
    http: reqwest::Client,
}

impl FalFluxProvider {
    pub fn new(api_key: &str) -> Self {
        Self { api_key: api_key.to_string(), http: reqwest::Client::new() }
    }
}

#[async_trait]
impl ImageGenProvider for FalFluxProvider {
    async fn generate(&self, prompt: &str, params: ImageGenParams) -> Result<ImageGenOutput, MediaError> {
        let image_size = match params.size.as_deref() {
            Some("1024x1024") | None => "square_hd",
            Some("1792x1024") => "landscape_16_9",
            Some("1024x1792") => "portrait_16_9",
            Some(other) => other,
        };

        let body = serde_json::json!({
            "prompt": prompt,
            "image_size": image_size,
            "num_images": params.n.unwrap_or(1),
            "enable_safety_checker": true,
        });

        let response = self.http
            .post("https://queue.fal.run/fal-ai/flux/schnell")
            .header("Authorization", format!("Key {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(MediaError::ProviderError(format!("FAL FLUX {status}: {text}")));
        }

        let result: serde_json::Value = response.json().await?;
        let fal_images = result.get("images").and_then(|i| i.as_array()).cloned().unwrap_or_default();

        let mut images = Vec::new();
        for item in &fal_images {
            if let Some(url) = item.get("url").and_then(|u| u.as_str()) {
                let img_response = self.http.get(url).send().await?;
                let bytes = img_response.bytes().await?.to_vec();
                let content_type = item.get("content_type").and_then(|c| c.as_str()).unwrap_or("image/jpeg");
                let format = if content_type.contains("png") { "png" } else { "jpeg" };
                images.push(GeneratedImage { data: bytes, format: format.into(), revised_prompt: None });
            }
        }

        Ok(ImageGenOutput { images })
    }

    fn name(&self) -> &str { "flux" }
}
