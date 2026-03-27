use async_trait::async_trait;
use crate::MediaError;

#[derive(Debug, Clone, Default)]
pub struct ImageGenParams {
    pub size: Option<String>,
    pub quality: Option<String>,
    pub n: Option<u32>,
}

#[derive(Debug)]
pub struct GeneratedImage {
    pub data: Vec<u8>,
    pub format: String,
    pub revised_prompt: Option<String>,
}

#[derive(Debug)]
pub struct ImageGenOutput {
    pub images: Vec<GeneratedImage>,
}

#[async_trait]
pub trait ImageGenProvider: Send + Sync {
    async fn generate(&self, prompt: &str, params: ImageGenParams) -> Result<ImageGenOutput, MediaError>;
    fn name(&self) -> &str;
}

pub struct ImageGenRouter {
    provider: Box<dyn ImageGenProvider>,
}

impl ImageGenRouter {
    pub fn new(provider_name: &str, api_key: Option<&str>, fal_key: Option<&str>) -> Result<Self, MediaError> {
        let provider: Box<dyn ImageGenProvider> = match provider_name {
            "openai" => {
                let key = api_key.ok_or_else(|| MediaError::NotConfigured("DALL-E requires api_key".into()))?;
                Box::new(crate::image_gen_openai::DalleProvider::new(key))
            }
            "flux" => {
                let key = fal_key.ok_or_else(|| MediaError::NotConfigured("FAL FLUX requires fal_key".into()))?;
                Box::new(crate::image_gen_fal::FalFluxProvider::new(key))
            }
            other => return Err(MediaError::NotConfigured(format!("Unknown image gen provider: {other}"))),
        };
        Ok(Self { provider })
    }

    pub async fn generate(&self, prompt: &str, params: ImageGenParams) -> Result<ImageGenOutput, MediaError> {
        self.provider.generate(prompt, params).await
    }
}
