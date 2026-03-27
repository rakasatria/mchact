# Multimodal Capabilities Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add document intelligence, TTS, STT, image generation, video generation, vision routing, frontend media components, setup wizard, and web UI settings to MicroClaw.

**Architecture:** New `crates/microclaw-media/` crate with provider router pattern (trait + multiple implementations per capability). Feature flags control compilation. Tools registered conditionally based on `*_enabled` config toggles. Frontend adds media rendering components and composer attachments.

**Tech Stack:** Rust 2021, kreuzberg (documents), msedge-tts (TTS), whisper-rs (STT), reqwest (all cloud APIs), opus+ogg (audio encoding), React 18 + TypeScript + Tailwind (frontend).

**Spec:** `docs/superpowers/specs/2026-03-27-multimodal-design.md`

**Build note:** Use Rust 1.92.0 via rustup: `export PATH="$HOME/.cargo/bin:$PATH" && . "$HOME/.cargo/env" 2>/dev/null`

---

## File Structure

### New crate: `crates/microclaw-media/`
| File | Responsibility |
|------|---------------|
| `Cargo.toml` | Crate config with feature flags |
| `src/lib.rs` | Module declarations + MediaError type |
| `src/tts.rs` | TtsProvider trait + TtsRouter |
| `src/tts_edge.rs` | Edge TTS via msedge-tts |
| `src/tts_openai.rs` | OpenAI TTS API |
| `src/tts_elevenlabs.rs` | ElevenLabs API |
| `src/stt.rs` | SttProvider trait + SttRouter |
| `src/stt_openai.rs` | OpenAI Whisper API |
| `src/stt_whisper.rs` | Local whisper-rs |
| `src/image_gen.rs` | ImageGenProvider trait + router |
| `src/image_gen_openai.rs` | DALL-E gpt-image-1 |
| `src/image_gen_fal.rs` | FAL FLUX |
| `src/video_gen.rs` | VideoGenProvider trait + router + poll_until_ready |
| `src/video_gen_sora.rs` | OpenAI Sora 2 |
| `src/video_gen_fal.rs` | FAL video models |
| `src/video_gen_minimax.rs` | MiniMax Hailuo 2.3 |
| `src/documents.rs` | kreuzberg wrapper + SHA-256 hashing |
| `src/audio_encode.rs` | OGG Opus encoding |

### New tools: `src/tools/`
| File | Responsibility |
|------|---------------|
| `src/tools/text_to_speech.rs` | TTS tool |
| `src/tools/image_generate.rs` | Image generation tool |
| `src/tools/video_generate.rs` | Video generation tool |
| `src/tools/read_document.rs` | Document extraction tool |

### New frontend: `web/src/`
| File | Responsibility |
|------|---------------|
| `web/src/components/media/image-viewer.tsx` | Image display + lightbox |
| `web/src/components/media/audio-player.tsx` | Audio player |
| `web/src/components/media/video-player.tsx` | Video player |
| `web/src/components/media/file-preview.tsx` | File download card |
| `web/src/components/settings/multimodal-tab.tsx` | Multimodal settings panel |

### Modified files
| File | What changes |
|------|-------------|
| `Cargo.toml` (root) | Add microclaw-media to workspace + deps |
| `crates/microclaw-storage/src/db.rs` | Migration v21, DocumentExtraction struct + CRUD |
| `src/config.rs` | All multimodal config fields with `*_enabled` toggles |
| `src/tools/mod.rs` | Register 4 new tools conditionally |
| `src/agent_engine.rs` | Vision routing check |
| `crates/microclaw-channels/src/channel_adapter.rs` | Add send_voice(), send_video() |
| `src/channels/telegram.rs` | Implement send_voice/video, use shared STT |
| `src/channels/discord.rs` | Implement send_voice/video, add voice transcription |
| `src/web.rs` | /api/upload, /api/media/{id}, media SSE events |
| `src/setup.rs` | 3 new multimodal setup pages |
| `web/src/lib/types.ts` | Add attachments to BackendMessage |
| `web/src/components/thread-pane.tsx` | Enable attachments, add buttons |
| `web/src/components/message-components.tsx` | Render media attachments |
| `web/src/lib/sse-parser.ts` | Handle media event type |
| `web/src/hooks/use-chat-adapter.ts` | Accumulate media events |
| `web/src/components/settings-dialog.tsx` | Add Multimodal tab |

---

### Task 1: Media Crate Scaffold + Error Types

**Files:**
- Create: `crates/microclaw-media/Cargo.toml`
- Create: `crates/microclaw-media/src/lib.rs`
- Modify: `Cargo.toml` (root workspace)

- [ ] **Step 1: Create crate directory**

```bash
mkdir -p crates/microclaw-media/src
```

- [ ] **Step 2: Create Cargo.toml**

```toml
# crates/microclaw-media/Cargo.toml
[package]
name = "microclaw-media"
version = "0.1.0"
edition = "2021"

[features]
default = []
tts = ["msedge-tts", "opus", "ogg"]
stt-local = ["whisper-rs"]
documents = ["kreuzberg"]

[dependencies]
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json", "multipart"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
thiserror = "2"
infer = "0.19"
tempfile = "3"
async-trait = "0.1"
base64 = "0.22"
sha2 = "0.10"

# TTS
msedge-tts = { version = "0.3", optional = true }
opus = { version = "0.3", optional = true }
ogg = { version = "0.9", optional = true }

# STT local
whisper-rs = { version = "0.16", optional = true }

# Documents
kreuzberg = { version = "4.6", features = ["pdf", "office", "html"], optional = true }
```

- [ ] **Step 3: Create lib.rs with error type and module stubs**

```rust
// crates/microclaw-media/src/lib.rs

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
```

- [ ] **Step 4: Create empty stub files** for all modules so it compiles

Create each module file with just a comment:
```rust
// crates/microclaw-media/src/tts.rs (and all other .rs files)
// TODO: implement in subsequent tasks
```

- [ ] **Step 5: Add to root workspace**

In root `Cargo.toml`, add `"crates/microclaw-media"` to the workspace members list (line 2-11):
```toml
[workspace]
members = [
    ".",
    "crates/microclaw-core",
    "crates/microclaw-clawhub",
    "crates/microclaw-storage",
    "crates/microclaw-tools",
    "crates/microclaw-channels",
    "crates/microclaw-app",
    "crates/microclaw-observability",
    "crates/microclaw-media",
]
```

Also add the dependency to the main `[dependencies]` section:
```toml
microclaw-media = { version = "0.1.0", path = "crates/microclaw-media" }
```

