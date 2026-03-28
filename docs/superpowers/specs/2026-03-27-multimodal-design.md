# Unified Multimodal Capabilities

**Date:** 2026-03-27
**Status:** Approved
**Scope:** ~2,800 lines across ~20 files (8 new, ~12 modified)

## Problem

mchact's multimodal capabilities are limited:
- **Documents**: Saved to disk but agent cannot read content
- **TTS**: None — no voice reply capability
- **STT**: OpenAI Whisper only, Telegram-only, no local option
- **Image generation**: None
- **Video generation**: None
- **Vision routing**: No fallback when model doesn't support images
- **Web UI**: No media rendering, composer attachments disabled
- **Setup wizard**: No multimodal configuration pages

## Solution

Eight modules delivered as a unified system behind feature flags.

---

## Module 1: Media Crate (`crates/mchact-media/`)

New crate housing all multimodal provider routers. Keeps the main binary lean when features are disabled.

**Cargo.toml:**
```toml
[package]
name = "mchact-media"
version = "0.1.0"
edition = "2021"

[features]
default = []
tts = ["msedge-tts", "opus", "ogg"]
tts-local = ["dep:espeak-ng-sys"]  # kitten_tts_rs vendored
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

# TTS
msedge-tts = { version = "0.3", optional = true }
opus = { version = "0.3", optional = true }
ogg = { version = "0.9", optional = true }

# STT local
whisper-rs = { version = "0.16", optional = true }

# Documents
kreuzberg = { version = "4.6", features = ["pdf", "office", "html"], optional = true }
```

**Module structure:**
```
crates/mchact-media/src/
  lib.rs          -- pub mod declarations
  tts.rs          -- TtsProvider trait + router
  tts_edge.rs     -- Edge TTS (msedge-tts)
  tts_kitten.rs   -- KittenTTS local (vendored or git dep)
  tts_openai.rs   -- OpenAI TTS API
  tts_elevenlabs.rs -- ElevenLabs API
  stt.rs          -- SttProvider trait + router
  stt_openai.rs   -- OpenAI Whisper API (refactored from transcribe.rs)
  stt_whisper.rs  -- whisper-rs local
  image_gen.rs    -- ImageGenProvider trait + router
  image_gen_openai.rs -- DALL-E (gpt-image-1)
  image_gen_fal.rs    -- FAL FLUX
  video_gen.rs    -- VideoGenProvider trait + router
  video_gen_sora.rs   -- OpenAI Sora 2
  video_gen_fal.rs    -- FAL video models
  video_gen_minimax.rs -- MiniMax Hailuo 2.3
  documents.rs    -- kreuzberg wrapper
  audio_encode.rs -- OGG Opus encoding for Telegram
```

---

## Module 2: Document Intelligence

**Crate:** `kreuzberg` v4.6.2 (Rust-native, 91+ formats, MIT)

### Document Extraction Storage (per-chat persistent)

Extracted content is stored per-chat to enable later recall without re-extraction. Privacy is preserved — each chat can only see its own documents. Control chats can optionally access cross-chat documents.

**New table** (migration v21):

```sql
CREATE TABLE document_extractions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    chat_id INTEGER NOT NULL,
    file_hash TEXT NOT NULL,          -- SHA-256 of file bytes
    filename TEXT NOT NULL,
    mime_type TEXT,
    file_size INTEGER,
    extracted_text TEXT NOT NULL,
    extraction_method TEXT DEFAULT 'kreuzberg',
    char_count INTEGER,
    created_at TEXT NOT NULL,
    UNIQUE(chat_id, file_hash)       -- dedup per chat
);

CREATE INDEX idx_doc_extractions_chat ON document_extractions(chat_id);
```

**Upload flow (all channels):**

```
User uploads document in Chat A
  |
  v
1. Save file to disk (existing)
2. Compute SHA-256 hash of file bytes
3. Check: SELECT extracted_text FROM document_extractions
         WHERE chat_id = ? AND file_hash = ?
   |
   +-- Cache HIT: use stored text (skip re-extraction)
   +-- Cache MISS: kreuzberg::extract_text() -> INSERT into table
   |
   v
4. Inject into message: "[document] filename=X\n\nExtracted content:\n{text}"
5. Agent processes with full document context
```

