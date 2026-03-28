# Media Storage Backends — Design Spec

**Status:** Draft
**Date:** 2026-03-27

---

## Overview

Replace mchact's hardcoded local filesystem media storage with a pluggable backend system supporting local, S3-compatible, Azure Blob, and Google Cloud Storage. All media files tracked in a `media_objects` database table. LRU cache layer for cloud backends. Tool output uses `media_object_id` references instead of raw filesystem paths.

**Motivation:** Production readiness — multi-instance deployment, durable cloud storage, CDN-friendly serving, cost-efficient scaling, deduplication.

---

## 1. ObjectStorage Trait

New crate: `crates/mchact-storage-backend/`

```rust
#[async_trait]
pub trait ObjectStorage: Send + Sync {
    /// Store bytes at the given key.
    async fn put(&self, key: &str, data: &[u8], mime_type: Option<&str>) -> Result<(), StorageError>;

    /// Retrieve bytes for the given key.
    async fn get(&self, key: &str) -> Result<Vec<u8>, StorageError>;

    /// Delete the object at the given key.
    async fn delete(&self, key: &str) -> Result<(), StorageError>;

    /// Check if an object exists at the given key.
    async fn exists(&self, key: &str) -> Result<bool, StorageError>;

    /// Backend identifier: "local", "s3", "azure", "gcs"
    fn backend_name(&self) -> &str;
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Object not found: {0}")]
    NotFound(String),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("Backend error: {0}")]
    Backend(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
```

### Implementations

#### LocalStorage

- Reads/writes to `{data_dir}/{key}`
- Creates parent directories on `put()`
- Zero dependencies beyond std
- Always compiled (default feature)

#### S3Storage

- Uses `aws-sdk-s3` + `aws-config` crates
- Supports: AWS S3, MinIO, Cloudflare R2, Backblaze B2, DigitalOcean Spaces, Wasabi
- Custom endpoint URL for S3-compatible services
- Credentials: config fields OR `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` env vars
- Feature-gated: `--features s3`

#### AzureBlobStorage

- Uses `azure_storage_blobs` + `azure_storage` crates
- Credentials: connection string OR account name + key
- Env fallback: `AZURE_STORAGE_CONNECTION_STRING`
- Feature-gated: `--features azure`

#### GcsStorage

- Uses `google-cloud-storage` crate
- Credentials: service account JSON file path
- Env fallback: `GOOGLE_APPLICATION_CREDENTIALS`
- Feature-gated: `--features gcs`

### Crate Structure

```
crates/mchact-storage-backend/
├── Cargo.toml
├── src/
│   ├── lib.rs          # ObjectStorage trait, StorageError, factory function
│   ├── local.rs        # LocalStorage
│   ├── s3.rs           # S3Storage (feature = "s3")
│   ├── azure.rs        # AzureBlobStorage (feature = "azure")
│   ├── gcs.rs          # GcsStorage (feature = "gcs")
│   └── cache.rs        # CachedStorage wrapper
```

```toml
[features]
default = ["local"]
local = []
s3 = ["aws-sdk-s3", "aws-config"]
azure = ["azure_storage_blobs", "azure_storage"]
gcs = ["google-cloud-storage"]
```

---

## 2. LRU Cache Layer

`CachedStorage` wraps any `ObjectStorage` backend:

```rust
pub struct CachedStorage {
    backend: Box<dyn ObjectStorage>,
    cache_dir: PathBuf,
    max_cache_bytes: u64,
    metadata: Mutex<CacheMetadata>,
}

struct CacheMetadata {
    entries: HashMap<String, CacheEntry>,
    total_bytes: u64,
}

struct CacheEntry {
    size_bytes: u64,
    last_accessed: SystemTime,
}
```

### Behavior

