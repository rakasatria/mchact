use std::sync::Arc;

use serde::{Deserialize, Serialize};

use mchact_storage::db::{Database, Knowledge};

const TARGET_TOKENS_PER_CHUNK: usize = 500;

// ── Public helper functions ────────────────────────────────────────────────────

/// Naive token estimator: 1 token ≈ 4 bytes.
pub fn estimate_tokens(text: &str) -> usize {
    (text.len() + 3) / 4
}

/// Encode a slice of f32 values as little-endian bytes for embedding storage.
pub fn f32_vec_to_bytes(vec: &[f32]) -> Vec<u8> {
    vec.iter()
        .flat_map(|f| f.to_le_bytes())
        .collect()
}

/// Decode little-endian bytes back into a Vec<f32>.
#[allow(dead_code)]
fn bytes_to_f32_vec(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| {
            let arr: [u8; 4] = chunk.try_into().expect("chunk is exactly 4 bytes");
            f32::from_le_bytes(arr)
        })
        .collect()
}

/// Cosine similarity between two equal-length vectors.
/// Returns 0.0 if either vector has zero magnitude.
#[allow(dead_code)]
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len(), "vectors must have equal length");

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if mag_a == 0.0 || mag_b == 0.0 {
        return 0.0;
    }

    dot / (mag_a * mag_b)
}

/// Split `text` into logical pages / chunks.
///
/// 1. If the text contains form-feed characters (`\x0c`) — typical of PDF
///    extraction — split on those, trim each page, and drop empty ones.
/// 2. Otherwise split on blank lines (`\n\n`), then merge adjacent small
///    paragraphs until the chunk reaches ~500 tokens.
/// 3. If neither strategy produces multiple pieces, return the whole text as a
///    single-element vector.
pub fn split_into_pages(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }

    // ── Strategy 1: form-feed page separators ─────────────────────────────
    if text.contains('\x0c') {
        let pages: Vec<String> = text
            .split('\x0c')
            .map(|p| p.trim().to_owned())
            .filter(|p| !p.is_empty())
            .collect();
        if !pages.is_empty() {
            return pages;
        }
    }

    // ── Strategy 2: paragraph merging ─────────────────────────────────────
    let paragraphs: Vec<&str> = text.split("\n\n").collect();

    if paragraphs.len() <= 1 {
        return vec![text.trim().to_owned()];
    }

    let mut chunks: Vec<String> = Vec::new();
    let mut current = String::new();

    for para in paragraphs {
        let trimmed = para.trim();
        if trimmed.is_empty() {
            continue;
        }

        if current.is_empty() {
            current = trimmed.to_owned();
        } else {
            let candidate = format!("{}\n\n{}", current, trimmed);
            if estimate_tokens(&candidate) <= TARGET_TOKENS_PER_CHUNK {
                current = candidate;
            } else {
                chunks.push(current);
                current = trimmed.to_owned();
            }
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    if chunks.is_empty() {
        return vec![text.trim().to_owned()];
    }

    chunks
}

// ── Stats struct ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeStats {
    pub name: String,
    pub description: String,
    pub owner_chat_id: i64,
    pub document_count: i64,
    pub chunk_count: i64,
    pub chunks_embedded: i64,
    pub chunks_pending: i64,
    pub chunks_failed: i64,
    pub observations_done: i64,
    pub observations_pending: i64,
}

// ── KnowledgeManager ──────────────────────────────────────────────────────────

pub struct KnowledgeManager {
    db: Arc<Database>,
}

impl KnowledgeManager {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Create a new knowledge collection and grant the owner chat access.
    ///
    /// Returns the new knowledge row id.
    pub fn create(
        &self,
        name: &str,
        description: &str,
        owner_chat_id: i64,
    ) -> Result<i64, String> {
        let id = self
            .db
            .create_knowledge(name, description, owner_chat_id)
            .map_err(|e| format!("failed to create knowledge '{name}': {e}"))?;

        // Auto-grant access to the owner.
        self.db
            .add_knowledge_chat_access(id, owner_chat_id)
            .map_err(|e| format!("failed to grant owner access for knowledge '{name}': {e}"))?;

        Ok(id)
    }