Large documents (>50K chars): truncate injected text with note `(truncated, {total_chars} chars total — use read_document tool for specific sections)`. Full text always stored in the table.

### Database methods

```rust
impl Database {
    pub fn get_document_extraction(
        &self,
        chat_id: i64,
        file_hash: &str,
    ) -> Result<Option<DocumentExtraction>, mchactError>;

    pub fn insert_document_extraction(
        &self,
        chat_id: i64,
        file_hash: &str,
        filename: &str,
        mime_type: Option<&str>,
        file_size: i64,
        extracted_text: &str,
    ) -> Result<i64, mchactError>;

    pub fn search_document_extractions(
        &self,
        chat_id: Option<i64>,  // None = all chats (control only)
        query: &str,
        limit: usize,
    ) -> Result<Vec<DocumentExtraction>, mchactError>;

    pub fn list_document_extractions(
        &self,
        chat_id: i64,
        limit: usize,
    ) -> Result<Vec<DocumentExtraction>, mchactError>;
}

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

### New tool: `read_document`

```rust
pub struct ReadDocumentTool {
    db: Arc<DynDataStore>,
    control_chat_ids: Vec<i64>,
}
```

**Modes:**

- **Extract from path**: `{ "file_path": "/path/to/file" }` — extract via kreuzberg, store in DB, return text
- **Search stored documents**: `{ "query": "budget projections" }` — search extracted_text via LIKE across this chat's documents
- **List documents**: `{ "list": true }` — list all documents uploaded to this chat
- **Retrieve by hash**: `{ "file_hash": "abc123" }` — get specific previously-extracted document

**Authorization:** Non-control chats can only access their own documents. Control chats can pass `chat_id` to access any chat's documents (same pattern as `session_search`).

**Supported formats:** PDF, DOCX, XLSX, PPTX, HTML, Markdown, plain text, email, archives (91+ via kreuzberg)

**Path guard:** Reuse existing `path_guard` module to block sensitive paths

**Feature flag:** `documents` on `mchact-media`

---

## Module 3: Text-to-Speech

**Trait:**
```rust
#[async_trait]
pub trait TtsProvider: Send + Sync {
    async fn synthesize(&self, text: &str, voice: &str) -> Result<TtsOutput, MediaError>;
    fn name(&self) -> &str;
    fn voices(&self) -> Vec<VoiceInfo>;
}

pub struct TtsOutput {
    pub audio_bytes: Vec<u8>,
    pub format: AudioFormat, // Mp3, Wav, Opus, Ogg
    pub duration_ms: Option<u64>,
}
```

**Providers:**

| Provider | Crate | Cost | Output | Offline | Default |
|---|---|---|---|---|---|
| **Edge TTS** | `msedge-tts` v0.3.0 | Free | MP3 | No | **Yes** |
| **KittenTTS** | vendored/git `kitten_tts_rs` | Free | WAV | Yes (25MB + espeak-ng) | No |
| **OpenAI TTS** | `reqwest` POST to `/v1/audio/speech` | Paid | opus/mp3 | No | No |
| **ElevenLabs** | `reqwest` POST to `/v1/text-to-speech/{voice_id}` | Paid | mp3 | No | No |

**Config:**
```yaml
tts_provider: "edge"               # edge | kitten | openai | elevenlabs
tts_voice: "en-US-AriaNeural"      # provider-specific voice name
tts_api_key: ""                     # for openai/elevenlabs
tts_elevenlabs_voice_id: ""         # elevenlabs voice ID
```

**New tool:** `text_to_speech`
- Input: `{ "text": "Hello", "voice": "optional", "language": "optional" }`
- Calls `TtsRouter::synthesize()` -> audio bytes
- Encodes to OGG Opus for Telegram via `audio_encode.rs`
- Saves to temp file, returns path
- Agent delivers via `send_message` or directly via `send_voice()`

**Audio encoding (`audio_encode.rs`):**
- Input: MP3 or WAV bytes from TTS providers
- Output: OGG Opus bytes for Telegram voice messages
- Uses `opus` v0.3.1 + `ogg` v0.9.2
- OpenAI TTS can output Opus directly (request `response_format: "opus"`) — skip encoding

**New `ChannelAdapter` trait method:**
```rust
async fn send_voice(
    &self,
    external_chat_id: &str,
    audio_bytes: &[u8],
    duration_secs: Option<u32>,
    caption: Option<&str>,
) -> Result<String, String> {
    // Default: save to temp file, call send_attachment()
    // Telegram overrides: bot.send_voice() for native voice bubble
}
```

---

## Module 4: Speech-to-Text (Enhanced)

**Refactor:** Move transcription from `telegram.rs` and `transcribe.rs` into `crates/mchact-media/src/stt.rs`.

**Trait:**
```rust
#[async_trait]
pub trait SttProvider: Send + Sync {
    async fn transcribe(&self, audio_bytes: &[u8], mime_type: &str) -> Result<String, MediaError>;
    fn name(&self) -> &str;
}
```

**Providers:**

| Provider | Method | Cost | Offline | Default |
|---|---|---|---|---|
| **whisper-rs** (local) | whisper.cpp bindings, Metal/CUDA | Free | **Yes** | **Yes** |
| **OpenAI Whisper** | Existing API call refactored | Paid | No | Fallback |

**Config:**
```yaml
stt_provider: "whisper-local"      # whisper-local | openai
stt_model: "base"                  # tiny | base | small | medium | large-v3
stt_model_path: ""                 # optional custom model file path
```

Model auto-downloads from HuggingFace on first use (~150MB for base).

**Shared flow (all channels):**
```
Voice message (any channel)
  -> download audio bytes
  -> SttRouter::transcribe(audio_bytes, mime_type)
  -> inject as "[voice message from {user}]: {text}"