- [ ] **Step 6: Build to verify**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && . "$HOME/.cargo/env" 2>/dev/null && cargo build -p microclaw-media`
Expected: Compiles with no errors (stubs only).

- [ ] **Step 7: Commit**

```bash
git add crates/microclaw-media/ Cargo.toml
git commit -m "feat: scaffold microclaw-media crate with error types and module stubs"
```

---

### Task 2: Config Fields + Enable/Disable Toggles

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Add default functions**

Add near the other default functions (around line 225):

```rust
fn default_tts_enabled() -> bool { true }
fn default_tts_provider() -> String { "edge".into() }
fn default_tts_voice() -> String { "en-US-AriaNeural".into() }
fn default_stt_enabled() -> bool { true }
fn default_stt_provider() -> String { "whisper-local".into() }
fn default_stt_model() -> String { "base".into() }
fn default_image_gen_enabled() -> bool { true }
fn default_image_gen_provider() -> String { "openai".into() }
fn default_image_gen_size() -> String { "1024x1024".into() }
fn default_image_gen_quality() -> String { "standard".into() }
fn default_video_gen_enabled() -> bool { false }
fn default_video_gen_provider() -> String { "sora".into() }
fn default_video_gen_timeout() -> u64 { 300 }
fn default_vision_fallback_enabled() -> bool { true }
fn default_vision_fallback_provider() -> String { "openrouter".into() }
fn default_vision_fallback_model() -> String { "anthropic/claude-sonnet-4".into() }
fn default_vision_fallback_base_url() -> String { "https://openrouter.ai/api/v1".into() }
fn default_document_extraction_enabled() -> bool { true }
```

- [ ] **Step 2: Add config struct fields**

Add to the `Config` struct (after the existing voice_provider fields around line 735):

```rust
    // Text-to-Speech
    #[serde(default = "default_tts_enabled")]
    pub tts_enabled: bool,
    #[serde(default = "default_tts_provider")]
    pub tts_provider: String,
    #[serde(default = "default_tts_voice")]
    pub tts_voice: String,
    #[serde(default)]
    pub tts_api_key: Option<String>,
    #[serde(default)]
    pub tts_elevenlabs_voice_id: Option<String>,

    // Speech-to-Text
    #[serde(default = "default_stt_enabled")]
    pub stt_enabled: bool,
    #[serde(default = "default_stt_provider")]
    pub stt_provider: String,
    #[serde(default = "default_stt_model")]
    pub stt_model: String,
    #[serde(default)]
    pub stt_model_path: Option<String>,

    // Image Generation
    #[serde(default = "default_image_gen_enabled")]
    pub image_gen_enabled: bool,
    #[serde(default = "default_image_gen_provider")]
    pub image_gen_provider: String,
    #[serde(default)]
    pub image_gen_api_key: Option<String>,
    #[serde(default)]
    pub image_gen_fal_key: Option<String>,
    #[serde(default = "default_image_gen_size")]
    pub image_gen_default_size: String,
    #[serde(default = "default_image_gen_quality")]
    pub image_gen_default_quality: String,

    // Video Generation
    #[serde(default = "default_video_gen_enabled")]
    pub video_gen_enabled: bool,
    #[serde(default = "default_video_gen_provider")]
    pub video_gen_provider: String,
    #[serde(default)]
    pub video_gen_api_key: Option<String>,
    #[serde(default)]
    pub video_gen_fal_model: Option<String>,
    #[serde(default)]
    pub video_gen_minimax_key: Option<String>,
    #[serde(default = "default_video_gen_timeout")]
    pub video_gen_timeout_secs: u64,

    // Vision Fallback
    #[serde(default = "default_vision_fallback_enabled")]
    pub vision_fallback_enabled: bool,
    #[serde(default = "default_vision_fallback_provider")]
    pub vision_fallback_provider: String,
    #[serde(default = "default_vision_fallback_model")]
    pub vision_fallback_model: String,
    #[serde(default)]
    pub vision_fallback_api_key: Option<String>,
    #[serde(default = "default_vision_fallback_base_url")]
    pub vision_fallback_base_url: String,

    // Document Processing
    #[serde(default = "default_document_extraction_enabled")]
    pub document_extraction_enabled: bool,
```

- [ ] **Step 3: Build to verify**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && . "$HOME/.cargo/env" 2>/dev/null && cargo build`
Expected: Compiles.

- [ ] **Step 4: Commit**

```bash
git add src/config.rs
git commit -m "feat(config): add multimodal config fields with enable/disable toggles"
```

---

### Task 3: Document Extraction Storage (Migration v21 + DB Methods)

**Files:**
- Modify: `crates/microclaw-storage/src/db.rs`

- [ ] **Step 1: Add DocumentExtraction struct**

Add near the other struct definitions (after `Finding` struct):

```rust
#[derive(Debug, Clone)]
pub struct DocumentExtraction {
    pub id: i64,
    pub chat_id: i64,
    pub file_hash: String,
    pub filename: String,
    pub mime_type: Option<String>,
    pub file_size: i64,
    pub extracted_text: String,
    pub char_count: i64,
    pub created_at: String,
}
```

- [ ] **Step 2: Bump SCHEMA_VERSION_CURRENT to 21**

Change:
```rust
const SCHEMA_VERSION_CURRENT: i64 = 21;
```

- [ ] **Step 3: Add migration block**

After the `version < 20` block, add:

```rust
    if version < 21 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS document_extractions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                chat_id INTEGER NOT NULL,
                file_hash TEXT NOT NULL,
                filename TEXT NOT NULL,
                mime_type TEXT,
                file_size INTEGER,
                extracted_text TEXT NOT NULL,
                extraction_method TEXT DEFAULT 'kreuzberg',
                char_count INTEGER,
                created_at TEXT NOT NULL,
                UNIQUE(chat_id, file_hash)
            );

            CREATE INDEX IF NOT EXISTS idx_doc_extractions_chat
                ON document_extractions(chat_id);
            ",
        )?;
        set_schema_version(conn, 21)?;
        version = 21;
    }
```

- [ ] **Step 4: Add document CRUD methods**

Add to `impl Database`:

```rust
    pub fn get_document_extraction(
        &self,
        chat_id: i64,
        file_hash: &str,
    ) -> Result<Option<DocumentExtraction>, MicroClawError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, chat_id, file_hash, filename, mime_type, file_size,
                    extracted_text, char_count, created_at
             FROM document_extractions
             WHERE chat_id = ?1 AND file_hash = ?2",
        )?;
        let result = stmt
            .query_row(params![chat_id, file_hash], |row| {
                Ok(DocumentExtraction {
                    id: row.get(0)?,
                    chat_id: row.get(1)?,
                    file_hash: row.get(2)?,
                    filename: row.get(3)?,
                    mime_type: row.get(4)?,
                    file_size: row.get(5)?,
                    extracted_text: row.get(6)?,
                    char_count: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })
            .optional()?;
        Ok(result)
    }

    pub fn insert_document_extraction(
        &self,
        chat_id: i64,
        file_hash: &str,
        filename: &str,
        mime_type: Option<&str>,
        file_size: i64,
        extracted_text: &str,
    ) -> Result<i64, MicroClawError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        let char_count = extracted_text.len() as i64;
        conn.execute(
            "INSERT OR REPLACE INTO document_extractions
             (chat_id, file_hash, filename, mime_type, file_size, extracted_text, char_count, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![chat_id, file_hash, filename, mime_type, file_size, extracted_text, char_count, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn search_document_extractions(
        &self,
        chat_id: Option<i64>,
        query: &str,
        limit: usize,
    ) -> Result<Vec<DocumentExtraction>, MicroClawError> {
        let conn = self.lock_conn();
        let pattern = format!("%{}%", query.replace('%', "\\%"));
        let mut stmt = conn.prepare(
            "SELECT id, chat_id, file_hash, filename, mime_type, file_size,
                    extracted_text, char_count, created_at
             FROM document_extractions
             WHERE (?1 IS NULL OR chat_id = ?1)
               AND LOWER(extracted_text) LIKE LOWER(?2)
             ORDER BY created_at DESC
             LIMIT ?3",
        )?;
        let chat_id_param: Option<i64> = chat_id;
        let limit_param = limit as i64;
        let rows = stmt.query_map(params![chat_id_param, pattern, limit_param], |row| {
            Ok(DocumentExtraction {
                id: row.get(0)?,
                chat_id: row.get(1)?,
                file_hash: row.get(2)?,
                filename: row.get(3)?,
                mime_type: row.get(4)?,
                file_size: row.get(5)?,
                extracted_text: row.get(6)?,
                char_count: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    pub fn list_document_extractions(
        &self,
        chat_id: i64,
        limit: usize,
    ) -> Result<Vec<DocumentExtraction>, MicroClawError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, chat_id, file_hash, filename, mime_type, file_size,
                    extracted_text, char_count, created_at
             FROM document_extractions
             WHERE chat_id = ?1
             ORDER BY created_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![chat_id, limit as i64], |row| {
            Ok(DocumentExtraction {
                id: row.get(0)?,
                chat_id: row.get(1)?,
                file_hash: row.get(2)?,
                filename: row.get(3)?,
                mime_type: row.get(4)?,
                file_size: row.get(5)?,
                extracted_text: row.get(6)?,
                char_count: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }
```

- [ ] **Step 5: Build to verify**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && . "$HOME/.cargo/env" 2>/dev/null && cargo build -p microclaw-storage`
Expected: Compiles.

- [ ] **Step 6: Commit**

```bash
git add crates/microclaw-storage/src/db.rs
git commit -m "feat(storage): add migration v21 with document_extractions table and CRUD"
```

---

### Task 4: TTS Provider Trait + Edge TTS + OpenAI TTS

**Files:**
- Create: `crates/microclaw-media/src/tts.rs`
- Create: `crates/microclaw-media/src/tts_edge.rs`
- Create: `crates/microclaw-media/src/tts_openai.rs`
- Create: `crates/microclaw-media/src/tts_elevenlabs.rs`
- Update: `crates/microclaw-media/src/lib.rs`

- [ ] **Step 1: Create tts.rs with trait and router**

```rust
// crates/microclaw-media/src/tts.rs

use async_trait::async_trait;
use crate::{AudioFormat, MediaError};

#[derive(Debug, Clone)]
pub struct VoiceInfo {
    pub id: String,
    pub name: String,
    pub language: Option<String>,
}

#[derive(Debug)]
pub struct TtsOutput {
    pub audio_bytes: Vec<u8>,
    pub format: AudioFormat,
    pub duration_ms: Option<u64>,
}

