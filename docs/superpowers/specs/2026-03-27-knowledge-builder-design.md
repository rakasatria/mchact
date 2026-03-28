# Knowledge Builder — Design Spec

**Status:** Draft
**Date:** 2026-03-27

---

## Overview

Add a knowledge management system to mchact: named collections of documents with per-page chunking, vector embeddings, observation engine integration, LLM-based auto-grouping, and scheduled processing. Knowledge is searchable via vector similarity + observation DAG traversal, returning results with page-level citations.

**Builds on:** `media_objects` table (Plan D), `document_extractions` table, `mchact-memory` observation engine, existing embedding provider, existing scheduler.

---

## 1. Data Model

### New Tables (Migration v23)

**`knowledge`** — Named collections of documents.

```sql
CREATE TABLE knowledge (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    description TEXT DEFAULT '',
    owner_chat_id INTEGER NOT NULL,
    last_grouping_check_at TEXT,
    document_count_at_last_check INTEGER DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX idx_knowledge_owner ON knowledge(owner_chat_id);
```

**`knowledge_documents`** — Junction: which extractions belong to which collection.

```sql
CREATE TABLE knowledge_documents (
    knowledge_id INTEGER NOT NULL REFERENCES knowledge(id) ON DELETE CASCADE,
    document_extraction_id INTEGER NOT NULL REFERENCES document_extractions(id),
    added_at TEXT NOT NULL,
    PRIMARY KEY (knowledge_id, document_extraction_id)
);
```

**`knowledge_chat_access`** — Which chats can query a collection.

```sql
CREATE TABLE knowledge_chat_access (
    knowledge_id INTEGER NOT NULL REFERENCES knowledge(id) ON DELETE CASCADE,
    chat_id INTEGER NOT NULL,
    attached_at TEXT NOT NULL,
    PRIMARY KEY (knowledge_id, chat_id)
);
CREATE INDEX idx_knowledge_access_chat ON knowledge_chat_access(chat_id);
```

**`document_chunks`** — Per-page text + embeddings.

```sql
CREATE TABLE document_chunks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    document_extraction_id INTEGER NOT NULL REFERENCES document_extractions(id) ON DELETE CASCADE,
    page_number INTEGER NOT NULL,
    text TEXT NOT NULL,
    token_count INTEGER,
    embedding BLOB,
    embedding_status TEXT NOT NULL DEFAULT 'pending',
    observation_status TEXT NOT NULL DEFAULT 'pending',
    created_at TEXT NOT NULL
);
CREATE INDEX idx_chunks_extraction ON document_chunks(document_extraction_id);
CREATE INDEX idx_chunks_embedding_status ON document_chunks(embedding_status);
CREATE INDEX idx_chunks_observation_status ON document_chunks(observation_status);
```

### Processing Status Values

| Column | Values | Meaning |
|--------|--------|---------|
| `embedding_status` | `pending` | Not yet embedded |
| | `done` | Embedding generated |
| | `failed` | Embedding failed (retry next cycle) |
| `observation_status` | `pending` | Not yet fed to observation engine |
| | `done` | Observations extracted |
| | `failed` | Extraction failed (retry next cycle) |

---

## 2. Chunking Strategy

**Per-page chunking.** Each page of a document becomes one chunk.

### Page Detection

