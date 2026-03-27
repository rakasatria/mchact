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
        let body = serde_json::json!({
            "prompt": prompt,
            "image_size": params.size.as_deref().unwrap_or("landscape_4_3"),
            "num_images": params.n.unwrap_or(1),
        });

        let response = self.http
            .post("https://fal.run/fal-ai/flux/schnell")
            .header("Authorization", format!("Key {}", self.api_key))
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(MediaError::ProviderError(format!("FAL FLUX {status}: {text}")));
        }

        let result: serde_json::Value = response.json().await?;
        let image_list = result
            .get("images")
            .and_then(|i| i.as_array())
            .cloned()
            .unwrap_or_default();

        let mut images = Vec::new();
        for item in &image_list {
            if let Some(url) = item.get("url").and_then(|u| u.as_str()) {
                let bytes = self.http.get(url).send().await?.bytes().await?.to_vec();
                images.push(GeneratedImage { data: bytes, format: "jpeg".into(), revised_prompt: None });
            }
        }

        Ok(ImageGenOutput { images })
    }

    fn name(&self) -> &str { "flux" }
}