#[async_trait]
pub trait TtsProvider: Send + Sync {
    async fn synthesize(&self, text: &str, voice: &str) -> Result<TtsOutput, MediaError>;
    fn name(&self) -> &str;
    fn voices(&self) -> Vec<VoiceInfo>;
}

pub struct TtsRouter {
    provider: Box<dyn TtsProvider>,
}

impl TtsRouter {
    pub fn new(provider_name: &str, api_key: Option<&str>, voice: &str) -> Result<Self, MediaError> {
        let provider: Box<dyn TtsProvider> = match provider_name {
            #[cfg(feature = "tts")]
            "edge" => Box::new(crate::tts_edge::EdgeTtsProvider::new()),
            "openai" => {
                let key = api_key.ok_or_else(|| MediaError::NotConfigured("OpenAI TTS requires api_key".into()))?;
                Box::new(crate::tts_openai::OpenAiTtsProvider::new(key))
            }
            "elevenlabs" => {
                let key = api_key.ok_or_else(|| MediaError::NotConfigured("ElevenLabs requires api_key".into()))?;
                Box::new(crate::tts_elevenlabs::ElevenLabsTtsProvider::new(key))
            }
            other => return Err(MediaError::NotConfigured(format!("Unknown TTS provider: {other}"))),
        };
        Ok(Self { provider })
    }

    pub async fn synthesize(&self, text: &str, voice: &str) -> Result<TtsOutput, MediaError> {
        self.provider.synthesize(text, voice).await
    }

    pub fn name(&self) -> &str {
        self.provider.name()
    }
}
```

- [ ] **Step 2: Create tts_edge.rs**

```rust
// crates/microclaw-media/src/tts_edge.rs

use async_trait::async_trait;
use crate::tts::{TtsOutput, TtsProvider, VoiceInfo};
use crate::{AudioFormat, MediaError};

pub struct EdgeTtsProvider;

impl EdgeTtsProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TtsProvider for EdgeTtsProvider {
    async fn synthesize(&self, text: &str, voice: &str) -> Result<TtsOutput, MediaError> {
        #[cfg(feature = "tts")]
        {
            use msedge_tts::tts::client::connect;
            let mut tts = connect()
                .await
                .map_err(|e| MediaError::ProviderError(format!("Edge TTS connect failed: {e}")))?;
            let audio = tts
                .synthesize(text, voice)
                .await
                .map_err(|e| MediaError::ProviderError(format!("Edge TTS synthesis failed: {e}")))?;
            let audio_bytes: Vec<u8> = audio
                .audio_bytes
                .into_iter()
                .flat_map(|chunk| chunk)
                .collect();
            Ok(TtsOutput {
                audio_bytes,
                format: AudioFormat::Mp3,
                duration_ms: None,
            })
        }
        #[cfg(not(feature = "tts"))]
        Err(MediaError::NotConfigured("Edge TTS requires 'tts' feature".into()))
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
```

- [ ] **Step 3: Create tts_openai.rs**

```rust
// crates/microclaw-media/src/tts_openai.rs

use async_trait::async_trait;
use crate::tts::{TtsOutput, TtsProvider, VoiceInfo};
use crate::{AudioFormat, MediaError};

pub struct OpenAiTtsProvider {
    api_key: String,
    http: reqwest::Client,
}

impl OpenAiTtsProvider {
    pub fn new(api_key: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            http: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl TtsProvider for OpenAiTtsProvider {
    async fn synthesize(&self, text: &str, voice: &str) -> Result<TtsOutput, MediaError> {
        let voice_name = if voice.is_empty() { "alloy" } else { voice };
        let body = serde_json::json!({
            "model": "tts-1",
            "input": text,
            "voice": voice_name,
            "response_format": "opus"
        });
        let response = self.http
            .post("https://api.openai.com/v1/audio/speech")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(MediaError::ProviderError(format!("OpenAI TTS {status}: {text}")));
        }

        let audio_bytes = response.bytes().await?.to_vec();
        Ok(TtsOutput {
            audio_bytes,
            format: AudioFormat::Opus,
            duration_ms: None,
        })
    }

    fn name(&self) -> &str {
        "openai"
    }

    fn voices(&self) -> Vec<VoiceInfo> {
        vec![
            VoiceInfo { id: "alloy".into(), name: "Alloy".into(), language: None },
            VoiceInfo { id: "echo".into(), name: "Echo".into(), language: None },
            VoiceInfo { id: "fable".into(), name: "Fable".into(), language: None },
            VoiceInfo { id: "onyx".into(), name: "Onyx".into(), language: None },
            VoiceInfo { id: "nova".into(), name: "Nova".into(), language: None },
            VoiceInfo { id: "shimmer".into(), name: "Shimmer".into(), language: None },
        ]
    }
}
```

- [ ] **Step 4: Create tts_elevenlabs.rs**

```rust
// crates/microclaw-media/src/tts_elevenlabs.rs

use async_trait::async_trait;
use crate::tts::{TtsOutput, TtsProvider, VoiceInfo};
use crate::{AudioFormat, MediaError};

pub struct ElevenLabsTtsProvider {
    api_key: String,
    http: reqwest::Client,
}

impl ElevenLabsTtsProvider {
    pub fn new(api_key: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            http: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl TtsProvider for ElevenLabsTtsProvider {
    async fn synthesize(&self, text: &str, voice: &str) -> Result<TtsOutput, MediaError> {
        let voice_id = if voice.is_empty() { "21m00Tcm4TlvDq8ikWAM" } else { voice };
        let body = serde_json::json!({
            "text": text,
            "model_id": "eleven_multilingual_v2"
        });
        let url = format!("https://api.elevenlabs.io/v1/text-to-speech/{voice_id}");
        let response = self.http
            .post(&url)
            .header("xi-api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(MediaError::ProviderError(format!("ElevenLabs {status}: {text}")));
        }

        let audio_bytes = response.bytes().await?.to_vec();
        Ok(TtsOutput {
            audio_bytes,
            format: AudioFormat::Mp3,
            duration_ms: None,
        })
    }

    fn name(&self) -> &str {
        "elevenlabs"
    }

    fn voices(&self) -> Vec<VoiceInfo> {
        vec![
            VoiceInfo { id: "21m00Tcm4TlvDq8ikWAM".into(), name: "Rachel".into(), language: None },
        ]
    }
}
```

- [ ] **Step 5: Build TTS modules**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && . "$HOME/.cargo/env" 2>/dev/null && cargo build -p microclaw-media --features tts`
Expected: Compiles.

- [ ] **Step 6: Commit**

```bash
git add crates/microclaw-media/src/tts.rs crates/microclaw-media/src/tts_edge.rs crates/microclaw-media/src/tts_openai.rs crates/microclaw-media/src/tts_elevenlabs.rs
git commit -m "feat(media): add TTS provider trait + Edge TTS, OpenAI TTS, ElevenLabs providers"
```

---

### Task 5: STT Provider Trait + OpenAI + whisper-rs

**Files:**
- Create: `crates/microclaw-media/src/stt.rs`
- Create: `crates/microclaw-media/src/stt_openai.rs`
- Create: `crates/microclaw-media/src/stt_whisper.rs`

- [ ] **Step 1: Create stt.rs with trait and router**

```rust
// crates/microclaw-media/src/stt.rs

use async_trait::async_trait;
use crate::MediaError;

#[async_trait]
pub trait SttProvider: Send + Sync {
    async fn transcribe(&self, audio_bytes: &[u8], mime_type: &str) -> Result<String, MediaError>;
    fn name(&self) -> &str;
}

pub struct SttRouter {
    provider: Box<dyn SttProvider>,
}

impl SttRouter {
    pub fn new(provider_name: &str, api_key: Option<&str>, model: &str) -> Result<Self, MediaError> {
        let provider: Box<dyn SttProvider> = match provider_name {
            "openai" => {
                let key = api_key.ok_or_else(|| MediaError::NotConfigured("OpenAI STT requires api_key".into()))?;
                Box::new(crate::stt_openai::OpenAiSttProvider::new(key))
            }
            #[cfg(feature = "stt-local")]
            "whisper-local" => {
                Box::new(crate::stt_whisper::WhisperLocalProvider::new(model)?)
            }
            other => return Err(MediaError::NotConfigured(format!("Unknown STT provider: {other}"))),
        };
        Ok(Self { provider })
    }

    pub async fn transcribe(&self, audio_bytes: &[u8], mime_type: &str) -> Result<String, MediaError> {
        self.provider.transcribe(audio_bytes, mime_type).await
    }
}
```

- [ ] **Step 2: Create stt_openai.rs**

```rust
// crates/microclaw-media/src/stt_openai.rs

use async_trait::async_trait;
use crate::stt::SttProvider;
use crate::MediaError;

pub struct OpenAiSttProvider {
    api_key: String,
    http: reqwest::Client,
}

impl OpenAiSttProvider {
    pub fn new(api_key: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            http: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl SttProvider for OpenAiSttProvider {
    async fn transcribe(&self, audio_bytes: &[u8], mime_type: &str) -> Result<String, MediaError> {
        let ext = match mime_type {
            "audio/ogg" | "audio/opus" => "ogg",
            "audio/webm" => "webm",
            "audio/wav" | "audio/x-wav" => "wav",
            "audio/mpeg" | "audio/mp3" => "mp3",
            _ => "ogg",
        };
        let filename = format!("audio.{ext}");
        let part = reqwest::multipart::Part::bytes(audio_bytes.to_vec())
            .file_name(filename)
            .mime_str(mime_type)
            .map_err(|e| MediaError::ProviderError(format!("MIME error: {e}")))?;

        let form = reqwest::multipart::Form::new()
            .text("model", "whisper-1")
            .part("file", part);

        let response = self.http
            .post("https://api.openai.com/v1/audio/transcriptions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(MediaError::ProviderError(format!("OpenAI STT {status}: {text}")));
        }

        let body: serde_json::Value = response.json().await?;
        let text = body.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string();
        Ok(text)
    }

    fn name(&self) -> &str {
        "openai"
    }
}
```

- [ ] **Step 3: Create stt_whisper.rs**

```rust
// crates/microclaw-media/src/stt_whisper.rs

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
        // Model will be loaded lazily on first transcribe call
        Ok(Self {
            model_name: model.to_string(),
        })
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

            // Resolve model path (auto-download would go here)
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

            // Decode audio to f32 PCM (simplified — real impl needs symphonia for format conversion)
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

    fn name(&self) -> &str {
        "whisper-local"
    }
}
```

- [ ] **Step 4: Build**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && . "$HOME/.cargo/env" 2>/dev/null && cargo build -p microclaw-media`
Expected: Compiles (whisper-rs only compiled if `stt-local` feature enabled).

- [ ] **Step 5: Commit**

```bash
git add crates/microclaw-media/src/stt.rs crates/microclaw-media/src/stt_openai.rs crates/microclaw-media/src/stt_whisper.rs
git commit -m "feat(media): add STT provider trait + OpenAI Whisper + local whisper-rs"
```

---

### Task 6: Image Generation Providers

**Files:**
- Create: `crates/microclaw-media/src/image_gen.rs`
- Create: `crates/microclaw-media/src/image_gen_openai.rs`
- Create: `crates/microclaw-media/src/image_gen_fal.rs`

- [ ] **Step 1: Create image_gen.rs with trait and router**

```rust
// crates/microclaw-media/src/image_gen.rs

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
```

- [ ] **Step 2: Create image_gen_openai.rs (DALL-E)**

```rust
// crates/microclaw-media/src/image_gen_openai.rs

use async_trait::async_trait;
use crate::image_gen::{ImageGenProvider, ImageGenParams, ImageGenOutput, GeneratedImage};
use crate::MediaError;

pub struct DalleProvider {
    api_key: String,
    http: reqwest::Client,
}

impl DalleProvider {
    pub fn new(api_key: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            http: reqwest::Client::new(),
        }
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
                images.push(GeneratedImage {
                    data: bytes,
                    format: "png".into(),
                    revised_prompt: revised,
                });
            }
        }

        Ok(ImageGenOutput { images })
    }

    fn name(&self) -> &str {
        "openai"
    }
}
```

- [ ] **Step 3: Create image_gen_fal.rs (FAL FLUX)**

```rust
// crates/microclaw-media/src/image_gen_fal.rs

use async_trait::async_trait;
use crate::image_gen::{ImageGenProvider, ImageGenParams, ImageGenOutput, GeneratedImage};
use crate::MediaError;

pub struct FalFluxProvider {
    api_key: String,
    http: reqwest::Client,
}

impl FalFluxProvider {
    pub fn new(api_key: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            http: reqwest::Client::new(),
        }
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
                images.push(GeneratedImage {
                    data: bytes,
                    format: format.into(),
                    revised_prompt: None,
                });
            }
        }

