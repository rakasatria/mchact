use mchact_core::error::MchactError;

use crate::db::types::{DocumentChunk, Knowledge};

pub trait KnowledgeStore {
    fn create_knowledge(
        &self,
        name: &str,
        description: &str,
        owner_chat_id: i64,
    ) -> Result<i64, MchactError>;

    fn get_knowledge_by_name(&self, name: &str) -> Result<Option<Knowledge>, MchactError>;

    fn list_knowledge(&self) -> Result<Vec<Knowledge>, MchactError>;

    fn delete_knowledge(&self, id: i64) -> Result<(), MchactError>;

    fn update_knowledge_timestamp(&self, id: i64) -> Result<(), MchactError>;

    fn update_knowledge_grouping_check(
        &self,
        knowledge_id: i64,
        doc_count: i64,
    ) -> Result<(), MchactError>;

    fn get_knowledge_needing_grouping(
        &self,
        min_docs: i64,
    ) -> Result<Vec<Knowledge>, MchactError>;

    fn add_document_to_knowledge(
        &self,
        knowledge_id: i64,
        doc_extraction_id: i64,
    ) -> Result<(), MchactError>;

    fn remove_document_from_knowledge(
        &self,
        knowledge_id: i64,
        doc_extraction_id: i64,
    ) -> Result<(), MchactError>;

    fn list_knowledge_documents(&self, knowledge_id: i64) -> Result<Vec<i64>, MchactError>;

    fn count_knowledge_documents(&self, knowledge_id: i64) -> Result<i64, MchactError>;

    fn add_knowledge_chat_access(
        &self,
        knowledge_id: i64,
        chat_id: i64,
    ) -> Result<(), MchactError>;

    fn has_knowledge_chat_access(
        &self,
        knowledge_id: i64,
        chat_id: i64,
    ) -> Result<bool, MchactError>;

    fn list_knowledge_for_chat(&self, chat_id: i64) -> Result<Vec<Knowledge>, MchactError>;

    fn list_knowledge_chat_ids(&self, knowledge_id: i64) -> Result<Vec<i64>, MchactError>;

    fn insert_document_chunk(
        &self,
        doc_extraction_id: i64,
        page_number: i64,
        text: &str,
        token_count: Option<i64>,
    ) -> Result<i64, MchactError>;

    fn get_chunks_by_status(
        &self,
        embedding_status: &str,
        limit: i64,
    ) -> Result<Vec<DocumentChunk>, MchactError>;

    fn get_chunks_for_observation(&self, limit: i64) -> Result<Vec<DocumentChunk>, MchactError>;

    fn update_chunk_embedding(
        &self,
        chunk_id: i64,
        embedding_bytes: &[u8],
        status: &str,
    ) -> Result<(), MchactError>;

    fn update_chunk_observation_status(
        &self,
        chunk_id: i64,
        status: &str,
    ) -> Result<(), MchactError>;

    fn get_chunks_for_document(
        &self,
        doc_extraction_id: i64,
    ) -> Result<Vec<DocumentChunk>, MchactError>;

    fn reset_failed_chunks(&self, older_than_mins: i64) -> Result<i64, MchactError>;

    fn get_knowledge_chunk_stats(
        &self,
        knowledge_id: i64,
    ) -> Result<(i64, i64, i64, i64, i64, i64), MchactError>;
}