- **`get(key)`:** Check `{cache_dir}/{key}`. If exists, update `last_accessed`, return bytes. If miss, fetch from backend, write to cache, evict if over limit.
- **`put(key, data)`:** Write to `{cache_dir}/{key}` AND `backend.put()`. Write-through ensures cloud has the data immediately.
- **`delete(key)`:** Delete from cache AND backend.
- **Eviction:** When `total_bytes > max_cache_bytes`, evict entries with oldest `last_accessed` until under limit.
- **Metadata persistence:** `{cache_dir}/.cache_meta.json` — loaded on startup, saved after mutations.

### Configuration

```yaml
storage:
  cache_max_size_mb: 1024    # Default 1GB. Set 0 to disable cache.
```

When `backend = "local"`, CachedStorage is bypassed (LocalStorage used directly — no double-write).

---

## 3. Database Schema

### New Table: `media_objects` (Migration v22)

```sql
CREATE TABLE media_objects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    object_key TEXT NOT NULL UNIQUE,
    storage_backend TEXT NOT NULL DEFAULT 'local',
    original_chat_id INTEGER NOT NULL,
    mime_type TEXT,
    size_bytes INTEGER,
    sha256_hash TEXT,
    source TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX idx_media_objects_chat ON media_objects(original_chat_id);
CREATE INDEX idx_media_objects_hash ON media_objects(sha256_hash);
```

Fields:
- `object_key`: Relative path like `media/img_abc123.png` or `uploads/def456.pdf` (not absolute)
- `storage_backend`: `"local"`, `"s3"`, `"azure"`, `"gcs"`
- `original_chat_id`: Chat that created this file
- `source`: `"upload"` | `"image_gen"` | `"video_gen"` | `"tts"` | `"document"`
- `sha256_hash`: For deduplication

### Modified: `document_extractions`

```sql
ALTER TABLE document_extractions ADD COLUMN media_object_id INTEGER REFERENCES media_objects(id);
CREATE INDEX idx_doc_extractions_media ON document_extractions(media_object_id);
```

Nullable — legacy rows have `NULL` until backfill runs.

### CRUD Methods (in `crates/mchact-storage/src/db.rs`)

```rust
pub fn insert_media_object(&self, key: &str, backend: &str, chat_id: i64, mime: Option<&str>, size: Option<i64>, hash: Option<&str>, source: &str) -> Result<i64>;
pub fn get_media_object(&self, id: i64) -> Result<Option<MediaObject>>;
pub fn get_media_object_by_key(&self, key: &str) -> Result<Option<MediaObject>>;
pub fn get_media_object_by_hash(&self, hash: &str) -> Result<Option<MediaObject>>;
pub fn list_media_objects_for_chat(&self, chat_id: i64) -> Result<Vec<MediaObject>>;
pub fn delete_media_object(&self, id: i64) -> Result<()>;
pub fn set_document_extraction_media_id(&self, extraction_id: i64, media_object_id: i64) -> Result<()>;
```

---

## 4. MediaManager Service

Central service wiring storage + DB:

```rust
pub struct MediaManager {
    storage: Arc<dyn ObjectStorage>,
    db: Arc<Database>,
}

impl MediaManager {
    /// Store a file: compute hash, dedup check, write to storage, insert DB row.
    pub async fn store_file(
        &self,
        data: &[u8],
        filename: &str,
        mime_type: Option<&str>,
        chat_id: i64,
        source: &str,
    ) -> Result<i64, String>;  // returns media_object_id

    /// Retrieve file bytes by media_object_id.
    pub async fn get_file(&self, media_object_id: i64) -> Result<(Vec<u8>, MediaObject), String>;

    /// Delete a media object (storage + DB).
    pub async fn delete_file(&self, media_object_id: i64) -> Result<(), String>;

    /// List media for a chat.
    pub fn list_for_chat(&self, chat_id: i64) -> Result<Vec<MediaObject>, String>;
}
```

### Deduplication Flow