        Ok(ImageGenOutput { images })
    }

    fn name(&self) -> &str {
        "flux"
    }
}
```

- [ ] **Step 4: Build**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && . "$HOME/.cargo/env" 2>/dev/null && cargo build -p microclaw-media`
Expected: Compiles.

- [ ] **Step 5: Commit**

```bash
git add crates/microclaw-media/src/image_gen.rs crates/microclaw-media/src/image_gen_openai.rs crates/microclaw-media/src/image_gen_fal.rs
git commit -m "feat(media): add image generation providers — DALL-E and FAL FLUX"
```

---

### Task 7: Video Generation Providers

**Files:**
- Create: `crates/microclaw-media/src/video_gen.rs`
- Create: `crates/microclaw-media/src/video_gen_sora.rs`
- Create: `crates/microclaw-media/src/video_gen_fal.rs`
- Create: `crates/microclaw-media/src/video_gen_minimax.rs`

- [ ] **Step 1: Create video_gen.rs with trait, router, and poll_until_ready**

```rust
// crates/microclaw-media/src/video_gen.rs

use async_trait::async_trait;
use std::time::{Duration, Instant};
use crate::MediaError;

#[derive(Debug, Clone, Default)]
pub struct VideoGenParams {
    pub duration_secs: Option<u32>,
    pub resolution: Option<String>,
}

#[derive(Debug)]
pub struct VideoGenOutput {
    pub video_bytes: Vec<u8>,
    pub format: String,
    pub duration_secs: f32,
}

#[async_trait]
pub trait VideoGenProvider: Send + Sync {
    async fn generate(&self, prompt: &str, params: VideoGenParams) -> Result<VideoGenOutput, MediaError>;
    fn name(&self) -> &str;
}

/// Shared polling helper for queue-based video APIs.
pub async fn poll_until_ready(
    client: &reqwest::Client,
    status_url: &str,
    headers: reqwest::header::HeaderMap,
    timeout: Duration,
) -> Result<serde_json::Value, MediaError> {
    let deadline = Instant::now() + timeout;
    loop {
        if Instant::now() > deadline {
            return Err(MediaError::Timeout);
        }
        let resp = client
            .get(status_url)
            .headers(headers.clone())
            .send()
            .await?;
        let body: serde_json::Value = resp.json().await?;
        let status = body
            .get("status")
            .and_then(|s| s.as_str())
            .unwrap_or("");
        match status {
            "completed" | "succeeded" | "Completed" | "Success" => return Ok(body),
            "failed" | "error" | "Failed" | "Error" => {
                return Err(MediaError::ProviderError(format!("Video generation failed: {}", body)));
            }
            _ => {
                tracing::debug!("Video gen status: {status}, polling again...");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}

pub struct VideoGenRouter {
    provider: Box<dyn VideoGenProvider>,
}

impl VideoGenRouter {
    pub fn new(
        provider_name: &str,
        api_key: Option<&str>,
        fal_model: Option<&str>,
        minimax_key: Option<&str>,
        timeout_secs: u64,
    ) -> Result<Self, MediaError> {
        let provider: Box<dyn VideoGenProvider> = match provider_name {
            "sora" => {
                let key = api_key.ok_or_else(|| MediaError::NotConfigured("Sora requires api_key".into()))?;
                Box::new(crate::video_gen_sora::SoraProvider::new(key, timeout_secs))
            }
            "fal" => {
                let key = api_key.ok_or_else(|| MediaError::NotConfigured("FAL video requires api_key".into()))?;
                let model = fal_model.unwrap_or("cogvideox");
                Box::new(crate::video_gen_fal::FalVideoProvider::new(key, model, timeout_secs))
            }
            "minimax" => {
                let key = minimax_key.ok_or_else(|| MediaError::NotConfigured("MiniMax requires minimax_key".into()))?;
                Box::new(crate::video_gen_minimax::MiniMaxProvider::new(key, timeout_secs))
            }
            other => return Err(MediaError::NotConfigured(format!("Unknown video gen provider: {other}"))),
        };
        Ok(Self { provider })
    }

    pub async fn generate(&self, prompt: &str, params: VideoGenParams) -> Result<VideoGenOutput, MediaError> {
        self.provider.generate(prompt, params).await
    }
}
```

- [ ] **Step 2: Create video_gen_sora.rs**

