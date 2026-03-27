pub mod documents;
pub mod image_gen;
pub mod image_gen_fal;
pub mod image_gen_openai;
pub mod stt;
pub mod stt_openai;
pub mod tts;
pub mod tts_edge;
pub mod tts_elevenlabs;
pub mod tts_openai;
pub mod video_gen;
pub mod video_gen_fal;
pub mod video_gen_minimax;
pub mod video_gen_sora;

#[cfg(feature = "stt-local")]
pub mod stt_whisper;

pub mod audio_encode;

#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    #[error("Provider error: {0}")]
    ProviderError(String),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Timeout")]
    Timeout,
    #[error("Not configured: {0}")]
    NotConfigured(String),
    #[error("Disabled: {0}")]
    Disabled(String),
    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AudioFormat {
    Mp3,
    Wav,
    Opus,
    Ogg,
}