```
store_file(data, filename, mime, chat_id, source)
  1. hash = sha256(data)
  2. existing = db.get_media_object_by_hash(hash)
  3. if existing → return existing.id  (no re-upload)
  4. key = "{source_prefix}/{uuid}.{ext}"
  5. storage.put(key, data, mime)
  6. id = db.insert_media_object(key, backend, chat_id, mime, size, hash, source)
  7. return id
```

Source prefix mapping:
- `upload` → `uploads/`
- `image_gen` → `media/img_`
- `video_gen` → `media/vid_`
- `tts` → `media/tts_`
- `document` → `documents/`

---

## 5. Tool Output Changes

### Before (current)

```rust
// image_generate tool
let path = format!("{}/media/img_{}.png", data_dir, uuid);
std::fs::write(&path, &bytes)?;
return ToolResult { content: format!("Image saved to {path}"), .. };
```

Web UI regex scrapes path from text.

### After

```rust
// image_generate tool
let media_id = media_manager.store_file(&bytes, &filename, Some("image/png"), chat_id, "image_gen").await?;
return ToolResult {
    content: json!({"media_object_id": media_id, "type": "image", "mime_type": "image/png"}).to_string(),
    metadata: Some(json!({"media_object_id": media_id})),
    ..
};
```

### Affected Tools

| Tool | Current Output | New Output |
|------|---------------|------------|
| `image_generate` | Path string | `{"media_object_id": N, "type": "image"}` |
| `video_generate` | Path string | `{"media_object_id": N, "type": "video"}` |
| `text_to_speech` | Path string | `{"media_object_id": N, "type": "audio"}` |
| `read_document` | Extracted text | Extracted text + `{"media_object_id": N}` in metadata |
| Web upload handler | Returns path | Returns `{"media_id": N, "url": "/api/media/N"}` |

---

## 6. Web API Changes

### `GET /api/media/:id` (modified)

```
1. Parse id as media_object_id (integer)
2. Lookup media_objects row
3. Resolve bytes:
   a. If CachedStorage: check cache → serve
   b. If cache miss: fetch from backend → cache → serve
   c. If LocalStorage: read directly
4. Set Content-Type from mime_type
5. Return bytes
```

Replaces current path-based lookup (`/api/media/img_abc.png`).

### `POST /api/upload` (modified)

```
1. Receive multipart file
2. Call media_manager.store_file(bytes, filename, mime, chat_id, "upload")
3. Return {"media_id": N, "url": "/api/media/N", "mime_type": "...", "size": N}
```

### Backward Compatibility

Keep the old path-based `/api/media/:filename` endpoint as a fallback for existing messages that contain file paths. It searches local disk only. New messages use integer IDs.

---

## 7. Migration v22

### Phase 1: Schema

```sql
-- Create media_objects table
CREATE TABLE IF NOT EXISTS media_objects (...);

-- Add media_object_id to document_extractions
ALTER TABLE document_extractions ADD COLUMN media_object_id INTEGER REFERENCES media_objects(id);
CREATE INDEX IF NOT EXISTS idx_doc_extractions_media ON document_extractions(media_object_id);
```

### Phase 2: Backfill (runs once on first startup after upgrade)

```rust
fn backfill_media_objects(db: &Database, data_dir: &str) -> Result<()> {
    // 1. Scan {data_dir}/media/* → insert media_objects
    //    - img_* → source: "image_gen"
    //    - vid_* → source: "video_gen"
    //    - tts_* → source: "tts"
    //    - Compute SHA256, file size, mime type from extension
    //    - storage_backend = "local", original_chat_id = 0 (unknown for legacy)

    // 2. Scan {data_dir}/uploads/* → insert media_objects
    //    - source: "upload"

    // 3. Match document_extractions to media_objects by file_hash = sha256_hash
    //    - UPDATE document_extractions SET media_object_id = ? WHERE file_hash = ?
}
```

Legacy files with `original_chat_id = 0` can be updated if chat context is discoverable from message content.

