use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;

use super::{auth_context_from_input, schema_object, Tool, ToolResult};
use mchact_core::llm_types::ToolDefinition;
use mchact_storage::db::call_blocking;
use mchact_storage::DynDataStore;
use mchact_storage::prelude::*;

// ── CreateKnowledgeTool ────────────────────────────────────────────────────────

pub struct CreateKnowledgeTool {
    knowledge_manager: Arc<crate::knowledge::KnowledgeManager>,
}

impl CreateKnowledgeTool {
    pub fn new(knowledge_manager: Arc<crate::knowledge::KnowledgeManager>) -> Self {
        Self { knowledge_manager }
    }
}

#[async_trait]
impl Tool for CreateKnowledgeTool {
    fn name(&self) -> &str {
        "create_knowledge"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "create_knowledge".into(),
            description: "Create a new knowledge collection. Knowledge collections let you organize documents and query them semantically using vector search.".into(),
            input_schema: schema_object(
                json!({
                    "name": {
                        "type": "string",
                        "description": "Unique name for the knowledge collection"
                    },
                    "description": {
                        "type": "string",
                        "description": "Optional description of the collection's purpose"
                    }
                }),
                &["name"],
            ),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let name = match input.get("name").and_then(|v| v.as_str()) {
            Some(n) if !n.trim().is_empty() => n.trim().to_string(),
            _ => return ToolResult::error("Missing or empty required parameter: name".into()),
        };

        let description = input
            .get("description")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .unwrap_or("")
            .to_string();

        let chat_id = match auth_context_from_input(&input).map(|a| a.caller_chat_id) {
            Some(id) => id,
            None => return ToolResult::error("No chat context available".into()),
        };

        let km = self.knowledge_manager.clone();
        match tokio::task::spawn_blocking(move || km.create(&name, &description, chat_id)).await {
            Ok(Ok(id)) => ToolResult::success(format!(
                "Knowledge collection created with id {id}."
            )),
            Ok(Err(e)) => ToolResult::error(e),
            Err(e) => ToolResult::error(format!("Task panicked: {e}")),
        }
    }
}

// ── AddDocumentToKnowledgeTool ─────────────────────────────────────────────────

pub struct AddDocumentToKnowledgeTool {
    knowledge_manager: Arc<crate::knowledge::KnowledgeManager>,
    db: Arc<DynDataStore>,
}

impl AddDocumentToKnowledgeTool {
    pub fn new(knowledge_manager: Arc<crate::knowledge::KnowledgeManager>, db: Arc<DynDataStore>) -> Self {
        Self { knowledge_manager, db }
    }
}

#[async_trait]
impl Tool for AddDocumentToKnowledgeTool {
    fn name(&self) -> &str {
        "add_document_to_knowledge"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "add_document_to_knowledge".into(),
            description: "Add a document to a knowledge collection for semantic search. Provide either document_id (extraction id) or media_object_id.".into(),
            input_schema: schema_object(
                json!({
                    "knowledge_name": {
                        "type": "string",
                        "description": "Name of the knowledge collection to add the document to"
                    },
                    "document_id": {
                        "type": "integer",
                        "description": "Document extraction id to add"
                    },
                    "media_object_id": {
                        "type": "integer",
                        "description": "Media object id — the corresponding document extraction will be looked up automatically"
                    }
                }),
                &["knowledge_name"],
            ),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let knowledge_name = match input.get("knowledge_name").and_then(|v| v.as_str()) {
            Some(n) if !n.trim().is_empty() => n.trim().to_string(),
            _ => {
                return ToolResult::error(
                    "Missing or empty required parameter: knowledge_name".into(),
                )
            }
        };

        // Resolve doc_extraction_id from either document_id or media_object_id.
        let doc_id: i64 = if let Some(id) = input.get("document_id").and_then(|v| v.as_i64()) {
            id
        } else if let Some(media_id) = input.get("media_object_id").and_then(|v| v.as_i64()) {
            let db = self.db.clone();
            match call_blocking(db, move |db| {
                db.get_document_extraction_id_by_media_object_id(media_id)
            })
            .await
            {
                Ok(Some(id)) => id,
                Ok(None) => {
                    return ToolResult::error(format!(
                        "No document extraction found for media_object_id {media_id}"
                    ))
                }
                Err(e) => return ToolResult::error(format!("DB error: {e}")),
            }
        } else {
            return ToolResult::error(
                "Provide either document_id or media_object_id".into(),
            );
        };

        let km = self.knowledge_manager.clone();
        match tokio::task::spawn_blocking(move || km.add_document(&knowledge_name, doc_id)).await {
            Ok(Ok(chunks)) => ToolResult::success(format!(
                "Document added. {chunks} chunk(s) created."
            )),
            Ok(Err(e)) => ToolResult::error(e),
            Err(e) => ToolResult::error(format!("Task panicked: {e}")),
        }
    }
}

