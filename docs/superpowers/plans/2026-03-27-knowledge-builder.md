# Knowledge Builder Implementation Plan (Plan E)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add knowledge management with named document collections, per-page chunking, vector embeddings, observation engine integration, scheduled processing (embed/observe/auto-group), multi-knowledge query with citations, and CLI/web interfaces.

**Architecture:** New `knowledge` + `knowledge_documents` + `knowledge_chat_access` + `document_chunks` tables (migration v23). `KnowledgeManager` service handles CRUD + chunking + query. Three scheduled jobs process chunks asynchronously (embed, observe, auto-group). 7 agent tools for conversational use. CLI subcommand + web API for inspection.

**Tech Stack:** Rust (serde_json, rusqlite, reqwest for embeddings), existing `EmbeddingProvider` trait, existing `mchact-memory` observation engine, existing scheduler

**Spec:** `docs/superpowers/specs/2026-03-27-knowledge-builder-design.md`

**Depends on:** Plan D (media_objects + MediaManager), mchact-memory crate, embedding provider

---

### Task 1: Migration v23 — Knowledge Tables + Chunk CRUD

**Files:**
- Modify: `crates/mchact-storage/src/db.rs`

- [ ] **Step 1: Add migration v23**

Find the migrations list in db.rs (after v22). Add:

```rust
("v23", &[
    "CREATE TABLE IF NOT EXISTS knowledge (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL UNIQUE,
        description TEXT DEFAULT '',
        owner_chat_id INTEGER NOT NULL,
        last_grouping_check_at TEXT,
        document_count_at_last_check INTEGER DEFAULT 0,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS idx_knowledge_owner ON knowledge(owner_chat_id)",
    "CREATE TABLE IF NOT EXISTS knowledge_documents (
        knowledge_id INTEGER NOT NULL REFERENCES knowledge(id) ON DELETE CASCADE,
        document_extraction_id INTEGER NOT NULL REFERENCES document_extractions(id),
        added_at TEXT NOT NULL,
        PRIMARY KEY (knowledge_id, document_extraction_id)
    )",
    "CREATE TABLE IF NOT EXISTS knowledge_chat_access (
        knowledge_id INTEGER NOT NULL REFERENCES knowledge(id) ON DELETE CASCADE,
        chat_id INTEGER NOT NULL,
        attached_at TEXT NOT NULL,
        PRIMARY KEY (knowledge_id, chat_id)
    )",
    "CREATE INDEX IF NOT EXISTS idx_knowledge_access_chat ON knowledge_chat_access(chat_id)",
    "CREATE TABLE IF NOT EXISTS document_chunks (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        document_extraction_id INTEGER NOT NULL REFERENCES document_extractions(id) ON DELETE CASCADE,
        page_number INTEGER NOT NULL,
        text TEXT NOT NULL,
        token_count INTEGER,
        embedding BLOB,
        embedding_status TEXT NOT NULL DEFAULT 'pending',
        observation_status TEXT NOT NULL DEFAULT 'pending',
        created_at TEXT NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS idx_chunks_extraction ON document_chunks(document_extraction_id)",
    "CREATE INDEX IF NOT EXISTS idx_chunks_embedding_status ON document_chunks(embedding_status)",
    "CREATE INDEX IF NOT EXISTS idx_chunks_observation_status ON document_chunks(observation_status)",
]),
```