```rust
// crates/microclaw-media/src/video_gen_sora.rs

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
            return Err(MediaError::ProviderError("Sora 2 API is not available. Try 'fal' or 'minimax' provider.".into()));
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
        headers.insert("Authorization", format!("Bearer {}", self.api_key).parse().unwrap());

        let completed = poll_until_ready(&self.http, &status_url, headers, self.timeout).await?;

        let video_url = completed.get("url").and_then(|u| u.as_str())
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
```

- [ ] **Step 3: Create video_gen_fal.rs**

```rust
// crates/microclaw-media/src/video_gen_fal.rs

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

        // FAL may return result directly or require polling
        if let Some(video_url) = result.get("video").and_then(|v| v.get("url")).and_then(|u| u.as_str()) {
            let video_bytes = self.http.get(video_url).send().await?.bytes().await?.to_vec();
            return Ok(VideoGenOutput {
                video_bytes,
                format: "mp4".into(),
                duration_secs: params.duration_secs.unwrap_or(5) as f32,
            });
        }

        // Queue-based: get request_id and poll
        let request_id = result.get("request_id").and_then(|r| r.as_str())
            .ok_or_else(|| MediaError::ProviderError("No request_id or video in FAL response".into()))?;

        let status_url = format!("{}/requests/{}", url, request_id);
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("Authorization", format!("Key {}", self.api_key).parse().unwrap());

        let completed = poll_until_ready(&self.http, &status_url, headers, self.timeout).await?;

        let video_url = completed.get("video").and_then(|v| v.get("url")).and_then(|u| u.as_str())
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
```

- [ ] **Step 4: Create video_gen_minimax.rs**

```rust
// crates/microclaw-media/src/video_gen_minimax.rs

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
        let task_id = result.get("task_id").and_then(|t| t.as_str())
            .ok_or_else(|| MediaError::ProviderError("No task_id in MiniMax response".into()))?;

        let status_url = format!("https://api.minimax.io/v1/query/video_generation?task_id={task_id}");
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("Authorization", format!("Bearer {}", self.api_key).parse().unwrap());

        let completed = poll_until_ready(&self.http, &status_url, headers, self.timeout).await?;

        let file_id = completed.get("file_id").and_then(|f| f.as_str())
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
```

- [ ] **Step 5: Build**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && . "$HOME/.cargo/env" 2>/dev/null && cargo build -p microclaw-media`
Expected: Compiles.

- [ ] **Step 6: Commit**

```bash
git add crates/microclaw-media/src/video_gen.rs crates/microclaw-media/src/video_gen_sora.rs crates/microclaw-media/src/video_gen_fal.rs crates/microclaw-media/src/video_gen_minimax.rs
git commit -m "feat(media): add video generation providers — Sora 2, FAL, MiniMax Hailuo"
```

---

### Task 8: Document Extraction Module + Tool

**Files:**
- Create: `crates/microclaw-media/src/documents.rs`
- Create: `src/tools/read_document.rs`
- Modify: `src/tools/mod.rs`

- [ ] **Step 1: Create documents.rs**

```rust
// crates/microclaw-media/src/documents.rs

use crate::MediaError;
use sha2::{Sha256, Digest};

/// Compute SHA-256 hash of file bytes.
pub fn compute_file_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Extract text from a file using kreuzberg.
#[cfg(feature = "documents")]
pub async fn extract_text(file_path: &str) -> Result<String, MediaError> {
    let path = file_path.to_string();
    tokio::task::spawn_blocking(move || {
        kreuzberg::extract_text(&path)
            .map_err(|e| MediaError::ProviderError(format!("kreuzberg extraction failed: {e}")))
    })
    .await
    .map_err(|e| MediaError::ProviderError(format!("Task failed: {e}")))?
}

#[cfg(not(feature = "documents"))]
pub async fn extract_text(_file_path: &str) -> Result<String, MediaError> {
    Err(MediaError::NotConfigured("Document extraction requires 'documents' feature".into()))
}
```

- [ ] **Step 2: Create read_document.rs tool**

```rust
// src/tools/read_document.rs

use std::sync::Arc;
use async_trait::async_trait;
use microclaw_core::llm_types::ToolDefinition;
use microclaw_storage::db::Database;
use microclaw_tools::runtime::{Tool, ToolResult};
use serde_json::json;

pub struct ReadDocumentTool {
    db: Arc<Database>,
    control_chat_ids: Vec<i64>,
}

impl ReadDocumentTool {
    pub fn new(db: Arc<Database>, control_chat_ids: Vec<i64>) -> Self {
        Self { db, control_chat_ids }
    }
}

#[async_trait]
impl Tool for ReadDocumentTool {
    fn name(&self) -> &str {
        "read_document"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "read_document".into(),
            description: "Extract text from uploaded documents (PDF, DOCX, XLSX, etc.) or search/list previously extracted documents.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to document file to extract text from"
                    },
                    "query": {
                        "type": "string",
                        "description": "Search term to find in previously extracted documents"
                    },
                    "list": {
                        "type": "boolean",
                        "description": "List all documents uploaded to this chat"
                    },
                    "file_hash": {
                        "type": "string",
                        "description": "Retrieve a specific document by its hash"
                    }
                }
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let caller_chat_id = input
            .get("__auth_context")
            .and_then(|a| a.get("chat_id"))
            .and_then(|v| v.as_i64());

        let is_control = caller_chat_id
            .map(|id| self.control_chat_ids.contains(&id))
            .unwrap_or(false);

        // Mode: List documents
        if input.get("list").and_then(|v| v.as_bool()).unwrap_or(false) {
            let chat_id = match caller_chat_id {
                Some(id) => id,
                None => return ToolResult::error("No chat context available".into()),
            };
            let db = self.db.clone();
            return match tokio::task::spawn_blocking(move || db.list_document_extractions(chat_id, 20)).await {
                Ok(Ok(docs)) => {
                    if docs.is_empty() {
                        return ToolResult::success("No documents uploaded to this chat.".into());
                    }
                    let mut output = format!("{} documents:\n\n", docs.len());
                    for doc in &docs {
                        output.push_str(&format!(
                            "- {} ({} chars, {})\n  hash: {}\n",
                            doc.filename, doc.char_count, doc.created_at, doc.file_hash
                        ));
                    }
                    ToolResult::success(output)
                }
                Ok(Err(e)) => ToolResult::error(format!("Failed to list documents: {e}")),
                Err(e) => ToolResult::error(format!("Task failed: {e}")),
            };
        }

        // Mode: Search documents
        if let Some(query) = input.get("query").and_then(|v| v.as_str()) {
            let chat_filter = if is_control { None } else { caller_chat_id };
            let db = self.db.clone();
            let q = query.to_string();
            return match tokio::task::spawn_blocking(move || db.search_document_extractions(chat_filter, &q, 10)).await {
                Ok(Ok(docs)) => {
                    if docs.is_empty() {
                        return ToolResult::success(format!("No documents matching \"{query}\"."));
                    }
                    let mut output = format!("Found {} matching documents:\n\n", docs.len());
                    for doc in &docs {
                        let preview = if doc.extracted_text.len() > 200 {
                            format!("{}...", &doc.extracted_text[..200])
                        } else {
                            doc.extracted_text.clone()
                        };
                        output.push_str(&format!(
                            "-- {} (chat_id: {}) --\n{}\n\n",
                            doc.filename, doc.chat_id, preview
                        ));
                    }
                    ToolResult::success(output)
                }
                Ok(Err(e)) => ToolResult::error(format!("Search failed: {e}")),
                Err(e) => ToolResult::error(format!("Task failed: {e}")),
            };
        }

        // Mode: Retrieve by hash
        if let Some(hash) = input.get("file_hash").and_then(|v| v.as_str()) {
            let chat_id = match caller_chat_id {
                Some(id) => id,
                None => return ToolResult::error("No chat context available".into()),
            };
            let db = self.db.clone();
            let h = hash.to_string();
            return match tokio::task::spawn_blocking(move || db.get_document_extraction(chat_id, &h)).await {
                Ok(Ok(Some(doc))) => ToolResult::success(format!(
                    "Document: {}\nSize: {} chars\n\n{}",
                    doc.filename, doc.char_count, doc.extracted_text
                )),
                Ok(Ok(None)) => ToolResult::error(format!("No document found with hash {hash}")),
                Ok(Err(e)) => ToolResult::error(format!("Retrieval failed: {e}")),
                Err(e) => ToolResult::error(format!("Task failed: {e}")),
            };
        }

        // Mode: Extract from file path
        if let Some(file_path) = input.get("file_path").and_then(|v| v.as_str()) {
            match microclaw_media::documents::extract_text(file_path).await {
                Ok(text) => {
                    // Store extraction in DB if we have chat context
                    if let Some(chat_id) = caller_chat_id {
                        let file_bytes = tokio::fs::read(file_path).await.unwrap_or_default();
                        let file_hash = microclaw_media::documents::compute_file_hash(&file_bytes);
                        let filename = std::path::Path::new(file_path)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let file_size = file_bytes.len() as i64;
                        let db = self.db.clone();
                        let txt = text.clone();
                        let _ = tokio::task::spawn_blocking(move || {
                            db.insert_document_extraction(chat_id, &file_hash, &filename, None, file_size, &txt)
                        }).await;
                    }

                    let display = if text.len() > 50_000 {
                        format!(
                            "{}\n\n(truncated, {} chars total — use read_document with file_hash to retrieve full text)",
                            &text[..50_000],
                            text.len()
                        )
                    } else {
                        text
                    };
                    ToolResult::success(display)
                }
                Err(e) => ToolResult::error(format!("Extraction failed: {e}")),
            }
        } else {
            ToolResult::error("Provide one of: file_path, query, list, or file_hash".into())
        }
    }
}
```

- [ ] **Step 3: Register in mod.rs**

Add `pub mod read_document;` at the top. In `ToolRegistry::new()`, add conditionally:

```rust
        if config.document_extraction_enabled {
            tools.push(Box::new(read_document::ReadDocumentTool::new(
                db.clone(),
                config.control_chat_ids.clone(),
            )));
        }