```

**Audio format handling:**
- Telegram: OGG Opus -> whisper-rs needs 16kHz PCM, decode via `symphonia`
- Discord: WebM -> decode via `symphonia` or shell to `ffmpeg`
- OpenAI API: accepts raw OGG/WebM (no conversion needed)

**Channel expansion:** Discord, Slack, Feishu, Web, Matrix all gain voice transcription.

**Feature flag:** `stt-local` on `mchact-media` (for whisper-rs). OpenAI STT always available (just reqwest).

---

## Module 5: Image Generation

**Trait:**
```rust
#[async_trait]
pub trait ImageGenProvider: Send + Sync {
    async fn generate(&self, prompt: &str, params: ImageGenParams) -> Result<ImageGenOutput, MediaError>;
    fn name(&self) -> &str;
}

pub struct ImageGenParams {
    pub size: Option<String>,       // "1024x1024"
    pub quality: Option<String>,    // "standard" | "hd" | "high"
    pub n: Option<u32>,             // number of images
}

pub struct ImageGenOutput {
    pub images: Vec<GeneratedImage>,
}

pub struct GeneratedImage {
    pub data: Vec<u8>,              // image bytes
    pub format: String,             // "png", "jpeg", "webp"
    pub revised_prompt: Option<String>,
}
```

**Providers:**

| Provider | Endpoint | Price | Default |
|---|---|---|---|
| **DALL-E** (gpt-image-1) | `api.openai.com/v1/images/generations` | $0.01-0.17/img | **Yes** |
| **FAL FLUX** | `queue.fal.run/fal-ai/flux/schnell` | $0.003/img | Speed option |

**Config:**
```yaml
image_gen_provider: "openai"       # openai | flux
image_gen_api_key: ""              # defaults to main api_key for openai
image_gen_fal_key: ""              # FAL.ai API key
image_gen_default_size: "1024x1024"
image_gen_default_quality: "standard"
```

**New tool:** `image_generate`
- Input: `{ "prompt": "a sunset", "size": "1024x1024", "quality": "high", "provider": "optional" }`
- Calls provider API -> gets image bytes
- DALL-E: returns b64_json, decode to bytes
- FAL FLUX: returns URL, download bytes
- Saves to temp file, returns path
- Delivers via `send_attachment()` (auto-detected as photo on Telegram)

---

## Module 6: Video Generation

**Trait:**
```rust
#[async_trait]
pub trait VideoGenProvider: Send + Sync {
    async fn generate(&self, prompt: &str, params: VideoGenParams) -> Result<VideoGenOutput, MediaError>;
    fn name(&self) -> &str;
}

pub struct VideoGenParams {
    pub duration_secs: Option<u32>, // 5-10
    pub resolution: Option<String>, // "720p", "1080p"
}

