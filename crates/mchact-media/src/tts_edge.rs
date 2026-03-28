use async_trait::async_trait;
use crate::tts::{TtsOutput, TtsProvider, VoiceInfo};
use crate::MediaError;
#[cfg(feature = "tts")]
use crate::AudioFormat;

pub struct EdgeTtsProvider;

impl EdgeTtsProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for EdgeTtsProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TtsProvider for EdgeTtsProvider {
    async fn synthesize(&self, text: &str, voice: &str) -> Result<TtsOutput, MediaError> {
        #[cfg(feature = "tts")]
        {
            use msedge_tts::tts::client::connect;
            use msedge_tts::tts::SpeechConfig;
            let mut tts = connect()
                .map_err(|e| MediaError::ProviderError(format!("Edge TTS connect failed: {e}")))?;
            let config = SpeechConfig {
                voice_name: voice.to_string(),
                audio_format: "audio-24khz-48kbitrate-mono-mp3".to_string(),
                pitch: 0,
                rate: 0,
                volume: 0,
            };
            let audio = tts.synthesize(text, &config)
                .map_err(|e| MediaError::ProviderError(format!("Edge TTS synthesis failed: {e}")))?;
            let audio_bytes: Vec<u8> = audio.audio_bytes;
            Ok(TtsOutput { audio_bytes, format: AudioFormat::Mp3, duration_ms: None })
        }
        #[cfg(not(feature = "tts"))]
        {
            let _ = (text, voice);
            Err(MediaError::NotConfigured("Edge TTS requires 'tts' feature".into()))
        }
    }

    fn name(&self) -> &str {
        "edge"
    }

    fn voices(&self) -> Vec<VoiceInfo> {
        vec![
            VoiceInfo { id: "en-US-AriaNeural".into(), name: "Aria".into(), language: Some("en-US".into()) },
            VoiceInfo { id: "en-US-GuyNeural".into(), name: "Guy".into(), language: Some("en-US".into()) },
            VoiceInfo { id: "en-GB-SoniaNeural".into(), name: "Sonia".into(), language: Some("en-GB".into()) },
        ]
    }
}