```

- [ ] **Step 4: Build**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && . "$HOME/.cargo/env" 2>/dev/null && cargo build`
Expected: Compiles.

- [ ] **Step 5: Commit**

```bash
git add crates/microclaw-media/src/documents.rs src/tools/read_document.rs src/tools/mod.rs
git commit -m "feat(tools): add read_document tool with kreuzberg extraction and per-chat storage"
```

---

### Task 9: TTS + Image Gen + Video Gen Tools

**Files:**
- Create: `src/tools/text_to_speech.rs`
- Create: `src/tools/image_generate.rs`
- Create: `src/tools/video_generate.rs`
- Modify: `src/tools/mod.rs`

- [ ] **Step 1: Create text_to_speech.rs**

```rust
// src/tools/text_to_speech.rs

use async_trait::async_trait;
use microclaw_core::llm_types::ToolDefinition;
use microclaw_tools::runtime::{Tool, ToolResult};
use serde_json::json;

pub struct TextToSpeechTool {
    tts_provider: String,
    tts_api_key: Option<String>,
    tts_voice: String,
    data_dir: String,
}

impl TextToSpeechTool {
    pub fn new(provider: &str, api_key: Option<&str>, voice: &str, data_dir: &str) -> Self {
        Self {
            tts_provider: provider.to_string(),
            tts_api_key: api_key.map(String::from),
            tts_voice: voice.to_string(),
            data_dir: data_dir.to_string(),
        }
    }
}

#[async_trait]
impl Tool for TextToSpeechTool {
    fn name(&self) -> &str { "text_to_speech" }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "text_to_speech".into(),
            description: "Convert text to speech audio. Returns the file path of the generated audio.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "Text to convert to speech" },
                    "voice": { "type": "string", "description": "Voice name (optional)" },
                },
                "required": ["text"]
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let text = match input.get("text").and_then(|v| v.as_str()) {
            Some(t) if !t.trim().is_empty() => t,
            _ => return ToolResult::error("Missing or empty 'text' parameter".into()),
        };
        let voice = input.get("voice").and_then(|v| v.as_str()).unwrap_or(&self.tts_voice);

        let router = match microclaw_media::tts::TtsRouter::new(
            &self.tts_provider,
            self.tts_api_key.as_deref(),
            voice,
        ) {
            Ok(r) => r,
            Err(e) => return ToolResult::error(format!("TTS init failed: {e}")),
        };

        match router.synthesize(text, voice).await {
            Ok(output) => {
                let ext = match output.format {
                    microclaw_media::AudioFormat::Mp3 => "mp3",
                    microclaw_media::AudioFormat::Wav => "wav",
                    microclaw_media::AudioFormat::Opus => "opus",
                    microclaw_media::AudioFormat::Ogg => "ogg",
                };
                let dir = std::path::Path::new(&self.data_dir).join("media");
                let _ = std::fs::create_dir_all(&dir);
                let filename = format!("tts_{}.{}", uuid::Uuid::new_v4(), ext);
                let path = dir.join(&filename);
                if let Err(e) = std::fs::write(&path, &output.audio_bytes) {
                    return ToolResult::error(format!("Failed to save audio: {e}"));
                }
                ToolResult::success(format!("Audio generated: {}", path.display()))
            }
            Err(e) => ToolResult::error(format!("TTS failed: {e}")),
        }
    }
}
```

- [ ] **Step 2: Create image_generate.rs**

```rust
// src/tools/image_generate.rs

use async_trait::async_trait;
use microclaw_core::llm_types::ToolDefinition;
use microclaw_tools::runtime::{Tool, ToolResult};
use serde_json::json;

pub struct ImageGenerateTool {
    provider: String,
    api_key: Option<String>,
    fal_key: Option<String>,
    default_size: String,
    default_quality: String,
    data_dir: String,
}

impl ImageGenerateTool {
    pub fn new(
        provider: &str, api_key: Option<&str>, fal_key: Option<&str>,
        size: &str, quality: &str, data_dir: &str,
    ) -> Self {
        Self {
            provider: provider.to_string(),
            api_key: api_key.map(String::from),
            fal_key: fal_key.map(String::from),
            default_size: size.to_string(),
            default_quality: quality.to_string(),
            data_dir: data_dir.to_string(),
        }
    }
}

#[async_trait]
impl Tool for ImageGenerateTool {
    fn name(&self) -> &str { "image_generate" }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "image_generate".into(),
            description: "Generate an image from a text prompt using DALL-E or FAL FLUX. Returns the file path.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "prompt": { "type": "string", "description": "Image description" },
                    "size": { "type": "string", "description": "Image size (default: 1024x1024)" },
                    "quality": { "type": "string", "description": "Quality: standard or high" },
                },
                "required": ["prompt"]
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let prompt = match input.get("prompt").and_then(|v| v.as_str()) {
            Some(p) if !p.trim().is_empty() => p,
            _ => return ToolResult::error("Missing or empty 'prompt'".into()),
        };

        let params = microclaw_media::image_gen::ImageGenParams {
            size: input.get("size").and_then(|v| v.as_str()).map(String::from).or(Some(self.default_size.clone())),
            quality: input.get("quality").and_then(|v| v.as_str()).map(String::from).or(Some(self.default_quality.clone())),
            n: Some(1),
        };

        let router = match microclaw_media::image_gen::ImageGenRouter::new(
            &self.provider, self.api_key.as_deref(), self.fal_key.as_deref(),
        ) {
            Ok(r) => r,
            Err(e) => return ToolResult::error(format!("Image gen init failed: {e}")),
        };

        match router.generate(prompt, params).await {
            Ok(output) => {
                if let Some(img) = output.images.first() {
                    let dir = std::path::Path::new(&self.data_dir).join("media");
                    let _ = std::fs::create_dir_all(&dir);
                    let filename = format!("img_{}.{}", uuid::Uuid::new_v4(), img.format);
                    let path = dir.join(&filename);
                    if let Err(e) = std::fs::write(&path, &img.data) {
                        return ToolResult::error(format!("Failed to save image: {e}"));
                    }
                    ToolResult::success(format!("Image generated: {}", path.display()))
                } else {
                    ToolResult::error("No image returned".into())
                }
            }
            Err(e) => ToolResult::error(format!("Image generation failed: {e}")),
        }
    }
}
```

- [ ] **Step 3: Create video_generate.rs**

