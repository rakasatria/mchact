use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;

use super::{auth_context_from_input, schema_object, Tool, ToolResult};
use mchact_core::llm_types::ToolDefinition;
use mchact_storage::db::call_blocking;
use mchact_storage::DynDataStore;
use mchact_storage::prelude::*;

fn mime_from_extension(path: &str) -> Option<&'static str> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    match ext.to_lowercase().as_str() {
        "pdf" => Some("application/pdf"),
        "docx" => Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
        "doc" => Some("application/msword"),
        "xlsx" => Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
        "xls" => Some("application/vnd.ms-excel"),
        "pptx" => Some(
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        ),
        "ppt" => Some("application/vnd.ms-powerpoint"),
        "txt" => Some("text/plain"),
        "md" => Some("text/markdown"),
        "csv" => Some("text/csv"),
        _ => None,
    }
}

pub struct ReadDocumentTool {
    db: Arc<DynDataStore>,
    control_chat_ids: Vec<i64>,
    media_manager: Arc<crate::media_manager::MediaManager>,
}

impl ReadDocumentTool {
    pub fn new(
        db: Arc<DynDataStore>,
        control_chat_ids: Vec<i64>,
        media_manager: Arc<crate::media_manager::MediaManager>,
    ) -> Self {
        Self {
            db,
            control_chat_ids,
            media_manager,
        }
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
            input_schema: schema_object(
                json!({
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
                }),
                &[],
            ),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let auth = auth_context_from_input(&input);
        let caller_chat_id = auth.as_ref().map(|a| a.caller_chat_id);
        let is_control = auth
            .as_ref()
            .map(|a| self.control_chat_ids.contains(&a.caller_chat_id))
            .unwrap_or(false);

        // Mode: List documents
        if input.get("list").and_then(|v| v.as_bool()).unwrap_or(false) {
            let chat_id = match caller_chat_id {
                Some(id) => id,
                None => return ToolResult::error("No chat context available".into()),
            };
            let db = self.db.clone();
            return match call_blocking(db, move |db| db.list_document_extractions(chat_id, 20))
                .await
            {
                Ok(docs) => {
                    if docs.is_empty() {
                        return ToolResult::success(
                            "No documents uploaded to this chat.".into(),
                        );
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
                Err(e) => ToolResult::error(format!("Failed to list documents: {e}")),
            };
        }

        // Mode: Search documents
        if let Some(query) = input.get("query").and_then(|v| v.as_str()) {
            let chat_filter = if is_control { None } else { caller_chat_id };
            let db = self.db.clone();
            let q = query.to_string();
            let query_display = query.to_string();
            return match call_blocking(db, move |db| {
                db.search_document_extractions(chat_filter, &q, 10)
            })
            .await
            {
                Ok(docs) => {
                    if docs.is_empty() {
                        return ToolResult::success(format!(
                            "No documents matching \"{query_display}\"."
                        ));
                    }
                    let mut output =
                        format!("Found {} matching documents:\n\n", docs.len());
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
                Err(e) => ToolResult::error(format!("Search failed: {e}")),
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
            let hash_display = hash.to_string();
            return match call_blocking(db, move |db| db.get_document_extraction(chat_id, &h))
                .await
            {
                Ok(Some(doc)) => ToolResult::success(format!(
                    "Document: {}\nSize: {} chars\n\n{}",
                    doc.filename, doc.char_count, doc.extracted_text
                )),
                Ok(None) => ToolResult::error(format!(
                    "No document found with hash {hash_display}"
                )),
                Err(e) => ToolResult::error(format!("Retrieval failed: {e}")),
            };
        }

        // Mode: Extract from file path
        if let Some(file_path) = input.get("file_path").and_then(|v| v.as_str()) {
            match mchact_media::documents::extract_text(file_path).await {
                Ok(text) => {
                    if let Some(chat_id) = caller_chat_id {
                        let file_bytes =
                            tokio::fs::read(file_path).await.unwrap_or_default();
                        let file_hash =
                            mchact_media::documents::compute_file_hash(&file_bytes);
                        let filename = std::path::Path::new(file_path)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let file_size = file_bytes.len() as i64;
                        let mime = mime_from_extension(file_path).map(str::to_string);
                        let db = self.db.clone();
                        let txt = text.clone();
                        let fname = filename.clone();
                        let fhash = file_hash.clone();
                        let mime_ref = mime.clone();
                        let extraction_result = call_blocking(db, move |db| {
                            db.insert_document_extraction(
                                chat_id,
                                &fhash,
                                &fname,
                                mime_ref.as_deref(),
                                file_size,
                                &txt,
                            )
                        })
                        .await;
                        if let Ok(extraction_id) = extraction_result {
                            let store_result = self
                                .media_manager
                                .store_file(
                                    file_bytes,
                                    &filename,
                                    mime.as_deref(),
                                    chat_id,
                                    "document",
                                )
                                .await;
                            if let Ok(media_id) = store_result {
                                let db2 = self.db.clone();
                                let _ = call_blocking(db2, move |db| {
                                    db.set_document_extraction_media_id(extraction_id, media_id)
                                })
                                .await;
                            }
                        }
                    }
                    let display = if text.len() > 50_000 {
                        format!(
                            "{}\n\n(truncated, {} chars total)",
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
            ToolResult::error(
                "Provide one of: file_path, query, list, or file_hash".into(),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mchact_storage::db::Database;
    use mchact_storage_backend::local::LocalStorage;
    use serde_json::json;

    fn make_db() -> Arc<Database> {
        let dir = std::env::temp_dir().join(format!(
            "mchact_read_doc_{}",
            uuid::Uuid::new_v4()
        ));
        Arc::new(Database::new(dir.to_str().unwrap()).unwrap())
    }

    async fn make_media_manager(db: Arc<DynDataStore>) -> Arc<crate::media_manager::MediaManager> {
        let dir = std::env::temp_dir()
            .join(format!("mchact_doc_mm_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let storage: Arc<dyn mchact_storage_backend::ObjectStorage> =
            Arc::new(LocalStorage::new(dir.to_str().unwrap()).await.unwrap());
        Arc::new(crate::media_manager::MediaManager::new(storage, db))
    }

    async fn make_tool(db: Arc<DynDataStore>) -> ReadDocumentTool {
        let mm = make_media_manager(db.clone()).await;
        ReadDocumentTool::new(db, vec![100], mm)
    }

    #[tokio::test]
    async fn test_no_params_returns_error() {
        let tool = make_tool(make_db()).await;
        let result = tool.execute(json!({})).await;
        assert!(result.is_error);
        assert!(result.content.contains("file_path"));
    }

    #[tokio::test]
    async fn test_list_empty_chat_returns_success() {
        let tool = make_tool(make_db()).await;
        let result = tool
            .execute(json!({
                "list": true,
                "__mchact_auth": {
                    "caller_channel": "telegram",
                    "caller_chat_id": 42,
                    "control_chat_ids": []
                }
            }))
            .await;
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("No documents"));
    }

    #[tokio::test]
    async fn test_list_no_auth_returns_error() {
        let tool = make_tool(make_db()).await;
        let result = tool.execute(json!({"list": true})).await;
        assert!(result.is_error);
        assert!(result.content.contains("chat context"));
    }

    #[tokio::test]
    async fn test_search_no_results() {
        let tool = make_tool(make_db()).await;
        let result = tool
            .execute(json!({
                "query": "zzznomatch",
                "__mchact_auth": {
                    "caller_channel": "telegram",
                    "caller_chat_id": 42,
                    "control_chat_ids": []
                }
            }))
            .await;
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("No documents matching"));
    }

    #[tokio::test]
    async fn test_search_finds_inserted_document() {
        let db = make_db();
        db.insert_document_extraction(42, "abc123", "report.pdf", None, 100, "quarterly earnings report data")
            .unwrap();

        let tool = make_tool(db.clone()).await;
        let result = tool
            .execute(json!({
                "query": "quarterly earnings",
                "__mchact_auth": {
                    "caller_channel": "telegram",
                    "caller_chat_id": 42,
                    "control_chat_ids": []
                }
            }))
            .await;
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("report.pdf"));
    }

    #[tokio::test]
    async fn test_retrieve_by_hash() {
        let db = make_db();
        db.insert_document_extraction(42, "hash999", "notes.docx", None, 50, "meeting notes content here")
            .unwrap();

        let tool = make_tool(db.clone()).await;
        let result = tool
            .execute(json!({
                "file_hash": "hash999",
                "__mchact_auth": {
                    "caller_channel": "telegram",
                    "caller_chat_id": 42,
                    "control_chat_ids": []
                }
            }))
            .await;
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("notes.docx"));
        assert!(result.content.contains("meeting notes content here"));
    }

    #[tokio::test]
    async fn test_retrieve_by_hash_not_found() {
        let tool = make_tool(make_db()).await;
        let result = tool
            .execute(json!({
                "file_hash": "nonexistent",
                "__mchact_auth": {
                    "caller_channel": "telegram",
                    "caller_chat_id": 42,
                    "control_chat_ids": []
                }
            }))
            .await;
        assert!(result.is_error);
        assert!(result.content.contains("No document found"));
    }

    #[tokio::test]
    async fn test_retrieve_by_hash_no_auth_returns_error() {
        let tool = make_tool(make_db()).await;
        let result = tool
            .execute(json!({"file_hash": "somehash"}))
            .await;
        assert!(result.is_error);
        assert!(result.content.contains("chat context"));
    }

    #[tokio::test]
    async fn test_extract_text_propagates_not_configured() {
        let tool = make_tool(make_db()).await;
        let result = tool
            .execute(json!({
                "file_path": "/tmp/test_nonexistent_doc.pdf",
                "__mchact_auth": {
                    "caller_channel": "telegram",
                    "caller_chat_id": 42,
                    "control_chat_ids": []
                }
            }))
            .await;
        // Without the 'documents' feature, this will return an extraction error
        assert!(result.is_error);
        assert!(result.content.contains("Extraction failed"));
    }

    #[tokio::test]
    async fn test_list_shows_inserted_document() {
        let db = make_db();
        db.insert_document_extraction(55, "filehash1", "slides.pptx", None, 2048, "slide content here")
            .unwrap();

        let tool = make_tool(db.clone()).await;
        let result = tool
            .execute(json!({
                "list": true,
                "__mchact_auth": {
                    "caller_channel": "telegram",
                    "caller_chat_id": 55,
                    "control_chat_ids": []
                }
            }))
            .await;
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("slides.pptx"));
        assert!(result.content.contains("filehash1"));
    }

    #[tokio::test]
    async fn test_control_chat_can_search_all_chats() {
        let db = make_db();
        // Insert document for a different chat
        db.insert_document_extraction(999, "xhash", "private.pdf", None, 100, "cross chat searchable content")
            .unwrap();

        let tool = make_tool(db.clone()).await; // control_chat_ids = [100]
        let result = tool
            .execute(json!({
                "query": "cross chat searchable",
                "__mchact_auth": {
                    "caller_channel": "telegram",
                    "caller_chat_id": 100,
                    "control_chat_ids": [100]
                }
            }))
            .await;
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("private.pdf"));
    }

    #[tokio::test]
    async fn test_non_control_restricted_to_own_chat() {
        let db = make_db();
        // Insert document for chat 999
        db.insert_document_extraction(999, "yhash", "other.pdf", None, 100, "other chat unique phrase")
            .unwrap();

        let tool = make_tool(db.clone()).await; // control_chat_ids = [100]
        let result = tool
            .execute(json!({
                "query": "other chat unique phrase",
                "__mchact_auth": {
                    "caller_channel": "telegram",
                    "caller_chat_id": 42,
                    "control_chat_ids": []
                }
            }))
            .await;
        assert!(!result.is_error, "{}", result.content);
        // Non-control chat 42 should NOT find documents from chat 999
        assert!(result.content.contains("No documents matching"));
    }
}
