#[cfg(feature = "stt-local")]
use async_trait::async_trait;
#[cfg(feature = "stt-local")]
use crate::stt::SttProvider;
#[cfg(feature = "stt-local")]
use crate::MediaError;

#[cfg(feature = "stt-local")]
pub struct WhisperLocalProvider {
    model_name: String,
}

#[cfg(feature = "stt-local")]
impl WhisperLocalProvider {
    pub fn new(model: &str) -> Result<Self, MediaError> {
        Ok(Self { model_name: model.to_string() })
    }
}

#[cfg(feature = "stt-local")]
#[async_trait]
impl SttProvider for WhisperLocalProvider {
    async fn transcribe(&self, audio_bytes: &[u8], _mime_type: &str) -> Result<String, MediaError> {
        let model_name = self.model_name.clone();
        let audio = audio_bytes.to_vec();

        tokio::task::spawn_blocking(move || {
            use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

            let model_path = format!("ggml-{}.bin", model_name);
            let ctx = WhisperContext::new_with_params(&model_path, WhisperContextParameters::default())
                .map_err(|e| MediaError::ProviderError(format!("Failed to load whisper model: {e}")))?;

            let mut state = ctx.create_state()
                .map_err(|e| MediaError::ProviderError(format!("Failed to create whisper state: {e}")))?;

            let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            params.set_language(Some("en"));
            params.set_print_special(false);
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);

            let samples: Vec<f32> = audio.chunks(2).map(|chunk| {
                if chunk.len() == 2 {
                    i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0
                } else {
                    0.0
                }
            }).collect();

            state.full(params, &samples)
                .map_err(|e| MediaError::ProviderError(format!("Whisper inference failed: {e}")))?;

            let num_segments = state.full_n_segments()
                .map_err(|e| MediaError::ProviderError(format!("Failed to get segments: {e}")))?;

            let mut text = String::new();
            for i in 0..num_segments {
                if let Ok(segment) = state.full_get_segment_text(i) {
                    text.push_str(&segment);
                    text.push(' ');
                }
            }

            Ok(text.trim().to_string())
        })
        .await
        .map_err(|e| MediaError::ProviderError(format!("Whisper task failed: {e}")))?
    }

    fn name(&self) -> &str { "whisper-local" }
}