pub struct VideoGenOutput {
    pub video_bytes: Vec<u8>,
    pub format: String,             // "mp4"
    pub duration_secs: f32,
}
```

**Providers (all queue-based: submit -> poll -> download):**

| Provider | Endpoint | Price | Default |
|---|---|---|---|
| **OpenAI Sora 2** | `api.openai.com/v1/videos/generations` | ~$0.10/s | **Yes** |
| **FAL video** | `queue.fal.run/fal-ai/{model}` | Varies | Fallback |
| **MiniMax Hailuo 2.3** | `api.minimax.io/v1/video_generation` | Varies | Fallback |

**Runtime availability check for Sora 2:** On first call, probe the endpoint. If 404/410/gone, log warning and auto-fallback to FAL. Cache result for session lifetime.

**Config:**
```yaml
video_gen_provider: "sora"         # sora | fal | minimax
video_gen_api_key: ""              # provider-specific
video_gen_fal_model: "cogvideox"   # for fal: cogvideox, kling-video, etc.
video_gen_minimax_key: ""          # MiniMax API key
video_gen_timeout_secs: 300        # video gen is slow
```

**New tool:** `video_generate`
- Input: `{ "prompt": "a cat playing piano", "duration": 5, "provider": "optional" }`
- Submit to provider -> poll every 2s -> download result
- Timeout: `video_gen_timeout_secs` (default 300)
- Saves MP4 to temp file, returns path
- Delivers via `send_video()` on channel

**Queue polling pattern (shared across all video providers):**
```rust
async fn poll_until_ready(
    client: &reqwest::Client,
    status_url: &str,
    headers: &HeaderMap,
    timeout: Duration,
) -> Result<serde_json::Value, MediaError> {
    let deadline = Instant::now() + timeout;
    loop {
        if Instant::now() > deadline {
            return Err(MediaError::Timeout);
        }
        let resp = client.get(status_url).headers(headers.clone()).send().await?;
        let body: serde_json::Value = resp.json().await?;
        let status = body.get("status").and_then(|s| s.as_str()).unwrap_or("");
        match status {
            "completed" | "succeeded" => return Ok(body),
            "failed" | "error" => return Err(MediaError::ProviderError(body.to_string())),
            _ => tokio::time::sleep(Duration::from_secs(2)).await,
        }
    }
}
```

**New `ChannelAdapter` trait method:**
```rust
async fn send_video(
    &self,
    external_chat_id: &str,
    video_path: &Path,
    caption: Option<&str>,
    duration_secs: Option<u32>,
) -> Result<String, String> {
    // Default: send_attachment() with .mp4 file
    // Telegram overrides: bot.send_video() for native video player
}
```

---

## Module 7: Vision Provider Routing

When user sends an image but the configured model doesn't support vision, route to a fallback via **OpenRouter**.

**Implementation in `src/agent_engine.rs`:**

```rust
// Before calling LLM, check if message has images
if message_has_images(&messages) && !model_supports_vision(&current_model) {
    // Route to vision fallback
    let fallback = create_vision_fallback_provider(&config);
    return fallback.send_message_with_model(system, messages, tools, model).await;
}
```

**Vision-capable model lookup:**
```rust
fn model_supports_vision(model: &str) -> bool {
    let model_lower = model.to_lowercase();
    // Anthropic: all Claude 3+
    model_lower.contains("claude-3") || model_lower.contains("claude-sonnet-4")
        || model_lower.contains("claude-opus-4")
    // OpenAI: gpt-4o+
        || model_lower.contains("gpt-4o") || model_lower.contains("gpt-4.1")
        || model_lower.contains("gpt-5") || model_lower.contains("o1") || model_lower.contains("o3")
    // Ollama vision models
        || model_lower.contains("llava") || model_lower.contains("vision")
        || model_lower.contains("qwen2.5-vl") || model_lower.contains("gemma3")
}
```

**Config:**
```yaml
vision_fallback_provider: "openrouter"
vision_fallback_model: "anthropic/claude-sonnet-4"
vision_fallback_api_key: "${OPENROUTER_API_KEY}"
vision_fallback_base_url: "https://openrouter.ai/api/v1"
```

OpenRouter uses OpenAI-compatible format — mchact's existing `OpenAiProvider` works as-is with different `base_url` + `api_key`. No new provider code needed.

**Image validation:** Replace custom `guess_image_media_type` in telegram.rs with `infer` crate (0.19.0) for MIME detection from magic bytes. Add `moka` cache (0.12.15) for downloaded images.

---

## Module 8: Frontend (Web UI)

### 8a: Media Rendering Components

**Keep `@assistant-ui/react`** as runtime. Add new components:

**`web/src/components/media/image-viewer.tsx`** (~60 lines)
- `<img>` with click-to-expand lightbox overlay
- Max width constrained to message bubble
- Loading skeleton while image loads

**`web/src/components/media/audio-player.tsx`** (~80 lines)
- HTML5 `<audio>` with custom controls (play/pause, progress bar, duration)
- Styled to match chat theme colors
- Waveform visualization optional

**`web/src/components/media/video-player.tsx`** (~60 lines)
- HTML5 `<video>` with native controls
- Max width constrained to message bubble
- Poster/thumbnail support

**`web/src/components/media/file-preview.tsx`** (~40 lines)
- File type icon + filename + size + download link
- Icons: PDF, DOCX, XLSX, image, audio, video, generic

### 8b: Message Renderer Update

**Update `web/src/components/message-components.tsx`:**
- Detect `attachments` array in message data
- Route each attachment to appropriate media component by type
- Handle inline images from markdown (render `<img>` tags)

**Update `web/src/lib/types.ts`:**
```typescript
type BackendMessage = {
  id?: string
  sender_name?: string
  content?: string
  is_from_bot?: boolean
  timestamp?: string
  attachments?: Array<{
    type: "image" | "audio" | "video" | "file"
    url: string
    mime_type?: string
    name?: string
    size?: number
  }>
}
```

### 8c: Composer Enhancements

**Update `web/src/components/thread-pane.tsx`:**
- Enable `allowAttachments: true` in Thread config
- Add attach dropdown button (AI Elements pattern):
  - Upload file
  - Upload photo
  - Record voice (MediaRecorder API)
- File preview strip below textarea showing selected attachments with remove button

### 8d: SSE Stream Events

**Update `web/src/lib/sse-parser.ts` and `web/src/hooks/use-chat-adapter.ts`:**

New event types alongside existing `delta`, `tool_start`, `tool_result`:
```
event: media
data: {"type": "image", "url": "/api/media/abc123", "mime_type": "image/png"}