// ── RemoveDocumentFromKnowledgeTool ───────────────────────────────────────────

pub struct RemoveDocumentFromKnowledgeTool {
    knowledge_manager: Arc<crate::knowledge::KnowledgeManager>,
}

impl RemoveDocumentFromKnowledgeTool {
    pub fn new(knowledge_manager: Arc<crate::knowledge::KnowledgeManager>) -> Self {
        Self { knowledge_manager }
    }
}

#[async_trait]
impl Tool for RemoveDocumentFromKnowledgeTool {
    fn name(&self) -> &str {
        "remove_document_from_knowledge"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "remove_document_from_knowledge".into(),
            description: "Remove a document from a knowledge collection. The document's chunks remain in the database but are disassociated from the collection.".into(),
            input_schema: schema_object(
                json!({
                    "knowledge_name": {
                        "type": "string",
                        "description": "Name of the knowledge collection"
                    },
                    "document_id": {
                        "type": "integer",
                        "description": "Document extraction id to remove"
                    }
                }),
                &["knowledge_name", "document_id"],
            ),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let knowledge_name = match input.get("knowledge_name").and_then(|v| v.as_str()) {
            Some(n) if !n.trim().is_empty() => n.trim().to_string(),
            _ => {
                return ToolResult::error(
                    "Missing or empty required parameter: knowledge_name".into(),
                )
            }
        };

        let doc_id = match input.get("document_id").and_then(|v| v.as_i64()) {
            Some(id) => id,
            None => {
                return ToolResult::error(
                    "Missing required parameter: document_id".into(),
                )
            }
        };

        let km = self.knowledge_manager.clone();
        match tokio::task::spawn_blocking(move || km.remove_document(&knowledge_name, doc_id))
            .await
        {
            Ok(Ok(())) => ToolResult::success("Document removed from knowledge collection.".into()),
            Ok(Err(e)) => ToolResult::error(e),
            Err(e) => ToolResult::error(format!("Task panicked: {e}")),
        }
    }
}

// ── ListKnowledgeTool ──────────────────────────────────────────────────────────

pub struct ListKnowledgeTool {
    knowledge_manager: Arc<crate::knowledge::KnowledgeManager>,
}

impl ListKnowledgeTool {
    pub fn new(knowledge_manager: Arc<crate::knowledge::KnowledgeManager>) -> Self {
        Self { knowledge_manager }
    }
}

#[async_trait]
impl Tool for ListKnowledgeTool {
    fn name(&self) -> &str {
        "list_knowledge"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "list_knowledge".into(),
            description: "List all knowledge collections with their document and chunk statistics.".into(),
            input_schema: schema_object(json!({}), &[]),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let _ = input;
        let km = self.knowledge_manager.clone();
        match tokio::task::spawn_blocking(move || km.list_all()).await {
            Ok(Ok(stats)) => {
                if stats.is_empty() {
                    return ToolResult::success("No knowledge collections found.".into());
                }
                match serde_json::to_string_pretty(&stats) {
                    Ok(json) => ToolResult::success(json),
                    Err(e) => ToolResult::error(format!("Failed to serialize stats: {e}")),
                }
            }
            Ok(Err(e)) => ToolResult::error(e),
            Err(e) => ToolResult::error(format!("Task panicked: {e}")),
        }
    }
}

// ── QueryKnowledgeTool ─────────────────────────────────────────────────────────

pub struct QueryKnowledgeTool {
    knowledge_manager: Arc<crate::knowledge::KnowledgeManager>,
    embedding: Option<Arc<dyn crate::embedding::EmbeddingProvider>>,
}

impl QueryKnowledgeTool {
    pub fn new(
        knowledge_manager: Arc<crate::knowledge::KnowledgeManager>,
        embedding: Option<Arc<dyn crate::embedding::EmbeddingProvider>>,
    ) -> Self {
        Self {
            knowledge_manager,
            embedding,
        }
    }
}

#[async_trait]
impl Tool for QueryKnowledgeTool {
    fn name(&self) -> &str {
        "query_knowledge"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "query_knowledge".into(),
            description: "Search one or more knowledge collections using semantic vector search. Returns the most relevant text chunks ranked by cosine similarity.".into(),
            input_schema: schema_object(
                json!({
                    "query": {
                        "type": "string",
                        "description": "The search query to embed and match against knowledge chunks"
                    },
                    "knowledge_names": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional list of collection names to search. If omitted, all accessible collections are searched."
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Maximum number of results to return (default: 5)"
                    }
                }),
                &["query"],
            ),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let query = match input.get("query").and_then(|v| v.as_str()) {
            Some(q) if !q.trim().is_empty() => q.trim().to_string(),
            _ => return ToolResult::error("Missing or empty required parameter: query".into()),
        };