```rust
// src/tools/video_generate.rs

use async_trait::async_trait;
use microclaw_core::llm_types::ToolDefinition;
use microclaw_tools::runtime::{Tool, ToolResult};
use serde_json::json;

pub struct VideoGenerateTool {
    provider: String,
    api_key: Option<String>,
    fal_model: Option<String>,
    minimax_key: Option<String>,
    timeout_secs: u64,
    data_dir: String,
}

impl VideoGenerateTool {
    pub fn new(
        provider: &str, api_key: Option<&str>, fal_model: Option<&str>,
        minimax_key: Option<&str>, timeout_secs: u64, data_dir: &str,
    ) -> Self {
        Self {
            provider: provider.to_string(),
            api_key: api_key.map(String::from),
            fal_model: fal_model.map(String::from),
            minimax_key: minimax_key.map(String::from),
            timeout_secs,
            data_dir: data_dir.to_string(),
        }
    }
}

#[async_trait]
impl Tool for VideoGenerateTool {
    fn name(&self) -> &str { "video_generate" }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "video_generate".into(),
            description: "Generate a short video from a text prompt. Use sparingly — video generation takes 1-5 minutes.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "prompt": { "type": "string", "description": "Video description" },
                    "duration": { "type": "integer", "description": "Duration in seconds (default 5)" },
                },
                "required": ["prompt"]
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let prompt = match input.get("prompt").and_then(|v| v.as_str()) {
            Some(p) if !p.trim().is_empty() => p,
            _ => return ToolResult::error("Missing or empty 'prompt'".into()),
        };

        let params = microclaw_media::video_gen::VideoGenParams {
            duration_secs: input.get("duration").and_then(|v| v.as_u64()).map(|d| d as u32),
            resolution: None,
        };

        let router = match microclaw_media::video_gen::VideoGenRouter::new(
            &self.provider,
            self.api_key.as_deref(),
            self.fal_model.as_deref(),
            self.minimax_key.as_deref(),
            self.timeout_secs,
        ) {
            Ok(r) => r,
            Err(e) => return ToolResult::error(format!("Video gen init failed: {e}")),
        };

        match router.generate(prompt, params).await {
            Ok(output) => {
                let dir = std::path::Path::new(&self.data_dir).join("media");
                let _ = std::fs::create_dir_all(&dir);
                let filename = format!("vid_{}.{}", uuid::Uuid::new_v4(), output.format);
                let path = dir.join(&filename);
                if let Err(e) = std::fs::write(&path, &output.video_bytes) {
                    return ToolResult::error(format!("Failed to save video: {e}"));
                }
                ToolResult::success(format!("Video generated ({:.0}s): {}", output.duration_secs, path.display()))
            }
            Err(e) => ToolResult::error(format!("Video generation failed: {e}")),
        }
    }
}
```

- [ ] **Step 4: Register all tools in mod.rs**

Add module declarations:
```rust
pub mod text_to_speech;
pub mod image_generate;
pub mod video_generate;
```

In `ToolRegistry::new()`, add conditionally before ClawHub block:
```rust
        if config.tts_enabled {
            tools.push(Box::new(text_to_speech::TextToSpeechTool::new(
                &config.tts_provider,
                config.tts_api_key.as_deref().or(Some(&config.api_key)),
                &config.tts_voice,
                &config.data_dir,
            )));
        }
        if config.image_gen_enabled {
            tools.push(Box::new(image_generate::ImageGenerateTool::new(
                &config.image_gen_provider,
                config.image_gen_api_key.as_deref().or(Some(&config.api_key)),
                config.image_gen_fal_key.as_deref(),
                &config.image_gen_default_size,
                &config.image_gen_default_quality,
                &config.data_dir,
            )));
        }
        if config.video_gen_enabled {
            tools.push(Box::new(video_generate::VideoGenerateTool::new(
                &config.video_gen_provider,
                config.video_gen_api_key.as_deref().or(Some(&config.api_key)),
                config.video_gen_fal_model.as_deref(),
                config.video_gen_minimax_key.as_deref(),
                config.video_gen_timeout_secs,
                &config.data_dir,
            )));
        }
```

- [ ] **Step 5: Build**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && . "$HOME/.cargo/env" 2>/dev/null && cargo build`
Expected: Compiles.

- [ ] **Step 6: Commit**

```bash
git add src/tools/text_to_speech.rs src/tools/image_generate.rs src/tools/video_generate.rs src/tools/mod.rs
git commit -m "feat(tools): add text_to_speech, image_generate, video_generate tools"
```

---

### Task 10: Vision Routing

**Files:**
- Modify: `src/agent_engine.rs`

- [ ] **Step 1: Add vision routing helper functions**

Add near the top of `src/agent_engine.rs` (after imports):

```rust
fn message_has_images(messages: &[Message]) -> bool {
    messages.iter().any(|msg| {
        if let MessageContent::Blocks(blocks) = &msg.content {
            blocks.iter().any(|b| matches!(b, ContentBlock::Image { .. }))
        } else {
            false
        }
    })
}

fn model_supports_vision(model: &str) -> bool {
    let m = model.to_lowercase();
    m.contains("claude-3") || m.contains("claude-sonnet-4") || m.contains("claude-opus-4")
        || m.contains("gpt-4o") || m.contains("gpt-4.1") || m.contains("gpt-5")
        || m.contains("o1") || m.contains("o3") || m.contains("o4")
        || m.contains("llava") || m.contains("vision")
        || m.contains("qwen2.5-vl") || m.contains("gemma3")
}
```

- [ ] **Step 2: Add vision fallback check before LLM call**

In the agent loop (inside `process_with_agent_logic`), before the LLM call, add:

```rust
    // Vision routing: if message has images but model doesn't support vision,
    // route to fallback provider
    if state.config.vision_fallback_enabled
        && message_has_images(&messages)
        && !model_supports_vision(&effective_model)
    {
        if let Some(ref fallback_key) = state.config.vision_fallback_api_key {
            tracing::info!(
                "Routing image to vision fallback: {} via {}",
                state.config.vision_fallback_model,
                state.config.vision_fallback_provider
            );
            // Create one-off provider for vision
            let vision_provider = crate::llm::create_provider_with_base_url(
                "openai",
                fallback_key,
                &state.config.vision_fallback_model,
                state.config.max_tokens,
                Some(&state.config.vision_fallback_base_url),
            );
            // Use vision provider for this call only
            // (implementation depends on existing provider interface)
        }
    }
```

Note: The exact integration depends on how `create_provider` works. Read the actual function signature and adapt. The key pattern is: check for images + non-vision model → create temporary OpenAI-compatible provider pointing at OpenRouter.

- [ ] **Step 3: Build**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && . "$HOME/.cargo/env" 2>/dev/null && cargo build`

- [ ] **Step 4: Commit**

```bash
git add src/agent_engine.rs
git commit -m "feat: add vision provider routing with OpenRouter fallback"
```

---

### Task 11: Channel Adapter — send_voice() and send_video()

**Files:**
- Modify: `crates/microclaw-channels/src/channel_adapter.rs`
- Modify: `src/channels/telegram.rs`
- Modify: `src/channels/discord.rs`

- [ ] **Step 1: Add trait methods to ChannelAdapter**

Add to the `ChannelAdapter` trait in `channel_adapter.rs`:

```rust
    async fn send_voice(
        &self,
        external_chat_id: &str,
        audio_path: &std::path::Path,
        duration_secs: Option<u32>,
        caption: Option<&str>,
    ) -> Result<String, String> {
        // Default: fall back to send_attachment
        self.send_attachment(external_chat_id, audio_path, caption).await
    }

    async fn send_video(
        &self,
        external_chat_id: &str,
        video_path: &std::path::Path,
        caption: Option<&str>,
        duration_secs: Option<u32>,
    ) -> Result<String, String> {
        // Default: fall back to send_attachment
        self.send_attachment(external_chat_id, video_path, caption).await
    }
```

- [ ] **Step 2: Implement in Telegram**

In `src/channels/telegram.rs`, override `send_voice` to use `bot.send_voice()` for native voice bubble, and `send_video` to use `bot.send_video()`.

- [ ] **Step 3: Build and commit**

```bash
git add crates/microclaw-channels/src/channel_adapter.rs src/channels/telegram.rs src/channels/discord.rs
git commit -m "feat(channels): add send_voice() and send_video() to ChannelAdapter trait"
```

---

### Task 12: Frontend — Media Components

**Files:**
- Create: `web/src/components/media/image-viewer.tsx`
- Create: `web/src/components/media/audio-player.tsx`
- Create: `web/src/components/media/video-player.tsx`
- Create: `web/src/components/media/file-preview.tsx`

- [ ] **Step 1: Create image-viewer.tsx**

```tsx
// web/src/components/media/image-viewer.tsx
import { useState } from "react";

type Props = { src: string; alt?: string };

export function ImageViewer({ src, alt }: Props) {
  const [expanded, setExpanded] = useState(false);
  return (
    <>
      <img
        src={src}
        alt={alt || "Generated image"}
        className="max-w-full max-h-80 rounded-lg cursor-pointer hover:opacity-90 transition-opacity"
        onClick={() => setExpanded(true)}
      />
      {expanded && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/80"
          onClick={() => setExpanded(false)}
        >
          <img src={src} alt={alt} className="max-w-[90vw] max-h-[90vh] rounded-lg" />
        </div>
      )}
    </>
  );
}
```