event: media
data: {"type": "audio", "url": "/api/media/def456", "mime_type": "audio/ogg"}

event: media
data: {"type": "video", "url": "/api/media/ghi789", "mime_type": "video/mp4"}
```

Adapter accumulates media events and adds them to message attachments array.

### 8e: Backend API Additions (Rust, `src/web.rs`)

**`POST /api/upload`** (~80 lines)
- Multipart file upload from web composer
- Validates MIME type via `infer` crate
- Enforces size limit (configurable, default 20MB)
- Saves to `{data_dir}/uploads/{uuid}.{ext}`
- Returns `{ "media_id": "abc123", "mime_type": "image/png", "size": 12345 }`

**`GET /api/media/{id}`** (~40 lines)
- Serves uploaded/generated media files
- Sets proper `Content-Type` header from stored MIME type
- Cache headers for static content

**Updated `POST /api/send_stream`:**
- Accept optional `attachments: [{ media_id }]` in request body
- Resolve media_ids to file paths
- Pass to `process_with_agent()` as image_data (for images) or document text (for documents via kreuzberg)

---

## Module 9: Setup Wizard

**Modify `src/setup.rs`** to add multimodal configuration pages in the TUI setup wizard.

### New setup pages (after existing channel setup):

**Page: "Voice & Speech"**
```
┌─ Voice & Speech ──────────────────────────┐
│                                            │
│  Text-to-Speech          [x] Enabled       │
│  ─────────────────────────────────────     │
│  Provider                                  │
│  [x] Edge TTS (free, recommended)          │
│  [ ] KittenTTS (local, offline)            │
│  [ ] OpenAI TTS                            │
│  [ ] ElevenLabs                            │
│                                            │
│  TTS Voice: [en-US-AriaNeural         ]    │
│  TTS API Key: [                        ]   │
│  (only for OpenAI/ElevenLabs)              │
│                                            │
│  Speech-to-Text          [x] Enabled       │
│  ─────────────────────────────────────     │
│  Provider                                  │
│  [x] Whisper Local (free, recommended)     │
│  [ ] OpenAI Whisper                        │
│                                            │
│  STT Model: [base                     ]    │
│  (tiny/base/small/medium/large-v3)         │
│                                            │
└────────────────────────────────────────────┘
```

**Page: "Media Generation"**
```
┌─ Media Generation ────────────────────────┐
│                                            │
│  Image Generation        [x] Enabled       │
│  ─────────────────────────────────────     │
│  Provider                                  │
│  [x] OpenAI DALL-E (recommended)           │
│  [ ] FAL FLUX (fast, cheap)                │
│                                            │
│  Image API Key: [                      ]   │
│  FAL API Key:   [                      ]   │
│                                            │
│  Video Generation        [ ] Enabled       │
│  ─────────────────────────────────────     │
│  (requires API key — disabled by default)  │
│  Provider                                  │
│  [x] OpenAI Sora 2                         │
│  [ ] FAL Video                             │
│  [ ] MiniMax Hailuo 2.3                    │
│                                            │
│  Video API Key:    [                   ]   │
│  MiniMax API Key:  [                   ]   │
│                                            │
└────────────────────────────────────────────┘
```

**Page: "Vision & Documents"**
```
┌─ Vision & Documents ──────────────────────┐
│                                            │
│  Vision Fallback         [x] Enabled       │
│  ─────────────────────────────────────     │
│  (routes images to vision model when       │
│   primary model lacks vision support)      │
│  [x] OpenRouter                            │
│                                            │
│  Vision Model: [anthropic/claude-sonnet-4] │
│  OpenRouter Key: [                     ]   │
│                                            │
│  Document Processing     [x] Enabled       │
│  ─────────────────────────────────────     │
│  Extracts text from PDF, DOCX, XLSX,       │
│  and 90+ other formats automatically.      │
│                                            │
│  Max Document Size: [100] MB               │
│                                            │
└────────────────────────────────────────────┘
```

---

## Module 10: Web UI Settings Panel

**New settings sections** in `web/src/components/settings/`:

**`multimodal-tab.tsx`** (~200 lines)
- TTS provider selector + voice picker + API key fields
- STT provider selector + model selector
- Image gen provider selector + API key fields
- Video gen provider selector + API key fields
- Vision fallback toggle + OpenRouter config
- Document processing toggle
- All fields use existing `ConfigFieldCard` and `ConfigToggleCard` components
- Saves via existing `PATCH /api/config/update` endpoint

**Register in `settings-dialog.tsx`:** Add "Multimodal" tab alongside existing General, Model, Telegram, Discord, etc. tabs.

---

## File Map

### New files (crate)

| File | Purpose | ~Lines |
|---|---|---|
| `crates/mchact-media/Cargo.toml` | Crate config with feature flags | 40 |
| `crates/mchact-media/src/lib.rs` | Module declarations | 20 |
| `crates/mchact-media/src/tts.rs` | TtsProvider trait + TtsRouter | 80 |
| `crates/mchact-media/src/tts_edge.rs` | Edge TTS provider | 60 |
| `crates/mchact-media/src/tts_kitten.rs` | KittenTTS local provider | 80 |
| `crates/mchact-media/src/tts_openai.rs` | OpenAI TTS provider | 50 |
| `crates/mchact-media/src/tts_elevenlabs.rs` | ElevenLabs provider | 50 |
| `crates/mchact-media/src/stt.rs` | SttProvider trait + SttRouter | 60 |
| `crates/mchact-media/src/stt_openai.rs` | OpenAI Whisper (refactored) | 40 |
| `crates/mchact-media/src/stt_whisper.rs` | whisper-rs local | 80 |
| `crates/mchact-media/src/image_gen.rs` | ImageGenProvider trait + router | 60 |
| `crates/mchact-media/src/image_gen_openai.rs` | DALL-E provider | 70 |
| `crates/mchact-media/src/image_gen_fal.rs` | FAL FLUX provider | 70 |
| `crates/mchact-media/src/video_gen.rs` | VideoGenProvider trait + router + poll_until_ready | 100 |
| `crates/mchact-media/src/video_gen_sora.rs` | Sora 2 provider | 60 |
| `crates/mchact-media/src/video_gen_fal.rs` | FAL video provider | 60 |
| `crates/mchact-media/src/video_gen_minimax.rs` | MiniMax Hailuo 2.3 provider | 60 |
| `crates/mchact-media/src/documents.rs` | kreuzberg wrapper + hash computation | 60 |
| `crates/mchact-media/src/audio_encode.rs` | OGG Opus encoding | 60 |

### New files (tools)

| File | Purpose | ~Lines |
|---|---|---|
| `src/tools/text_to_speech.rs` | TTS tool | 80 |
| `src/tools/image_generate.rs` | Image gen tool | 80 |
| `src/tools/video_generate.rs` | Video gen tool | 100 |
| `src/tools/read_document.rs` | Document extraction tool (extract/search/list/retrieve) | 120 |

### New files (frontend)

| File | Purpose | ~Lines |
|---|---|---|
| `web/src/components/media/image-viewer.tsx` | Image display + lightbox | 60 |
| `web/src/components/media/audio-player.tsx` | Audio player controls | 80 |
| `web/src/components/media/video-player.tsx` | Video player | 60 |
| `web/src/components/media/file-preview.tsx` | File download card | 40 |
| `web/src/components/settings/multimodal-tab.tsx` | Settings panel | 200 |

### Modified files

| File | Change | ~Lines |
|---|---|---|
| `Cargo.toml` (root) | Add `mchact-media` to workspace | 5 |
| `crates/mchact-storage/src/db.rs` | Migration v21 (document_extractions table), DocumentExtraction struct, document CRUD + search methods | 150 |
| `src/config.rs` | Add TTS/STT/image/video/vision/document config fields with `*_enabled` toggles | 100 |
| `src/tools/mod.rs` | Register 4 new tools (conditionally based on `*_enabled` config flags) | 30 |
| `src/agent_engine.rs` | Vision routing check before LLM call | 30 |
| `src/setup.rs` | 3 new multimodal setup pages | 150 |
| `src/web.rs` | `/api/upload`, `/api/media/{id}`, media SSE events | 150 |
| `crates/mchact-channels/src/channel_adapter.rs` | Add `send_voice()`, `send_video()` trait methods | 20 |
| `src/channels/telegram.rs` | Implement `send_voice()`, `send_video()`, use shared STT | 40 |
| `src/channels/discord.rs` | Implement `send_voice()`, `send_video()`, add voice transcription | 40 |
| `src/channels/slack.rs` | Implement voice/video send methods | 20 |
| `web/src/components/thread-pane.tsx` | Enable attachments, add attach/voice buttons | 60 |
| `web/src/components/message-components.tsx` | Render media attachments | 50 |
| `web/src/lib/types.ts` | Add attachments to BackendMessage | 15 |
| `web/src/lib/sse-parser.ts` | Handle `media` event type | 15 |
| `web/src/hooks/use-chat-adapter.ts` | Accumulate media events into attachments | 30 |
| `web/src/components/settings-dialog.tsx` | Add Multimodal tab | 10 |

---

## Configuration Summary

All new config fields in `mchact.config.yaml`:

```yaml
# Text-to-Speech
tts_enabled: true                       # master toggle
tts_provider: "edge"                    # edge | kitten | openai | elevenlabs
tts_voice: "en-US-AriaNeural"          # provider-specific
tts_api_key: ""                         # for openai/elevenlabs
tts_elevenlabs_voice_id: ""

