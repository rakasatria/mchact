use crate::MediaError;
use sha2::{Digest, Sha256};

pub fn compute_file_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

#[cfg(feature = "documents")]
pub async fn extract_text(file_path: &str) -> Result<String, MediaError> {
    let path = file_path.to_string();
    tokio::task::spawn_blocking(move || {
        let data = std::fs::read(&path)
            .map_err(|e| MediaError::ProviderError(format!("Failed to read file: {e}")))?;
        let mime = infer::get(&data)
            .map(|t| t.mime_type().to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string());
        let config = kreuzberg::ExtractionConfig::default();
        let rt = tokio::runtime::Handle::current();
        let result = rt.block_on(kreuzberg::extract_bytes(&data, &mime, &config))
            .map_err(|e| MediaError::ProviderError(format!("kreuzberg extraction failed: {e}")))?;
        Ok(result.text().to_string())
    })
    .await
    .map_err(|e| MediaError::ProviderError(format!("Task failed: {e}")))?
}

#[cfg(not(feature = "documents"))]
pub async fn extract_text(_file_path: &str) -> Result<String, MediaError> {
    Err(MediaError::NotConfigured(
        "Document extraction requires 'documents' feature".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_file_hash_consistent() {
        let data = b"hello world";
        let h1 = compute_file_hash(data);
        let h2 = compute_file_hash(data);
        assert_eq!(h1, h2);
        assert!(!h1.is_empty());
    }

    #[test]
    fn test_compute_file_hash_different_inputs() {
        let h1 = compute_file_hash(b"foo");
        let h2 = compute_file_hash(b"bar");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_compute_file_hash_empty() {
        let h = compute_file_hash(b"");
        // SHA-256 of empty string is a known value
        assert_eq!(
            h,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_compute_file_hash_hex_format() {
        let h = compute_file_hash(b"test");
        // Should be 64 hex chars
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[tokio::test]
    async fn test_extract_text_not_configured_without_feature() {
        // When compiled without the 'documents' feature, extract_text returns NotConfigured
        #[cfg(not(feature = "documents"))]
        {
            let result = extract_text("/tmp/test.pdf").await;
            assert!(result.is_err());
            let err = result.unwrap_err().to_string();
            assert!(err.contains("documents") || err.contains("configured"));
        }
        // When compiled with the 'documents' feature, this test is skipped
        #[cfg(feature = "documents")]
        {
            // Feature is enabled; no-op for this test
        }
    }
}