- [ ] **Step 2: Add data structs**

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Knowledge {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub owner_chat_id: i64,
    pub last_grouping_check_at: Option<String>,
    pub document_count_at_last_check: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DocumentChunk {
    pub id: i64,
    pub document_extraction_id: i64,
    pub page_number: i64,
    pub text: String,
    pub token_count: Option<i64>,
    pub embedding: Option<Vec<u8>>,
    pub embedding_status: String,
    pub observation_status: String,
    pub created_at: String,
}
```

- [ ] **Step 3: Add Knowledge CRUD methods**

```rust
impl Database {
    pub fn create_knowledge(&self, name: &str, description: &str, owner_chat_id: i64) -> Result<i64, rusqlite::Error>;
    pub fn get_knowledge_by_name(&self, name: &str) -> Result<Option<Knowledge>, rusqlite::Error>;
    pub fn list_knowledge(&self) -> Result<Vec<Knowledge>, rusqlite::Error>;
    pub fn delete_knowledge(&self, id: i64) -> Result<(), rusqlite::Error>;
    pub fn update_knowledge_timestamp(&self, id: i64) -> Result<(), rusqlite::Error>;

    // Knowledge documents junction
    pub fn add_document_to_knowledge(&self, knowledge_id: i64, doc_extraction_id: i64) -> Result<(), rusqlite::Error>;
    pub fn remove_document_from_knowledge(&self, knowledge_id: i64, doc_extraction_id: i64) -> Result<(), rusqlite::Error>;
    pub fn list_knowledge_documents(&self, knowledge_id: i64) -> Result<Vec<i64>, rusqlite::Error>; // returns doc_extraction_ids
    pub fn count_knowledge_documents(&self, knowledge_id: i64) -> Result<i64, rusqlite::Error>;

    // Chat access
    pub fn add_knowledge_chat_access(&self, knowledge_id: i64, chat_id: i64) -> Result<(), rusqlite::Error>;
    pub fn has_knowledge_chat_access(&self, knowledge_id: i64, chat_id: i64) -> Result<bool, rusqlite::Error>;
    pub fn list_knowledge_for_chat(&self, chat_id: i64) -> Result<Vec<Knowledge>, rusqlite::Error>;
    pub fn list_knowledge_chat_ids(&self, knowledge_id: i64) -> Result<Vec<i64>, rusqlite::Error>;

    // Document chunks
    pub fn insert_document_chunk(&self, doc_extraction_id: i64, page_number: i64, text: &str, token_count: Option<i64>) -> Result<i64, rusqlite::Error>;
    pub fn get_chunks_by_status(&self, embedding_status: &str, limit: i64) -> Result<Vec<DocumentChunk>, rusqlite::Error>;
    pub fn get_chunks_for_observation(&self, limit: i64) -> Result<Vec<DocumentChunk>, rusqlite::Error>; // embedding=done AND observation=pending
    pub fn update_chunk_embedding(&self, chunk_id: i64, embedding: &[u8], status: &str) -> Result<(), rusqlite::Error>;
    pub fn update_chunk_observation_status(&self, chunk_id: i64, status: &str) -> Result<(), rusqlite::Error>;
    pub fn get_chunks_for_document(&self, doc_extraction_id: i64) -> Result<Vec<DocumentChunk>, rusqlite::Error>;
    pub fn reset_failed_chunks(&self, older_than_mins: i64) -> Result<i64, rusqlite::Error>;

    // Knowledge grouping tracking
    pub fn update_knowledge_grouping_check(&self, knowledge_id: i64, doc_count: i64) -> Result<(), rusqlite::Error>;
    pub fn get_knowledge_needing_grouping(&self, min_docs: i64) -> Result<Vec<Knowledge>, rusqlite::Error>;
}
```

Each method follows existing patterns in db.rs (use `self.conn()`, `rusqlite::params![]`, `chrono::Utc::now().to_rfc3339()`).

- [ ] **Step 4: Run tests**

Run: `cargo build`
Expected: Clean build (migration runs automatically on test DB creation).

- [ ] **Step 5: Commit**

```bash
git add crates/mchact-storage/src/db.rs
git commit -m "feat: add migration v23 with knowledge tables and document_chunks"
```

---

### Task 2: KnowledgeManager Service — CRUD + Chunking

**Files:**
- Create: `src/knowledge.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Create KnowledgeManager with CRUD + chunking**

```rust
// src/knowledge.rs

use mchact_storage::db::{Database, Knowledge, DocumentChunk, DocumentExtraction};
use std::sync::Arc;
use serde::{Serialize, Deserialize};

pub struct KnowledgeManager {
    db: Arc<Database>,
}

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

impl KnowledgeManager {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Create a knowledge collection. Auto-adds owner chat to access.
    pub fn create(&self, name: &str, description: &str, owner_chat_id: i64) -> Result<i64, String> {
        let id = self.db.create_knowledge(name, description, owner_chat_id)
            .map_err(|e| format!("Failed to create knowledge: {e}"))?;
        self.db.add_knowledge_chat_access(id, owner_chat_id)
            .map_err(|e| format!("Failed to add owner access: {e}"))?;
        Ok(id)
    }

    /// Add a document extraction to a collection. Triggers chunking.
    pub fn add_document(&self, knowledge_name: &str, doc_extraction_id: i64) -> Result<i64, String> {
        let knowledge = self.db.get_knowledge_by_name(knowledge_name)
            .map_err(|e| format!("DB error: {e}"))?
            .ok_or_else(|| format!("Knowledge '{}' not found", knowledge_name))?;

        self.db.add_document_to_knowledge(knowledge.id, doc_extraction_id)
            .map_err(|e| format!("Failed to add document: {e}"))?;

        // Chunk the document
        let chunks_created = self.chunk_document(doc_extraction_id)?;

        self.db.update_knowledge_timestamp(knowledge.id)
            .map_err(|e| format!("Failed to update timestamp: {e}"))?;

        Ok(chunks_created)
    }

    /// Split document extracted text into per-page chunks.
    fn chunk_document(&self, doc_extraction_id: i64) -> Result<i64, String> {
        // Get the extraction to access its text
        // Look up via db method that returns extracted_text
        let extraction = self.db.get_document_extraction(doc_extraction_id)
            .map_err(|e| format!("DB error: {e}"))?
            .ok_or_else(|| format!("Document extraction {} not found", doc_extraction_id))?;

        let pages = split_into_pages(&extraction.extracted_text);
        let mut count = 0i64;
        for (page_num, page_text) in pages.iter().enumerate() {
            let token_count = estimate_tokens(page_text);
            self.db.insert_document_chunk(
                doc_extraction_id,
                (page_num + 1) as i64,
                page_text,
                Some(token_count as i64),
            ).map_err(|e| format!("Failed to insert chunk: {e}"))?;
            count += 1;
        }
        Ok(count)
    }

    /// Remove a document from a collection.
    pub fn remove_document(&self, knowledge_name: &str, doc_extraction_id: i64) -> Result<(), String> {
        let knowledge = self.db.get_knowledge_by_name(knowledge_name)
            .map_err(|e| format!("DB error: {e}"))?
            .ok_or_else(|| format!("Knowledge '{}' not found", knowledge_name))?;
        self.db.remove_document_from_knowledge(knowledge.id, doc_extraction_id)
            .map_err(|e| format!("Failed to remove: {e}"))?;
        Ok(())
    }

    /// Delete a knowledge collection.
    pub fn delete(&self, knowledge_name: &str, caller_chat_id: i64) -> Result<(), String> {
        let knowledge = self.db.get_knowledge_by_name(knowledge_name)
            .map_err(|e| format!("DB error: {e}"))?
            .ok_or_else(|| format!("Knowledge '{}' not found", knowledge_name))?;
        if knowledge.owner_chat_id != caller_chat_id {
            return Err("Only the owner chat can delete this knowledge collection".into());
        }
        self.db.delete_knowledge(knowledge.id)
            .map_err(|e| format!("Failed to delete: {e}"))?;
        Ok(())
    }

    /// List all knowledge collections with stats.
    pub fn list_all(&self) -> Result<Vec<KnowledgeStats>, String> {
        let collections = self.db.list_knowledge()
            .map_err(|e| format!("DB error: {e}"))?;
        let mut stats = Vec::new();
        for k in collections {
            stats.push(self.get_stats(&k)?);
        }
        Ok(stats)
    }

    /// Attach a knowledge collection to a chat.
    pub fn attach(&self, knowledge_name: &str, chat_id: i64) -> Result<(), String> {
        let knowledge = self.db.get_knowledge_by_name(knowledge_name)
            .map_err(|e| format!("DB error: {e}"))?
            .ok_or_else(|| format!("Knowledge '{}' not found", knowledge_name))?;
        self.db.add_knowledge_chat_access(knowledge.id, chat_id)
            .map_err(|e| format!("Failed to attach: {e}"))?;
        Ok(())
    }

    /// Check if a chat has access to a knowledge collection.
    pub fn has_access(&self, knowledge_name: &str, chat_id: i64) -> Result<bool, String> {
        let knowledge = self.db.get_knowledge_by_name(knowledge_name)
            .map_err(|e| format!("DB error: {e}"))?;
        match knowledge {
            None => Ok(false),
            Some(k) => self.db.has_knowledge_chat_access(k.id, chat_id)
                .map_err(|e| format!("DB error: {e}")),
        }
    }

    fn get_stats(&self, k: &Knowledge) -> Result<KnowledgeStats, String> {
        let doc_count = self.db.count_knowledge_documents(k.id)
            .map_err(|e| format!("DB error: {e}"))?;
        let chunks = self.db.get_chunks_for_knowledge(k.id)
            .unwrap_or_default();
        let total = chunks.len() as i64;
        let embedded = chunks.iter().filter(|c| c.embedding_status == "done").count() as i64;
        let pending = chunks.iter().filter(|c| c.embedding_status == "pending").count() as i64;
        let failed = chunks.iter().filter(|c| c.embedding_status == "failed").count() as i64;
        let obs_done = chunks.iter().filter(|c| c.observation_status == "done").count() as i64;
        let obs_pending = chunks.iter().filter(|c| c.observation_status == "pending").count() as i64;

        Ok(KnowledgeStats {
            name: k.name.clone(),
            description: k.description.clone(),
            owner_chat_id: k.owner_chat_id,
            document_count: doc_count,
            chunk_count: total,
            chunks_embedded: embedded,
            chunks_pending: pending,
            chunks_failed: failed,
            observations_done: obs_done,
            observations_pending: obs_pending,
        })
    }
}

/// Split extracted text into pages.
/// PDF/DOCX use form-feed (\x0c) as page separator.
/// Plain text/HTML fall back to paragraph-based sections (~500 tokens each).
pub fn split_into_pages(text: &str) -> Vec<String> {
    // Check for form-feed page separators (common in PDF extraction)
    let pages: Vec<&str> = text.split('\x0c').collect();
    if pages.len() > 1 {
        return pages.iter()
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect();
    }

    // No page markers — split on double newlines into ~500 token sections
    let paragraphs: Vec<&str> = text.split("\n\n").collect();
    let mut sections = Vec::new();
    let mut current = String::new();
    let mut current_tokens = 0usize;

    for para in paragraphs {
        let para_tokens = estimate_tokens(para);
        if current_tokens + para_tokens > 500 && !current.is_empty() {
            sections.push(current.trim().to_string());
            current = String::new();
            current_tokens = 0;
        }
        if !current.is_empty() {
            current.push_str("\n\n");
        }
        current.push_str(para);
        current_tokens += para_tokens;
    }
    if !current.trim().is_empty() {
        sections.push(current.trim().to_string());
    }

    if sections.is_empty() && !text.trim().is_empty() {
        sections.push(text.trim().to_string());
    }

    sections
}

/// Estimate token count (approximate: ~4 chars per token).
pub fn estimate_tokens(text: &str) -> usize {
    (text.len() + 3) / 4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_into_pages_with_formfeed() {
        let text = "Page 1 content\x0cPage 2 content\x0cPage 3 content";
        let pages = split_into_pages(text);
        assert_eq!(pages.len(), 3);
        assert_eq!(pages[0], "Page 1 content");
        assert_eq!(pages[2], "Page 3 content");
    }

    #[test]
    fn test_split_into_pages_no_formfeed() {
        let text = "Short paragraph.\n\nAnother short one.\n\nThird paragraph.";
        let pages = split_into_pages(text);
        // All short — merged into one section
        assert_eq!(pages.len(), 1);
    }

    #[test]
    fn test_split_into_pages_long_text() {
        let long_para = "word ".repeat(200); // ~200 tokens
        let text = format!("{}\n\n{}\n\n{}", long_para, long_para, long_para);
        let pages = split_into_pages(&text);
        assert!(pages.len() >= 2); // Should split since total > 500 tokens
    }

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens("hello world"), 3); // 11 chars / 4 ≈ 3
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_split_empty() {
        assert!(split_into_pages("").is_empty());
        assert!(split_into_pages("   ").is_empty());
    }
}
```

- [ ] **Step 2: Register module in `src/lib.rs`**

Add after `pub mod media_manager;`:
```rust
pub mod knowledge;
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib knowledge -- --nocapture`
Expected: All 5 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/knowledge.rs src/lib.rs
git commit -m "feat: add KnowledgeManager with CRUD, chunking, and stats"
```

---

### Task 3: Knowledge Query Flow (Vector + Observation DAG)

**Files:**
- Modify: `src/knowledge.rs`

- [ ] **Step 1: Add query method to KnowledgeManager**

Add to `KnowledgeManager`:

```rust
use crate::embedding::EmbeddingProvider;

impl KnowledgeManager {
    /// Query knowledge collections: embed query → vector search → observation DAG.
    pub async fn query(
        &self,
        knowledge_names: Option<&[String]>,
        query: &str,
        max_results: usize,
        caller_chat_id: i64,
        embedding_provider: &dyn EmbeddingProvider,
        observation_store: Option<&dyn mchact_memory::ObservationStore>,
    ) -> Result<KnowledgeQueryResult, String> {
        // 1. Determine scope
        let accessible_knowledge = match knowledge_names {
            Some(names) => {
                let mut collections = Vec::new();
                for name in names {
                    if !self.has_access(name, caller_chat_id)? {
                        return Err(format!("No access to knowledge '{name}'"));
                    }
                    let k = self.db.get_knowledge_by_name(name)
                        .map_err(|e| format!("DB: {e}"))?
                        .ok_or_else(|| format!("Knowledge '{name}' not found"))?;
                    collections.push(k);
                }
                collections
            }
            None => {
                self.db.list_knowledge_for_chat(caller_chat_id)
                    .map_err(|e| format!("DB: {e}"))?
            }
        };

        if accessible_knowledge.is_empty() {
            return Ok(KnowledgeQueryResult {
                query: query.to_string(),
                results: vec![],
                collections_searched: vec![],
                total_chunks_searched: 0,
            });
        }

        // 2. Collect all doc extraction IDs and map to knowledge names
        let mut doc_to_knowledge: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
        let mut all_doc_ids = Vec::new();
        let mut collection_names = Vec::new();
        for k in &accessible_knowledge {
            collection_names.push(k.name.clone());
            let doc_ids = self.db.list_knowledge_documents(k.id)
                .map_err(|e| format!("DB: {e}"))?;
            for did in &doc_ids {
                doc_to_knowledge.insert(*did, k.name.clone());
            }
            all_doc_ids.extend(doc_ids);
        }

        // 3. Embed query
        let query_embedding = embedding_provider.embed(query).await
            .map_err(|e| format!("Embedding failed: {e}"))?;

        // 4. Get all embedded chunks for these documents
        let mut scored_chunks: Vec<(f32, DocumentChunk, String)> = Vec::new(); // (score, chunk, knowledge_name)
        for doc_id in &all_doc_ids {
            let chunks = self.db.get_chunks_for_document(*doc_id)
                .map_err(|e| format!("DB: {e}"))?;
            let kname = doc_to_knowledge.get(doc_id).cloned().unwrap_or_default();
            for chunk in chunks {
                if chunk.embedding_status != "done" || chunk.embedding.is_none() {
                    continue;
                }
                let chunk_vec = bytes_to_f32_vec(chunk.embedding.as_ref().unwrap());
                let score = cosine_similarity(&query_embedding, &chunk_vec);
                scored_chunks.push((score, chunk, kname.clone()));
            }
        }

        let total_searched = scored_chunks.len();

        // 5. Sort by score descending, take top N
        scored_chunks.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored_chunks.truncate(max_results);

        // 6. Build results with citations + observations
        let mut results = Vec::new();
        for (score, chunk, kname) in scored_chunks {
            // Get document filename
            let filename = self.db.get_document_extraction(chunk.document_extraction_id)
                .ok()
                .flatten()
                .map(|de| de.filename.clone())
                .unwrap_or_else(|| format!("doc-{}", chunk.document_extraction_id));

            let snippet = if chunk.text.len() > 500 {
                format!("{}...", &chunk.text[..500])
            } else {
                chunk.text.clone()
            };

            // Get related observations if observation store available
            let related_observations = if let Some(_obs_store) = observation_store {
                // Query observations that reference this chunk
                // For now return empty — full DAG traversal is wired when observation engine supports chunk references
                vec![]
            } else {
                vec![]
            };

            results.push(KnowledgeResult {
                knowledge_name: kname,
                document: filename,
                page: chunk.page_number,
                relevance_score: score,
                text_snippet: snippet,
                related_observations,
            });
        }

        Ok(KnowledgeQueryResult {
            query: query.to_string(),
            results,
            collections_searched: collection_names,
            total_chunks_searched: total_searched as u64,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeQueryResult {
    pub query: String,
    pub results: Vec<KnowledgeResult>,
    pub collections_searched: Vec<String>,
    pub total_chunks_searched: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeResult {
    pub knowledge_name: String,
    pub document: String,
    pub page: i64,
    pub relevance_score: f32,
    pub text_snippet: String,
    pub related_observations: Vec<serde_json::Value>,
}

/// Convert embedding bytes (stored as BLOB) back to f32 vector.
fn bytes_to_f32_vec(bytes: &[u8]) -> Vec<f32> {
    bytes.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

/// Convert f32 vector to bytes for storage.
pub fn f32_vec_to_bytes(vec: &[f32]) -> Vec<u8> {
    vec.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}
```

- [ ] **Step 2: Add query tests**

```rust
#[test]
fn test_cosine_similarity_identical() {
    let v = vec![1.0, 2.0, 3.0];
    let sim = cosine_similarity(&v, &v);
    assert!((sim - 1.0).abs() < 0.001);
}

#[test]
fn test_cosine_similarity_orthogonal() {
    let a = vec![1.0, 0.0];
    let b = vec![0.0, 1.0];
    let sim = cosine_similarity(&a, &b);
    assert!(sim.abs() < 0.001);
}

#[test]
fn test_f32_vec_roundtrip() {
    let original = vec![1.5, -2.3, 0.0, 42.0];
    let bytes = f32_vec_to_bytes(&original);
    let recovered = bytes_to_f32_vec(&bytes);
    assert_eq!(original.len(), recovered.len());
    for (a, b) in original.iter().zip(recovered.iter()) {
        assert!((a - b).abs() < 0.0001);
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib knowledge -- --nocapture`
Expected: All 8 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/knowledge.rs
git commit -m "feat: add knowledge query with vector search and cosine similarity"
```

---

### Task 4: Scheduled Processing Jobs

**Files:**
- Create: `src/knowledge_scheduler.rs`
- Modify: `src/lib.rs`
- Modify: `src/scheduler.rs`

- [ ] **Step 1: Create knowledge_scheduler.rs**

```rust
// src/knowledge_scheduler.rs

use crate::config::Config;
use crate::embedding::EmbeddingProvider;
use crate::knowledge::f32_vec_to_bytes;
use mchact_storage::db::Database;
use std::sync::Arc;
use tracing::{info, warn};

/// Job 1: Embed pending chunks.
pub async fn run_embed_job(
    db: &Database,
    embedding: &dyn EmbeddingProvider,
    batch_size: i64,
) -> (i64, i64) {
    let chunks = match db.get_chunks_by_status("pending", batch_size) {
        Ok(c) => c,
        Err(e) => {
            warn!("Knowledge embed job: DB error: {e}");
            return (0, 0);
        }
    };

    let mut done = 0i64;
    let mut failed = 0i64;

    for chunk in &chunks {
        // Truncate to max embedding tokens if needed
        let text = if chunk.text.len() > 32000 {
            &chunk.text[..32000]
        } else {
            &chunk.text
        };

        match embedding.embed(text).await {
            Ok(vector) => {
                let bytes = f32_vec_to_bytes(&vector);
                if let Err(e) = db.update_chunk_embedding(chunk.id, &bytes, "done") {
                    warn!(chunk_id = chunk.id, "Failed to save embedding: {e}");
                    failed += 1;
                } else {
                    done += 1;
                }
            }
            Err(e) => {
                warn!(chunk_id = chunk.id, "Embedding failed: {e}");
                let _ = db.update_chunk_embedding(chunk.id, &[], "failed");
                failed += 1;
            }
        }
    }

    if done > 0 || failed > 0 {
        info!(done, failed, "Knowledge embed job completed");
    }
    (done, failed)
}

/// Job 2: Feed embedded chunks to observation engine.
pub async fn run_observe_job(
    db: &Database,
    observation_store: Option<&dyn mchact_memory::ObservationStore>,
    _config: &Config,
    batch_size: i64,
) -> (i64, i64) {
    let obs_store = match observation_store {
        Some(s) => s,
        None => return (0, 0), // No observation store configured
    };

    let chunks = match db.get_chunks_for_observation(batch_size) {
        Ok(c) => c,
        Err(e) => {
            warn!("Knowledge observe job: DB error: {e}");
            return (0, 0);
        }
    };

    let mut done = 0i64;
    let mut failed = 0i64;

    for chunk in &chunks {
        // Get document filename for context
        let filename = db.get_document_extraction(chunk.document_extraction_id)
            .ok()
            .flatten()
            .map(|de| de.filename.clone())
            .unwrap_or_else(|| "unknown".into());

        let context = format!(
            "Document: {}, Page {}\n---\n{}",
            filename, chunk.page_number, chunk.text
        );

        // Feed to observation engine as source material
        // The deriver will extract explicit observations from this
        match obs_store.store_observation(
            &mchact_memory::types::Observation {
                level: mchact_memory::types::ObservationLevel::Explicit,
                content: context,
                confidence: 0.9,
                source_ids: vec![],
                ..Default::default()
            }
        ).await {
            Ok(_) => {
                let _ = db.update_chunk_observation_status(chunk.id, "done");
                done += 1;
            }
            Err(e) => {
                warn!(chunk_id = chunk.id, "Observation storage failed: {e}");
                let _ = db.update_chunk_observation_status(chunk.id, "failed");
                failed += 1;
            }
        }
    }

    if done > 0 || failed > 0 {
        info!(done, failed, "Knowledge observe job completed");
    }
    (done, failed)
}

/// Job 3: Auto-group knowledge collections.
pub async fn run_autogroup_job(
    db: &Database,
    config: &Config,
    llm: &dyn crate::llm::LlmProvider,
    min_docs: i64,
) -> i64 {
    let collections = match db.get_knowledge_needing_grouping(min_docs) {
        Ok(c) => c,
        Err(e) => {
            warn!("Knowledge autogroup job: DB error: {e}");
            return 0;
        }
    };

    let mut processed = 0i64;

    for knowledge in &collections {
        let doc_ids = match db.list_knowledge_documents(knowledge.id) {
            Ok(ids) => ids,
            Err(_) => continue,
        };

        // Collect document summaries (first 500 chars of each)
        let mut doc_summaries = Vec::new();
        for did in &doc_ids {
            if let Ok(Some(de)) = db.get_document_extraction(*did) {
                let preview = if de.extracted_text.len() > 500 {
                    format!("{}...", &de.extracted_text[..500])
                } else {
                    de.extracted_text.clone()
                };
                doc_summaries.push(format!("- {} ({}): {}", de.filename, did, preview));
            }
        }

        if doc_summaries.is_empty() {
            continue;
        }

        let prompt = format!(
            "Given these {} documents in knowledge collection '{}':\n{}\n\n\
             Are they well-grouped? Suggest if any should be split into separate \
             collections or if the description should be updated. Be concise.",
            doc_summaries.len(),
            knowledge.name,
            doc_summaries.join("\n")
        );

        // Call LLM for grouping suggestion
        // For now just log — actual LLM call integration depends on LlmProvider interface
        info!(
            knowledge = %knowledge.name,
            docs = doc_summaries.len(),
            "Auto-group: would analyze {} documents",
            doc_summaries.len()
        );

        let _ = db.update_knowledge_grouping_check(knowledge.id, doc_ids.len() as i64);
        processed += 1;
    }

    if processed > 0 {
        info!(processed, "Knowledge autogroup job completed");
    }
    processed
}

/// Reset failed chunks for retry.
pub fn reset_failed_chunks(db: &Database, older_than_mins: i64) -> i64 {
    db.reset_failed_chunks(older_than_mins).unwrap_or(0)
}
```

- [ ] **Step 2: Register module and wire into scheduler**

Add to `src/lib.rs`:
```rust
pub mod knowledge_scheduler;
```

In `src/scheduler.rs`, find where `spawn_reflector` is defined and add a similar function that spawns the knowledge processing loop:

```rust
pub fn spawn_knowledge_processor(state: Arc<AppState>) {
    let embed_interval = state.config.knowledge_embed_interval_mins;
    let observe_interval = state.config.knowledge_observe_interval_mins;
    let autogroup_interval = state.config.knowledge_autogroup_interval_mins;

    // Embed job
    if embed_interval > 0 {
        let s = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                std::time::Duration::from_secs(embed_interval as u64 * 60)
            );
            loop {
                interval.tick().await;
                if let Some(ref embedding) = s.embedding {
                    crate::knowledge_scheduler::run_embed_job(
                        &s.db,
                        embedding.as_ref(),
                        s.config.knowledge_embed_batch_size as i64,
                    ).await;
                }
            }
        });
    }

    // Observe job
    if observe_interval > 0 {
        let s = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                std::time::Duration::from_secs(observe_interval as u64 * 60)
            );
            loop {
                interval.tick().await;
                crate::knowledge_scheduler::run_observe_job(
                    &s.db,
                    s.observation_store.as_deref(),
                    &s.config,
                    s.config.knowledge_observe_batch_size as i64,
                ).await;
            }
        });
    }

    // Auto-group job
    if autogroup_interval > 0 {
        let s = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                std::time::Duration::from_secs(autogroup_interval as u64 * 60)
            );
            loop {
                interval.tick().await;
                crate::knowledge_scheduler::run_autogroup_job(
                    &s.db,
                    &s.config,
                    s.llm.as_ref(),
                    s.config.knowledge_autogroup_min_docs as i64,
                ).await;
            }
        });
    }

    // Failed chunk retry (every 30 mins)
    let s = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30 * 60));
        loop {
            interval.tick().await;
            let reset = crate::knowledge_scheduler::reset_failed_chunks(
                &s.db,
                s.config.knowledge_retry_delay_mins as i64,
            );
            if reset > 0 {
                tracing::info!(reset, "Reset failed knowledge chunks for retry");
            }
        }
    });
}
```

- [ ] **Step 3: Verify build**

Run: `cargo build`
Expected: Clean build.

- [ ] **Step 4: Commit**

```bash
git add src/knowledge_scheduler.rs src/lib.rs src/scheduler.rs
git commit -m "feat: add knowledge scheduled processing (embed, observe, auto-group)"
```

---

### Task 5: Knowledge Config Fields

**Files:**
- Modify: `src/config.rs`
- Modify: `tests/config_validation.rs`

- [ ] **Step 1: Add 8 config fields**

```rust
#[serde(default = "default_knowledge_embed_interval_mins")]
pub knowledge_embed_interval_mins: u64,
#[serde(default = "default_knowledge_embed_batch_size")]
pub knowledge_embed_batch_size: u64,
#[serde(default = "default_knowledge_observe_interval_mins")]
pub knowledge_observe_interval_mins: u64,
#[serde(default = "default_knowledge_observe_batch_size")]
pub knowledge_observe_batch_size: u64,
#[serde(default = "default_knowledge_autogroup_interval_mins")]
pub knowledge_autogroup_interval_mins: u64,
#[serde(default = "default_knowledge_autogroup_min_docs")]
pub knowledge_autogroup_min_docs: u64,
#[serde(default = "default_knowledge_retry_delay_mins")]
pub knowledge_retry_delay_mins: u64,
#[serde(default = "default_knowledge_max_embedding_tokens")]
pub knowledge_max_embedding_tokens: u64,
```

Default functions:
```rust
fn default_knowledge_embed_interval_mins() -> u64 { 5 }
fn default_knowledge_embed_batch_size() -> u64 { 50 }
fn default_knowledge_observe_interval_mins() -> u64 { 15 }
fn default_knowledge_observe_batch_size() -> u64 { 20 }
fn default_knowledge_autogroup_interval_mins() -> u64 { 60 }
fn default_knowledge_autogroup_min_docs() -> u64 { 5 }
fn default_knowledge_retry_delay_mins() -> u64 { 30 }
fn default_knowledge_max_embedding_tokens() -> u64 { 8192 }
```

Add to `test_defaults()` and `tests/config_validation.rs::minimal_config()`.

- [ ] **Step 2: Verify build**

Run: `cargo build`

- [ ] **Step 3: Commit**

```bash
git add src/config.rs tests/config_validation.rs
git commit -m "feat: add knowledge builder config fields"
```

---

### Task 6: 7 Knowledge Agent Tools

**Files:**
- Create: `src/tools/knowledge.rs`
- Modify: `src/tools/mod.rs`

- [ ] **Step 1: Create 7 knowledge tools**

Create `src/tools/knowledge.rs` with these tools:

1. `CreateKnowledgeTool` — calls `knowledge_manager.create(name, desc, chat_id)`
2. `AddDocumentToKnowledgeTool` — calls `knowledge_manager.add_document(name, doc_id)`
3. `RemoveDocumentFromKnowledgeTool` — calls `knowledge_manager.remove_document(name, doc_id)`
4. `ListKnowledgeTool` — calls `knowledge_manager.list_all()`
5. `QueryKnowledgeTool` — calls `knowledge_manager.query(names, query, max, chat_id, embedding, obs_store)`
6. `AttachKnowledgeTool` — calls `knowledge_manager.attach(name, chat_id)`
7. `DeleteKnowledgeTool` — calls `knowledge_manager.delete(name, chat_id)`

Each tool follows the existing pattern:
```rust
use super::{auth_context_from_input, schema_object, Tool, ToolResult};
use crate::knowledge::KnowledgeManager;

pub struct CreateKnowledgeTool {
    knowledge_manager: Arc<KnowledgeManager>,
}

#[async_trait]
impl Tool for CreateKnowledgeTool {
    fn name(&self) -> &str { "create_knowledge" }
    fn definition(&self) -> ToolDefinition { ... }
    async fn execute(&self, input: Value) -> ToolResult { ... }
}
```

Tools needing embedding/observations (`QueryKnowledgeTool`) store `Arc<Option<Arc<dyn EmbeddingProvider>>>` and `Arc<Option<Arc<dyn ObservationStore>>>`.

- [ ] **Step 2: Register in mod.rs**

Add `pub mod knowledge;` and register all 7 tools in `ToolRegistry::new()`:

```rust
let knowledge_manager = Arc::new(crate::knowledge::KnowledgeManager::new(db.clone()));
tools.push(Box::new(knowledge::CreateKnowledgeTool::new(knowledge_manager.clone())));
tools.push(Box::new(knowledge::AddDocumentToKnowledgeTool::new(knowledge_manager.clone())));
tools.push(Box::new(knowledge::RemoveDocumentFromKnowledgeTool::new(knowledge_manager.clone())));
tools.push(Box::new(knowledge::ListKnowledgeTool::new(knowledge_manager.clone())));
tools.push(Box::new(knowledge::QueryKnowledgeTool::new(knowledge_manager.clone(), embedding_opt.clone(), obs_store_opt.clone())));
tools.push(Box::new(knowledge::AttachKnowledgeTool::new(knowledge_manager.clone())));
tools.push(Box::new(knowledge::DeleteKnowledgeTool::new(knowledge_manager)));
```

Where `embedding_opt` and `obs_store_opt` are cloned from the AppState fields that are already available during tool registry construction.

- [ ] **Step 3: Verify build**

Run: `cargo build`

- [ ] **Step 4: Commit**

```bash
git add src/tools/knowledge.rs src/tools/mod.rs
git commit -m "feat: add 7 knowledge agent tools"
```

---

### Task 7: Knowledge CLI Subcommand

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add Knowledge subcommand**

Add `KnowledgeAction` enum:

```rust
#[derive(Debug, Subcommand)]
enum KnowledgeAction {
    /// List all knowledge collections
    List,
    /// Show collection details
    Show { name: String },
    /// Create a collection
    Create {
        name: String,
        #[arg(long, default_value = "")]
        description: String,
    },
    /// Add document to collection
    Add { name: String, document_id: i64 },
    /// Remove document from collection
    Remove { name: String, document_id: i64 },
    /// Delete collection
    Delete { name: String },
    /// Show processing status
    Status { name: String },
    /// Search with citations
    Query { name: String, query: String },
    /// Share to a chat
    Share {
        name: String,
        #[arg(long)]
        chat_id: i64,
    },
}
```

Add to `MainCommand`:
```rust
/// Knowledge collection management
Knowledge {
    #[command(subcommand)]
    action: KnowledgeAction,
},
```

Add match arm that handles each action using `KnowledgeManager`.

- [ ] **Step 2: Verify build + CLI help**

Run: `cargo build && cargo run -- knowledge --help`

- [ ] **Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: add mchact knowledge CLI with list/show/create/add/query/status subcommands"
```

---

### Task 8: Web API Endpoints

**Files:**
- Modify: `src/web.rs`

- [ ] **Step 1: Add knowledge API routes**

Add to the axum router (where other /api routes are defined):

```rust
.route("/api/knowledge", get(api_knowledge_list).post(api_knowledge_create))
.route("/api/knowledge/:name", get(api_knowledge_show).delete(api_knowledge_delete))
.route("/api/knowledge/:name/documents", post(api_knowledge_add_document))
.route("/api/knowledge/:name/documents/:doc_id", delete(api_knowledge_remove_document))
.route("/api/knowledge/:name/status", get(api_knowledge_status))
.route("/api/knowledge/:name/query", post(api_knowledge_query))
.route("/api/knowledge/:name/attach", post(api_knowledge_attach))
```

Each handler creates a `KnowledgeManager` from AppState, calls the appropriate method, returns JSON.

- [ ] **Step 2: Verify build**

Run: `cargo build`

- [ ] **Step 3: Commit**

```bash
git add src/web.rs
git commit -m "feat: add knowledge web API endpoints"
```

---

### Task 9: Runtime Wiring + Final Verification

**Files:**
- Modify: `src/runtime.rs`

- [ ] **Step 1: Wire knowledge scheduler into runtime**

In the main `run()` function in runtime.rs, after the existing `spawn_reflector` call, add:

```rust
crate::scheduler::spawn_knowledge_processor(state.clone());
```

- [ ] **Step 2: Run all tests**

Run: `cargo test --lib`
Expected: All tests pass.

- [ ] **Step 3: Verify CLI**

Run: `cargo run -- knowledge --help`
Expected: Shows all subcommands.

- [ ] **Step 4: Commit**

```bash
git add src/runtime.rs
git commit -m "feat: wire knowledge scheduler into runtime"
```