    /// Add a document extraction to a knowledge collection, chunk it if no
    /// chunks exist yet, and return the number of chunks created.
    pub fn add_document(
        &self,
        knowledge_name: &str,
        doc_extraction_id: i64,
    ) -> Result<i64, String> {
        let knowledge = self.require_knowledge(knowledge_name)?;

        self.db
            .add_document_to_knowledge(knowledge.id, doc_extraction_id)
            .map_err(|e| {
                format!(
                    "failed to add document {doc_extraction_id} to '{knowledge_name}': {e}"
                )
            })?;

        let chunks_created = self.chunk_document(doc_extraction_id)?;
        Ok(chunks_created)
    }

    /// Remove a document from a knowledge collection.
    ///
    /// Existing chunks for the document are left in place (they may still be
    /// referenced by other collections or used in ongoing jobs).
    pub fn remove_document(
        &self,
        knowledge_name: &str,
        doc_extraction_id: i64,
    ) -> Result<(), String> {
        let knowledge = self.require_knowledge(knowledge_name)?;

        self.db
            .remove_document_from_knowledge(knowledge.id, doc_extraction_id)
            .map_err(|e| {
                format!(
                    "failed to remove document {doc_extraction_id} from '{knowledge_name}': {e}"
                )
            })?;

        Ok(())
    }

    /// Delete a knowledge collection. Only the owner may do this.
    pub fn delete(&self, knowledge_name: &str, caller_chat_id: i64) -> Result<(), String> {
        let knowledge = self.require_knowledge(knowledge_name)?;

        if knowledge.owner_chat_id != caller_chat_id {
            return Err(format!(
                "only the owner of '{knowledge_name}' may delete it"
            ));
        }

        self.db
            .delete_knowledge(knowledge.id)
            .map_err(|e| format!("failed to delete knowledge '{knowledge_name}': {e}"))?;

        Ok(())
    }

    /// List all knowledge collections with processing statistics.
    pub fn list_all(&self) -> Result<Vec<KnowledgeStats>, String> {
        let collections = self
            .db
            .list_knowledge()
            .map_err(|e| format!("failed to list knowledge collections: {e}"))?;

        let mut stats = Vec::with_capacity(collections.len());
        for k in collections {
            let doc_count = self
                .db
                .count_knowledge_documents(k.id)
                .map_err(|e| {
                    format!("failed to count documents for knowledge '{}': {e}", k.name)
                })?;

            let (chunk_count, chunks_embedded, chunks_pending, chunks_failed, obs_done, obs_pending) =
                self.db
                    .get_knowledge_chunk_stats(k.id)
                    .map_err(|e| {
                        format!("failed to get chunk stats for knowledge '{}': {e}", k.name)
                    })?;

            stats.push(KnowledgeStats {
                name: k.name,
                description: k.description,
                owner_chat_id: k.owner_chat_id,
                document_count: doc_count,
                chunk_count,
                chunks_embedded,
                chunks_pending,
                chunks_failed,
                observations_done: obs_done,
                observations_pending: obs_pending,
            });
        }

        Ok(stats)
    }

    /// Grant a chat access to a knowledge collection.
    pub fn attach(&self, knowledge_name: &str, chat_id: i64) -> Result<(), String> {
        let knowledge = self.require_knowledge(knowledge_name)?;

        self.db
            .add_knowledge_chat_access(knowledge.id, chat_id)
            .map_err(|e| {
                format!("failed to attach chat {chat_id} to '{knowledge_name}': {e}")
            })?;

        Ok(())
    }

    /// Check whether a chat has access to a knowledge collection.
    pub fn has_access(&self, knowledge_name: &str, chat_id: i64) -> Result<bool, String> {
        let knowledge = self.require_knowledge(knowledge_name)?;

        self.db
            .has_knowledge_chat_access(knowledge.id, chat_id)
            .map_err(|e| {
                format!(
                    "failed to check access for chat {chat_id} in '{knowledge_name}': {e}"
                )
            })
    }

    // ── Private helpers ────────────────────────────────────────────────────────

    fn require_knowledge(&self, name: &str) -> Result<Knowledge, String> {
        self.db
            .get_knowledge_by_name(name)
            .map_err(|e| format!("DB error looking up knowledge '{name}': {e}"))?
            .ok_or_else(|| format!("knowledge collection '{name}' not found"))
    }

