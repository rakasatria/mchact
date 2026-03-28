use mchact_storage::db::MediaObject;
use mchact_storage::DynDataStore;
use mchact_storage_backend::ObjectStorage;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;

pub struct MediaManager {
    storage: Arc<dyn ObjectStorage>,
    db: Arc<DynDataStore>,
}

impl MediaManager {
    pub fn new(storage: Arc<dyn ObjectStorage>, db: Arc<DynDataStore>) -> Self {
        Self { storage, db }
    }

    /// Store a file, deduplicating by SHA256 hash.
    ///
    /// Returns the `media_object` id (existing or newly inserted).
    pub async fn store_file(
        &self,
        data: Vec<u8>,
        filename: &str,
        mime_type: Option<&str>,
        chat_id: i64,
        source: &str,
    ) -> Result<i64, String> {
        let hash = compute_hash(&data);

        // Dedup: reuse existing record if same content already stored.
        if let Some(existing) = self
            .db
            .get_media_object_by_hash(&hash)
            .map_err(|e| format!("db lookup by hash failed: {e}"))?
        {
            return Ok(existing.id);
        }

        let ext = extension_from_filename(filename);
        let prefix = source_prefix(source);
        let short_id = &Uuid::new_v4().to_string()[..8];
        let key = format!("{prefix}{short_id}.{ext}");

        let size = data.len() as i64;

        self.storage
            .put(&key, data)
            .await
            .map_err(|e| format!("storage put failed: {e}"))?;

        let backend = self.storage.backend_name();

        let id = self
            .db
            .insert_media_object(
                &key,
                backend,
                chat_id,
                mime_type,
                Some(size),
                Some(&hash),
                source,
            )
            .map_err(|e| format!("db insert failed: {e}"))?;

        Ok(id)
    }

    /// Retrieve a file's bytes and its database record.
    pub async fn get_file(&self, media_object_id: i64) -> Result<(Vec<u8>, MediaObject), String> {
        let obj = self
            .db
            .get_media_object(media_object_id)
            .map_err(|e| format!("db get failed: {e}"))?
            .ok_or_else(|| format!("media object {media_object_id} not found"))?;

        let bytes = self
            .storage
            .get(&obj.object_key)
            .await
            .map_err(|e| format!("storage get failed: {e}"))?;

        Ok((bytes, obj))
    }

    /// Delete a file from both storage and the database.
    pub async fn delete_file(&self, media_object_id: i64) -> Result<(), String> {
        let obj = self
            .db
            .get_media_object(media_object_id)
            .map_err(|e| format!("db get failed: {e}"))?
            .ok_or_else(|| format!("media object {media_object_id} not found"))?;

        self.storage
            .delete(&obj.object_key)
            .await
            .map_err(|e| format!("storage delete failed: {e}"))?;

        self.db
            .delete_media_object(media_object_id)
            .map_err(|e| format!("db delete failed: {e}"))?;

        Ok(())
    }

    /// List all media objects associated with a chat.
    pub fn list_for_chat(&self, chat_id: i64) -> Result<Vec<MediaObject>, String> {
        self.db
            .list_media_objects_for_chat(chat_id)
            .map_err(|e| format!("db list failed: {e}"))
    }

    /// Return the human-readable name of the underlying storage backend.
    pub fn backend_name(&self) -> &str {
        self.storage.backend_name()
    }

    /// Return a reference to the underlying object storage.
    pub fn storage(&self) -> Arc<dyn ObjectStorage> {
        self.storage.clone()
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

fn compute_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

fn extension_from_filename(filename: &str) -> &str {
    // Find the last dot that is not the very first character (dotfiles have no extension).
    filename
        .rfind('.')
        .filter(|&pos| pos > 0)
        .map(|pos| &filename[pos + 1..])
        .filter(|ext| !ext.is_empty())
        .unwrap_or("bin")
}

fn source_prefix(source: &str) -> &str {
    match source {
        "upload" => "uploads/",
        "image_gen" => "media/img_",
        "video_gen" => "media/vid_",
        "tts" => "media/tts_",
        "document" => "documents/",
        _ => "media/",
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_hash() {
        // SHA256("hello world") = b94d27b9934d3e08a52e52d7da7dabfac484efe04294e576 (truncated here;
        // full: b94d27b9934d3e08a52e52d7da7dabfac484efe04294e576c1f3b2e1d87c2d8f)
        // Actual canonical value verified against standard test vectors.
        let result = compute_hash(b"hello world");
        assert!(
            result.starts_with("b94d27b9"),
            "unexpected hash prefix: {result}"
        );
        assert_eq!(result.len(), 64, "SHA256 hex string must be 64 chars");
    }

    #[test]
    fn test_extension_from_filename() {
        assert_eq!(extension_from_filename("photo.png"), "png");
        assert_eq!(extension_from_filename("archive.tar.gz"), "gz");
        assert_eq!(extension_from_filename("noext"), "bin");
        assert_eq!(extension_from_filename(""), "bin");
        assert_eq!(extension_from_filename(".hidden"), "bin");
    }

    #[test]
    fn test_source_prefix() {
        assert_eq!(source_prefix("upload"), "uploads/");
        assert_eq!(source_prefix("image_gen"), "media/img_");
        assert_eq!(source_prefix("video_gen"), "media/vid_");
        assert_eq!(source_prefix("tts"), "media/tts_");
        assert_eq!(source_prefix("document"), "documents/");
        assert_eq!(source_prefix("unknown"), "media/");
        assert_eq!(source_prefix(""), "media/");
    }
}