---

## 8. Configuration

```yaml
storage:
  backend: "local"                # "local" | "s3" | "azure" | "gcs"
  cache_max_size_mb: 1024         # LRU cache limit (0 = disabled)

  s3:
    bucket: "mchact-media"
    region: "us-east-1"
    endpoint: null                # For MinIO, R2, etc.
    access_key_id: null           # Or AWS_ACCESS_KEY_ID env
    secret_access_key: null       # Or AWS_SECRET_ACCESS_KEY env

  azure:
    container: "mchact-media"
    connection_string: null       # Or AZURE_STORAGE_CONNECTION_STRING env
    account_name: null
    account_key: null

  gcs:
    bucket: "mchact-media"
    credentials_path: null        # Or GOOGLE_APPLICATION_CREDENTIALS env
```

Config fields with serde defaults:
```rust
#[serde(default = "default_storage_backend")]
pub storage_backend: String,                    // "local"
#[serde(default = "default_storage_cache_max_mb")]
pub storage_cache_max_size_mb: u64,             // 1024
pub storage_s3_bucket: Option<String>,
pub storage_s3_region: Option<String>,
pub storage_s3_endpoint: Option<String>,
pub storage_s3_access_key_id: Option<String>,
pub storage_s3_secret_access_key: Option<String>,
pub storage_azure_container: Option<String>,
pub storage_azure_connection_string: Option<String>,
pub storage_azure_account_name: Option<String>,
pub storage_azure_account_key: Option<String>,
pub storage_gcs_bucket: Option<String>,
pub storage_gcs_credentials_path: Option<String>,
```

---

## 9. New Files

| File | Purpose | Lines (est) |
|------|---------|-------------|
| `crates/mchact-storage-backend/src/lib.rs` | ObjectStorage trait, StorageError, factory | ~80 |
| `crates/mchact-storage-backend/src/local.rs` | LocalStorage implementation | ~80 |
| `crates/mchact-storage-backend/src/s3.rs` | S3Storage (feature-gated) | ~120 |
| `crates/mchact-storage-backend/src/azure.rs` | AzureBlobStorage (feature-gated) | ~120 |
| `crates/mchact-storage-backend/src/gcs.rs` | GcsStorage (feature-gated) | ~120 |
| `crates/mchact-storage-backend/src/cache.rs` | CachedStorage LRU wrapper | ~200 |
| `src/media_manager.rs` | MediaManager service (store/get/delete/list) | ~200 |

## 10. Modified Files

| File | Changes |
|------|---------|
| `Cargo.toml` | Add `mchact-storage-backend` workspace member + dependency |
| `crates/mchact-storage/src/db.rs` | Migration v22, media_objects CRUD, document_extractions FK |
| `src/config.rs` | Storage config fields (13 new fields) |
| `src/runtime.rs` | Initialize MediaManager from config, inject into AppState |
| `src/tools/image_generate.rs` | Use MediaManager instead of fs::write |
| `src/tools/video_generate.rs` | Use MediaManager instead of fs::write |
| `src/tools/text_to_speech.rs` | Use MediaManager instead of fs::write |
| `src/tools/read_document.rs` | Set media_object_id on extractions |
| `src/web.rs` | Update /api/upload and /api/media endpoints |
| `src/tools/mod.rs` | Pass MediaManager to tools that need it |
| `tests/config_validation.rs` | Add storage config fields to test constructor |

## 11. Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| Cloud SDK compile times | Feature-gated — only compile what you use |
| Migration breaks existing paths in messages | Keep old path-based /api/media/:filename as fallback |
| S3 credentials leak | Config fields accept env var names, not raw secrets |
| Cache corruption | Metadata checksum validation on load, rebuild from backend on mismatch |
| Large file upload OOM | Stream to storage without buffering entire file (for cloud backends) |
| Backfill slow on large media dirs | Run async with progress logging, non-blocking |