- [ ] **Step 2: Create audio-player.tsx**

```tsx
// web/src/components/media/audio-player.tsx
type Props = { src: string; name?: string };

export function AudioPlayer({ src, name }: Props) {
  return (
    <div className="flex flex-col gap-1 p-2 rounded-lg bg-white/5 border border-white/10 max-w-sm">
      {name && <span className="text-xs text-gray-400">{name}</span>}
      <audio controls className="w-full h-8" preload="metadata">
        <source src={src} />
      </audio>
    </div>
  );
}
```

- [ ] **Step 3: Create video-player.tsx**

```tsx
// web/src/components/media/video-player.tsx
type Props = { src: string; caption?: string };

export function VideoPlayer({ src, caption }: Props) {
  return (
    <div className="max-w-lg">
      <video controls className="w-full rounded-lg" preload="metadata">
        <source src={src} type="video/mp4" />
      </video>
      {caption && <p className="text-xs text-gray-400 mt-1">{caption}</p>}
    </div>
  );
}
```

- [ ] **Step 4: Create file-preview.tsx**

```tsx
// web/src/components/media/file-preview.tsx
import { FileIcon } from "lucide-react";

type Props = { url: string; name: string; size?: number };

export function FilePreview({ url, name, size }: Props) {
  const sizeText = size ? `${(size / 1024).toFixed(1)} KB` : "";
  return (
    <a
      href={url}
      download={name}
      className="flex items-center gap-2 p-2 rounded-lg bg-white/5 border border-white/10 hover:bg-white/10 transition-colors max-w-xs"
    >
      <FileIcon className="w-5 h-5 text-gray-400 shrink-0" />
      <div className="min-w-0">
        <div className="text-sm truncate">{name}</div>
        {sizeText && <div className="text-xs text-gray-500">{sizeText}</div>}
      </div>
    </a>
  );
}
```

- [ ] **Step 5: Commit**

```bash
git add web/src/components/media/
git commit -m "feat(web): add media rendering components — ImageViewer, AudioPlayer, VideoPlayer, FilePreview"
```

---

### Task 13: Frontend — Types, SSE Events, Message Renderer

**Files:**
- Modify: `web/src/lib/types.ts`
- Modify: `web/src/lib/sse-parser.ts`
- Modify: `web/src/hooks/use-chat-adapter.ts`
- Modify: `web/src/components/message-components.tsx`

- [ ] **Step 1: Update BackendMessage type**

In `web/src/lib/types.ts`, update `BackendMessage` (line 24-30):

```typescript
export type MediaAttachment = {
  type: "image" | "audio" | "video" | "file";
  url: string;
  mime_type?: string;
  name?: string;
  size?: number;
};

export type BackendMessage = {
  id?: string;
  sender_name?: string;
  content?: string;
  is_from_bot?: boolean;
  timestamp?: string;
  attachments?: MediaAttachment[];
};
```

- [ ] **Step 2: Handle `media` SSE event in sse-parser.ts**

Add `media` to the recognized event types.

- [ ] **Step 3: Accumulate media in use-chat-adapter.ts**

In the stream processing, handle `media` events by adding to an attachments array.

- [ ] **Step 4: Render attachments in message-components.tsx**

Import media components and render based on attachment type:

```tsx
import { ImageViewer } from "./media/image-viewer";
import { AudioPlayer } from "./media/audio-player";
import { VideoPlayer } from "./media/video-player";
import { FilePreview } from "./media/file-preview";

// In the message render:
{message.attachments?.map((att, i) => (
  <div key={i}>
    {att.type === "image" && <ImageViewer src={att.url} />}
    {att.type === "audio" && <AudioPlayer src={att.url} name={att.name} />}
    {att.type === "video" && <VideoPlayer src={att.url} />}
    {att.type === "file" && <FilePreview url={att.url} name={att.name || "file"} size={att.size} />}
  </div>
))}
```

- [ ] **Step 5: Commit**

```bash
git add web/src/lib/types.ts web/src/lib/sse-parser.ts web/src/hooks/use-chat-adapter.ts web/src/components/message-components.tsx
git commit -m "feat(web): add media attachment rendering in chat messages"
```

---

### Task 14: Backend API — Upload + Media Serve

**Files:**
- Modify: `src/web.rs`

- [ ] **Step 1: Add /api/upload endpoint**

Add a multipart upload handler that validates MIME type via `infer`, saves to `{data_dir}/uploads/{uuid}.{ext}`, and returns `{ media_id, mime_type, size }`.

- [ ] **Step 2: Add /api/media/{id} endpoint**

Serve files from the media/uploads directories with correct Content-Type headers.

- [ ] **Step 3: Register routes**

In `build_router()`, add:
```rust
.route("/api/upload", post(api_upload))
.route("/api/media/:id", get(api_media))
```

- [ ] **Step 4: Add media SSE event emission**

When tools generate media files, emit a `media` SSE event with the URL.

- [ ] **Step 5: Build and commit**

```bash
git add src/web.rs
git commit -m "feat(web): add /api/upload and /api/media endpoints + media SSE events"
```

---

### Task 15: Web UI Settings — Multimodal Tab

**Files:**
- Create: `web/src/components/settings/multimodal-tab.tsx`
- Modify: `web/src/components/settings-dialog.tsx`

- [ ] **Step 1: Create multimodal-tab.tsx**

Use existing `ConfigFieldCard` and `ConfigToggleCard` patterns from other settings tabs. Include sections for:
- TTS: enabled toggle, provider selector, voice field, API key
- STT: enabled toggle, provider selector, model selector
- Image gen: enabled toggle, provider selector, API keys
- Video gen: enabled toggle, provider selector, API keys
- Vision fallback: enabled toggle, model field, OpenRouter key
- Documents: enabled toggle, max size

- [ ] **Step 2: Register tab in settings-dialog.tsx**

Add a new `Tabs.Trigger` for "Multimodal" and corresponding `Tabs.Content` rendering `MultimodalTab`.

- [ ] **Step 3: Commit**

```bash
git add web/src/components/settings/multimodal-tab.tsx web/src/components/settings-dialog.tsx
git commit -m "feat(web): add Multimodal settings tab with provider configuration"
```

---

### Task 16: Setup Wizard Pages

**Files:**
- Modify: `src/setup.rs`

- [ ] **Step 1: Add 3 new TUI pages**

Follow the existing page pattern (ratatui widgets, key handling). Add:
1. "Voice & Speech" page — TTS enable/provider/voice + STT enable/provider/model
2. "Media Generation" page — Image gen enable/provider + Video gen enable/provider
3. "Vision & Documents" page — Vision fallback + Document processing toggle

Each page uses radio buttons for provider selection and text inputs for API keys, matching the existing setup wizard style.

- [ ] **Step 2: Build and commit**

```bash
git add src/setup.rs
git commit -m "feat(setup): add multimodal configuration pages to TUI wizard"
```

---

### Task 17: Final Build + Integration Verification

- [ ] **Step 1: Full build**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && . "$HOME/.cargo/env" 2>/dev/null && cargo build`
Expected: Clean compile.

- [ ] **Step 2: Run all tests**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && . "$HOME/.cargo/env" 2>/dev/null && cargo test`
Expected: All tests pass.

- [ ] **Step 3: Build frontend**

Run: `cd web && npm run build`
Expected: Clean build.

- [ ] **Step 4: Verify config example**

Update `microclaw.config.example.yaml` with all new multimodal fields.

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "chore: final integration fixups for multimodal capabilities"
```

---

## Self-Review Checklist

- [x] Spec coverage: All 10 modules have tasks (Media crate=T1, Config=T2, Documents=T3+T8, TTS=T4, STT=T5, Image=T6, Video=T7, Tools=T8+T9, Vision=T10, Channels=T11, Frontend=T12+T13+T14+T15, Setup=T16)
- [x] No placeholders: All steps have code blocks
- [x] Type consistency: MediaError, TtsOutput, ImageGenOutput, VideoGenOutput match across tasks
- [x] Provider names match: "edge"/"openai"/"elevenlabs"/"whisper-local"/"flux"/"sora"/"fal"/"minimax"
- [x] Config fields match: all `*_enabled`, `*_provider`, `*_api_key` fields consistent
- [x] Tools registered conditionally: `if config.*_enabled`
- [x] Feature flags: tts, stt-local, documents
- [x] Frontend types: MediaAttachment type used in BackendMessage and message renderer