# Speech-to-Text
stt_enabled: true                       # master toggle
stt_provider: "whisper-local"           # whisper-local | openai
stt_model: "base"                       # tiny | base | small | medium | large-v3
stt_model_path: ""                      # optional custom path

# Image Generation
image_gen_enabled: true                 # master toggle
image_gen_provider: "openai"            # openai | flux
image_gen_api_key: ""                   # defaults to main api_key for openai
image_gen_fal_key: ""
image_gen_default_size: "1024x1024"
image_gen_default_quality: "standard"

# Video Generation
video_gen_enabled: false                # master toggle (off by default — requires API key)
video_gen_provider: "sora"              # sora | fal | minimax
video_gen_api_key: ""
video_gen_fal_model: "cogvideox"
video_gen_minimax_key: ""
video_gen_timeout_secs: 300

# Vision Fallback
vision_fallback_enabled: true           # master toggle
vision_fallback_provider: "openrouter"  # openrouter
vision_fallback_model: "anthropic/claude-sonnet-4"
vision_fallback_api_key: "${OPENROUTER_API_KEY}"
vision_fallback_base_url: "https://openrouter.ai/api/v1"

# Document Processing
document_extraction_enabled: true       # master toggle
max_document_size_mb: 100
```

---

## Data Flow

### Input flow (user sends media)

```
User sends voice/image/document (any channel)
  |
  +-- Voice: download audio -> SttRouter::transcribe() -> text
  +-- Image: download -> base64 encode -> ContentBlock::Image -> LLM
  +-- Document: save to disk -> kreuzberg::extract_text() -> text context
  |
  v