        let embedding = match &self.embedding {
            Some(e) => e.clone(),
            None => {
                return ToolResult::error("No embedding provider configured".into())
            }
        };

        let chat_id = match auth_context_from_input(&input).map(|a| a.caller_chat_id) {
            Some(id) => id,
            None => return ToolResult::error("No chat context available".into()),
        };

        let knowledge_names: Option<Vec<String>> = input
            .get("knowledge_names")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect()
            });

        let max_results = input
            .get("max_results")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(5);

        let km = self.knowledge_manager.clone();
        match km
            .query(
                knowledge_names.as_deref(),
                &query,
                max_results,
                chat_id,
                embedding.as_ref(),
            )
            .await
        {
            Ok(result) => match serde_json::to_string_pretty(&result) {
                Ok(json) => ToolResult::success(json),
                Err(e) => ToolResult::error(format!("Failed to serialize results: {e}")),
            },
            Err(e) => ToolResult::error(e),
        }
    }
}

// ── AttachKnowledgeTool ────────────────────────────────────────────────────────

pub struct AttachKnowledgeTool {
    knowledge_manager: Arc<crate::knowledge::KnowledgeManager>,
}

impl AttachKnowledgeTool {
    pub fn new(knowledge_manager: Arc<crate::knowledge::KnowledgeManager>) -> Self {
        Self { knowledge_manager }
    }
}

#[async_trait]
impl Tool for AttachKnowledgeTool {
    fn name(&self) -> &str {
        "attach_knowledge"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "attach_knowledge".into(),
            description: "Grant the current chat access to a knowledge collection so it can be queried.".into(),
            input_schema: schema_object(
                json!({
                    "knowledge_name": {
                        "type": "string",
                        "description": "Name of the knowledge collection to attach to this chat"
                    }
                }),
                &["knowledge_name"],
            ),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let knowledge_name = match input.get("knowledge_name").and_then(|v| v.as_str()) {
            Some(n) if !n.trim().is_empty() => n.trim().to_string(),
            _ => {
                return ToolResult::error(
                    "Missing or empty required parameter: knowledge_name".into(),
                )
            }
        };

        let chat_id = match auth_context_from_input(&input).map(|a| a.caller_chat_id) {
            Some(id) => id,
            None => return ToolResult::error("No chat context available".into()),
        };

        let km = self.knowledge_manager.clone();
        match tokio::task::spawn_blocking(move || km.attach(&knowledge_name, chat_id)).await {
            Ok(Ok(())) => ToolResult::success(
                "Knowledge collection attached. This chat can now query it.".into(),
            ),
            Ok(Err(e)) => ToolResult::error(e),
            Err(e) => ToolResult::error(format!("Task panicked: {e}")),
        }
    }
}

// ── DeleteKnowledgeTool ────────────────────────────────────────────────────────

pub struct DeleteKnowledgeTool {
    knowledge_manager: Arc<crate::knowledge::KnowledgeManager>,
}

impl DeleteKnowledgeTool {
    pub fn new(knowledge_manager: Arc<crate::knowledge::KnowledgeManager>) -> Self {
        Self { knowledge_manager }
    }
}

#[async_trait]
impl Tool for DeleteKnowledgeTool {
    fn name(&self) -> &str {
        "delete_knowledge"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "delete_knowledge".into(),
            description: "Permanently delete a knowledge collection. Only the owner of the collection can delete it.".into(),
            input_schema: schema_object(
                json!({
                    "knowledge_name": {
                        "type": "string",
                        "description": "Name of the knowledge collection to delete"
                    }
                }),
                &["knowledge_name"],
            ),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let knowledge_name = match input.get("knowledge_name").and_then(|v| v.as_str()) {
            Some(n) if !n.trim().is_empty() => n.trim().to_string(),
            _ => {
                return ToolResult::error(
                    "Missing or empty required parameter: knowledge_name".into(),
                )
            }
        };

        let chat_id = match auth_context_from_input(&input).map(|a| a.caller_chat_id) {
            Some(id) => id,
            None => return ToolResult::error("No chat context available".into()),
        };

        let km = self.knowledge_manager.clone();
        match tokio::task::spawn_blocking(move || km.delete(&knowledge_name, chat_id)).await {
            Ok(Ok(())) => ToolResult::success("Knowledge collection deleted.".into()),
            Ok(Err(e)) => ToolResult::error(e),
            Err(e) => ToolResult::error(format!("Task panicked: {e}")),
        }
    }
}