When a document is added to a knowledge collection:
1. Get extracted text from `document_extractions`
2. Split into pages:
   - **PDF/DOCX/PPTX:** kreuzberg extracts per-page — split on page separator (`\f` form feed, or kreuzberg's page markers)
   - **Plain text/HTML/code:** No natural pages. Split at paragraph boundaries (`\n\n`) into logical sections of ~500 tokens each. Label as "section 1", "section 2", etc.
3. Insert one `document_chunks` row per page with `embedding_status = 'pending'`, `observation_status = 'pending'`

### For Large Pages

If a single page exceeds 8192 tokens (embedding model limit):
- Truncate to 8192 tokens for embedding (the vector represents the first part)
- The full text is still stored in `document_chunks.text` and sent to the observation engine

### Token Counting

Use a simple heuristic: `token_count ≈ text.len() / 4` (approximate). Exact counting not needed — it's for progress reporting and budget estimation.

---

## 3. Agent Tools (7 tools)

### `create_knowledge`

```json
{
  "name": "create_knowledge",
  "description": "Create a named knowledge collection for grouping and searching documents.",
  "parameters": {
    "type": "object",
    "required": ["name"],
    "properties": {
      "name": {"type": "string", "description": "Unique name for the collection (e.g. 'project-docs')"},
      "description": {"type": "string", "description": "What this collection contains"}
    }
  }
}
```

- Creates `knowledge` row
- Auto-adds owner chat to `knowledge_chat_access`
- Returns: `{"knowledge_id": N, "name": "...", "message": "..."}`

### `add_document_to_knowledge`

```json
{
  "name": "add_document_to_knowledge",
  "parameters": {
    "type": "object",
    "required": ["knowledge_name"],
    "properties": {
      "knowledge_name": {"type": "string"},
      "document_id": {"type": "integer", "description": "document_extractions.id"},
      "media_object_id": {"type": "integer", "description": "Alternative: look up extraction by media_object_id"}
    }
  }
}
```

- Inserts `knowledge_documents` junction row
- Triggers chunking: splits extracted text into pages, inserts `document_chunks` rows
- Updates `knowledge.updated_at`
- Returns: `{"added": true, "chunks_created": N, "knowledge_name": "..."}`

### `remove_document_from_knowledge`

- Removes `knowledge_documents` junction row
- Does NOT delete chunks or observations (they may be referenced elsewhere)
- Returns confirmation

### `list_knowledge`

- No required params
- Returns all knowledge collections with: name, description, document count, chunk count, processing status summary (pending/done/failed counts)
- Any chat can call this (browse)

### `query_knowledge`

```json
{
  "name": "query_knowledge",
  "parameters": {
    "type": "object",
    "required": ["query"],
    "properties": {
      "knowledge_names": {
        "type": "array",
        "items": {"type": "string"},
        "description": "Collections to search (omit to search ALL attached collections)"
      },
      "query": {"type": "string", "description": "What to search for"},
      "max_results": {"type": "integer", "description": "Max results (default: 5)"}
    }
  }
}
```

- If `knowledge_names` provided: search only those collections (each must be in `knowledge_chat_access`)
- If omitted: search ALL collections the caller chat has access to
- Results include `knowledge_name` per result for attribution
- Query flow (see Section 5)

### `attach_knowledge`

```json
{
  "name": "attach_knowledge",
  "parameters": {
    "type": "object",
    "required": ["knowledge_name"],
    "properties": {
      "knowledge_name": {"type": "string"}
    }
  }
}
```

- Adds caller chat to `knowledge_chat_access`
- Self-service: any chat can attach
- Returns confirmation

### `delete_knowledge`

- Only owner chat can delete
- Removes `knowledge` row (CASCADE deletes junction rows and chat_access)
- Does NOT delete document_extractions, media_objects, or chunks
- Returns confirmation

---

## 4. Scheduled Processing Pipeline

Three jobs registered with the existing mchact scheduler at startup.

### Job 1: Chunk + Embed (default: every 5 minutes)

```
1. SELECT * FROM document_chunks WHERE embedding_status = 'pending' LIMIT 50
2. For each chunk:
   a. If text too long (> 8192 tokens): truncate for embedding
   b. Call embedding provider: embed(chunk.text) → vector
   c. UPDATE document_chunks SET embedding = vector, embedding_status = 'done'
   d. On error: SET embedding_status = 'failed'
3. Log: "Embedded {N} chunks, {M} failed"
```

Config: `knowledge.embed_interval_mins: 5`, `knowledge.embed_batch_size: 50`

### Job 2: Observe (default: every 15 minutes)

```
1. SELECT * FROM document_chunks
   WHERE embedding_status = 'done' AND observation_status = 'pending' LIMIT 20
2. For each chunk:
   a. Build prompt: "Extract key facts, entities, and relationships from this text:
      Document: {filename}, Page {page_number}
      ---
      {chunk.text}"
   b. Call observation engine deriver: extract explicit observations
   c. Each observation stored with source reference to chunk
   d. UPDATE document_chunks SET observation_status = 'done'
   e. On error: SET observation_status = 'failed'
3. Dreamer runs on its normal schedule — discovers deductive/inductive observations
```

Config: `knowledge.observe_interval_mins: 15`, `knowledge.observe_batch_size: 20`

### Job 3: Auto-Group (default: every 60 minutes)

```
1. SELECT * FROM knowledge
   WHERE document_count > document_count_at_last_check
   AND (SELECT COUNT(*) FROM knowledge_documents WHERE knowledge_id = knowledge.id) > 5
2. For each knowledge collection needing re-evaluation:
   a. Collect: document names + first-page text (truncated to 500 chars each)
   b. Call LLM: "Given these documents in collection '{name}':
      {doc list}
      Are they well-grouped? Suggest if any should be split into separate collections
      or if the description should be updated."
   c. Store suggestion in knowledge.description (append as "[Auto-suggestion: ...]")
   d. UPDATE knowledge SET last_grouping_check_at = now, document_count_at_last_check = current_count
3. Agent sees suggestions on next list_knowledge call
```

Config: `knowledge.autogroup_interval_mins: 60`, `knowledge.autogroup_min_docs: 5`

### Change Detection

| Job | Processes when | Skips when |
|-----|---------------|-----------|
| Embed | `embedding_status = 'pending'` | No pending chunks |
| Observe | `embedding_status = 'done' AND observation_status = 'pending'` | No newly-embedded chunks |
| Auto-group | `document_count > document_count_at_last_check` | No new docs since last check |

### Retry

Failed chunks (`embedding_status = 'failed'` or `observation_status = 'failed'`) are retried by resetting to `'pending'` after a configurable delay (default: 30 minutes). A background sweep resets failed chunks:

```sql
UPDATE document_chunks SET embedding_status = 'pending'
WHERE embedding_status = 'failed'
AND created_at < datetime('now', '-30 minutes')
```

---

## 5. Query Flow

**`query_knowledge(query, knowledge_names=None, max_results=5)`:**

```
1. Determine scope:
   a. If knowledge_names provided (array):
      Verify access: chat_id in knowledge_chat_access for each named collection
      Get document_extraction_ids for those collections
   b. If knowledge_names omitted:
      Get ALL knowledge_ids the caller chat has access to (knowledge_chat_access)
      Get ALL document_extraction_ids across all accessible collections
   c. In both cases, collect knowledge_name per document_extraction_id for attribution

2. Embed query: query_vector = embed(query_text)

3. Vector search on chunks:
   SELECT dc.*, de.filename, kd.knowledge_id
   FROM document_chunks dc
   JOIN document_extractions de ON dc.document_extraction_id = de.id
   JOIN knowledge_documents kd ON kd.document_extraction_id = de.id
   JOIN knowledge k ON k.id = kd.knowledge_id
   WHERE dc.document_extraction_id IN (accessible doc IDs)
   AND dc.embedding_status = 'done'
   ORDER BY vector_distance(dc.embedding, query_vector)
   LIMIT max_results

   (Uses sqlite-vec for vector search, or in-memory brute-force if sqlite-vec not available)

4. For each matching chunk:
   a. Knowledge attribution: which collection this result came from
   b. Citation: "{filename}, page {page_number}"
   c. Snippet: first 500 chars of chunk text
   d. Observations: query observation engine WHERE source references this chunk
   e. DAG traversal: follow deductive/inductive observations from those explicit ones

5. Return:
   {
     "query": "revenue growth",
     "results": [
       {
         "knowledge_name": "project-docs",
         "document": "quarterly-report.pdf",
         "page": 7,
         "relevance_score": 0.92,
         "text_snippet": "Revenue grew 23%...",
         "related_observations": [
           {"level": "explicit", "content": "Q3 revenue was $4.2M"},
           {"level": "deductive", "content": "Growth exceeds industry average"}
         ]
       },
       {
         "knowledge_name": "financial-reports",
         "document": "annual-summary.pdf",
         "page": 12,
         "relevance_score": 0.87,
         "text_snippet": "Full year revenue reached...",
         "related_observations": [...]
       }
     ],
     "collections_searched": ["project-docs", "financial-reports"],
     "total_chunks_searched": 120
   }
```

### Fallback Without sqlite-vec

If the `sqlite-vec` feature is not enabled:
- Load all chunk embeddings for the collection into memory
- Compute cosine similarity in Rust
- Sort and take top N
- Works for small-medium collections (<10K chunks)

---

## 6. CLI Interface

```
mchact knowledge list                                  List all collections with stats
mchact knowledge show <name>                           Collection detail + documents + status
mchact knowledge create <name> [--description "..."]   Create collection
mchact knowledge add <name> <document_id>              Add document to collection
mchact knowledge remove <name> <document_id>           Remove document
mchact knowledge delete <name>                         Delete collection
mchact knowledge status <name>                         Processing pipeline status
mchact knowledge query <name> "search text"            Search with citations
mchact knowledge share <name> --chat-id <id>           Share to a chat
```

### Status Output Example

```
Knowledge: project-docs
Description: Q3 project documentation
Owner: chat 42
Documents: 8

Processing:
  Chunks:       45 total
  Embeddings:   40 done, 3 pending, 2 failed
  Observations: 35 done, 5 pending, 0 failed

Last auto-group: 2h ago (no suggestions)
Shared with: chat 42 (owner), chat 87, chat 103
```

---

## 7. Web API

```
GET    /api/knowledge                       List collections (with stats)
POST   /api/knowledge                       Create collection
GET    /api/knowledge/:name                 Collection detail + documents
DELETE /api/knowledge/:name                 Delete collection
POST   /api/knowledge/:name/documents       Add document
DELETE /api/knowledge/:name/documents/:id   Remove document
GET    /api/knowledge/:name/status          Processing status
POST   /api/knowledge/:name/query           Search with citations
POST   /api/knowledge/:name/attach          Attach to session's chat
```

---

## 8. Configuration

```yaml
knowledge:
  embed_interval_mins: 5
  embed_batch_size: 50
  observe_interval_mins: 15
  observe_batch_size: 20
  autogroup_interval_mins: 60
  autogroup_min_docs: 5
  retry_delay_mins: 30
  max_embedding_tokens: 8192
```

Config fields with serde defaults in `src/config.rs`.

---

## 9. New Files

| File | Purpose | Lines (est) |
|------|---------|-------------|
| `src/knowledge.rs` | Knowledge CRUD, chunking, query flow | ~500 |
| `src/knowledge_scheduler.rs` | Three scheduled jobs (embed, observe, auto-group) | ~300 |
| `src/tools/knowledge.rs` | 7 agent tools | ~500 |

## 10. Modified Files

| File | Changes |
|------|---------|
| `crates/mchact-storage/src/db.rs` | Migration v23, 4 new tables, CRUD methods |
| `src/config.rs` | 8 knowledge config fields |
| `src/main.rs` | Add `Knowledge` CLI subcommand |
| `src/tools/mod.rs` | Register 7 knowledge tools |
| `src/scheduler.rs` | Register 3 knowledge processing jobs |
| `src/lib.rs` | Add `knowledge`, `knowledge_scheduler` modules |
| `src/runtime.rs` | Initialize knowledge scheduler jobs |
| `tests/config_validation.rs` | Add config fields to test constructors |

## 11. Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| Large documents create many chunks | Batch processing with configurable limits (50 embeds, 20 observations per cycle) |
| Embedding API costs | Only embed when `embedding_status = 'pending'` — no re-processing |
| Auto-group LLM costs | Only runs when new docs added, min 5 docs threshold, configurable interval |
| sqlite-vec not available | Fallback to in-memory cosine similarity (works for <10K chunks) |
| Failed embeddings accumulate | Retry sweep resets `failed` → `pending` after delay |
| Observation engine overloaded | Separate batch size for observe job (20 per cycle vs 50 for embed) |
| Knowledge deleted while processing | CASCADE delete on junction tables, scheduler skips orphaned chunks |