process_with_agent() with enriched message
```

### Output flow (agent generates media)

```
Agent calls tool (text_to_speech / image_generate / video_generate)
  |
  +-- TTS: TtsRouter::synthesize() -> audio bytes -> audio_encode -> temp file
  +-- Image: ImageGenRouter::generate() -> image bytes -> temp file
  +-- Video: VideoGenRouter::generate() -> poll -> video bytes -> temp file
  |
  v
Tool returns file path -> agent uses send_message with attachment
  |
  v
Channel delivers:
  +-- Telegram: send_voice() / send_photo() / send_video() (native)
  +-- Discord: send_attachment() (inline player)
  +-- Web: SSE media event -> frontend renders component
```

---

## Testing Strategy

### Unit tests
- TTS: each provider mock (verify request format, parse response)
- STT: each provider mock + audio format conversion
- Image gen: each provider mock
- Video gen: poll_until_ready with mock responses (pending, pending, completed)
- Documents: kreuzberg extraction on sample files
- Vision routing: model_supports_vision lookup
- Audio encoding: MP3/WAV -> OGG Opus round-trip
- FTS query sanitizer (already exists from previous spec)

### Integration tests
- TTS end-to-end: text -> audio file -> valid audio format
- STT end-to-end: sample OGG -> transcribed text
- Image gen: mock API -> valid image bytes
- Document: sample PDF/DOCX -> extracted text

### Frontend tests
- Media components render correctly for each type
- Composer attachment flow
- SSE media event handling

---

## Risks

| Risk | Mitigation |
|---|---|
| Sora 2 API may be down | Runtime probe + auto-fallback to FAL/MiniMax |
| KittenTTS requires espeak-ng system dep | Feature flag `tts-local`, documented in setup |
| whisper-rs model download on first use | Progress logging, configurable model path |
| Large audio/video files in temp dir | Cleanup via `tempfile` crate (auto-delete) or periodic sweep |
| kreuzberg adds compile time | Behind `documents` feature flag |
| OGG Opus encoding complexity | OpenAI TTS can output opus directly (skip encoding) |
| OpenRouter rate limits on vision fallback | Cache fallback result, only route when needed |
| Web UI attachment upload size | Server-side limit via config, client-side preview with size check |

---

## Implementation Phases

| Phase | Modules | Effort | Dependencies |
|---|---|---|---|
| **1** | Media crate scaffold + config fields | ~200 lines | None |
| **2** | Document intelligence (kreuzberg) | ~100 lines | Phase 1 |
| **3** | STT enhancement (shared router + whisper-rs) | ~300 lines | Phase 1 |
| **4** | TTS (Edge + KittenTTS + OpenAI + ElevenLabs + audio encoding) | ~500 lines | Phase 1 |
| **5** | Image generation (DALL-E + FAL FLUX) | ~300 lines | Phase 1 |
| **6** | Video generation (Sora 2 + FAL + MiniMax) | ~400 lines | Phase 1 |
| **7** | Vision routing (OpenRouter fallback) | ~100 lines | Phase 1 |
| **8** | Frontend media components + composer | ~570 lines | Phases 2-6 |
| **9** | Setup wizard pages | ~150 lines | Phase 1 |
| **10** | Web UI settings panel | ~200 lines | Phase 1 |

---

## Future Enhancements (out of scope)

- Retell AI voice call channel (real-time bidirectional voice)
- Smart model routing (cheap model for simple messages)
- Image editing / inpainting (DALL-E edit endpoint)
- Voice cloning (ElevenLabs / KittenTTS custom voices)
- Real-time voice streaming (WebSocket-based STT+TTS loop)
- OCR via kreuzberg PaddleOCR for scanned documents