    /// Split the extracted text for `doc_extraction_id` into pages and insert
    /// each as a `document_chunk` row.  Returns the number of chunks created.
    ///
    /// If chunks already exist for this document, this is a no-op and returns 0.
    fn chunk_document(&self, doc_extraction_id: i64) -> Result<i64, String> {
        // Skip if already chunked.
        let existing = self
            .db
            .get_chunks_for_document(doc_extraction_id)
            .map_err(|e| {
                format!("failed to check existing chunks for doc {doc_extraction_id}: {e}")
            })?;

        if !existing.is_empty() {
            return Ok(0);
        }

        let extraction = self
            .db
            .get_document_extraction_by_id(doc_extraction_id)
            .map_err(|e| format!("failed to load document extraction {doc_extraction_id}: {e}"))?
            .ok_or_else(|| {
                format!("document extraction {doc_extraction_id} not found")
            })?;

        let pages = split_into_pages(&extraction.extracted_text);

        let mut created: i64 = 0;
        for (idx, page_text) in pages.iter().enumerate() {
            let tokens = estimate_tokens(page_text) as i64;
            self.db
                .insert_document_chunk(
                    doc_extraction_id,
                    idx as i64 + 1,
                    page_text,
                    Some(tokens),
                )
                .map_err(|e| {
                    format!(
                        "failed to insert chunk {} for doc {doc_extraction_id}: {e}",
                        idx + 1
                    )
                })?;
            created += 1;
        }

        Ok(created)
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_into_pages_with_formfeed() {
        let text = "Page one content\x0cPage two content\x0cPage three content";
        let pages = split_into_pages(text);
        assert_eq!(pages.len(), 3);
        assert_eq!(pages[0], "Page one content");
        assert_eq!(pages[1], "Page two content");
        assert_eq!(pages[2], "Page three content");
    }

    #[test]
    fn test_split_into_pages_no_formfeed_short_text() {
        // Short text with no double-newlines → single section.
        let text = "This is a short paragraph with no blank lines.";
        let pages = split_into_pages(text);
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0], text);
    }

    #[test]
    fn test_split_into_pages_long_text() {
        // Build two paragraphs that each exceed 500 tokens so they stay separate.
        let big_para = "word ".repeat(600); // ≈ 600*5/4 = 750 tokens each
        let text = format!("{}\n\n{}", big_para.trim(), big_para.trim());
        let pages = split_into_pages(&text);
        // Each paragraph is too large to merge, so we expect 2 chunks.
        assert_eq!(pages.len(), 2);
    }

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abcd"), 1);   // 4 chars → (4+3)/4 = 1
        assert_eq!(estimate_tokens("abcde"), 2);  // 5 chars → (5+3)/4 = 2
        assert_eq!(estimate_tokens("hello world"), 3); // 11 chars → (11+3)/4 = 3
    }

    #[test]
    fn test_split_empty() {
        let pages = split_into_pages("");
        assert!(pages.is_empty());
    }

    #[test]
    fn test_f32_vec_roundtrip() {
        let original = vec![1.0_f32, -0.5, 0.25, 3.14];
        let bytes = f32_vec_to_bytes(&original);
        let recovered = bytes_to_f32_vec(&bytes);
        assert_eq!(original.len(), recovered.len());
        for (a, b) in original.iter().zip(recovered.iter()) {
            assert!((a - b).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let v = vec![1.0_f32, 0.0, 0.0];
        assert!((cosine_similarity(&v, &v) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0_f32, 0.0];
        let b = vec![0.0_f32, 1.0];
        assert!((cosine_similarity(&a, &b)).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_zero_vector() {
        let a = vec![0.0_f32, 0.0];
        let b = vec![1.0_f32, 0.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn test_split_into_pages_formfeed_filters_empty() {
        // Consecutive form-feeds produce empty pages that should be dropped.
        let text = "First\x0c\x0cThird";
        let pages = split_into_pages(text);
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0], "First");
        assert_eq!(pages[1], "Third");
    }

    #[test]
    fn test_split_paragraphs_merged_up_to_limit() {
        // Each paragraph is about 50 tokens, so several should be merged.
        let para = "word ".repeat(50); // ≈ 62 tokens per paragraph
        let text = (0..5)
            .map(|_| para.trim().to_owned())
            .collect::<Vec<_>>()
            .join("\n\n");
        let pages = split_into_pages(&text);
        // 5 × ~62 = ~310 tokens total; should fit in fewer than 5 chunks.
        assert!(pages.len() < 5);
        assert!(!pages.is_empty());
    }
}
