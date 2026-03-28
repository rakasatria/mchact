use mchact_core::error::MchactError;

use crate::db::types::{DocumentChunk, Knowledge};
use crate::traits::KnowledgeStore;

use super::{not_impl, PgDriver};

impl KnowledgeStore for PgDriver {
    fn create_knowledge(
        &self,
        _name: &str,
        _description: &str,
        _owner_chat_id: i64,
    ) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn get_knowledge_by_name(&self, _name: &str) -> Result<Option<Knowledge>, MchactError> {
        Err(not_impl())
    }

    fn list_knowledge(&self) -> Result<Vec<Knowledge>, MchactError> {
        Err(not_impl())
    }

    fn delete_knowledge(&self, _id: i64) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn update_knowledge_timestamp(&self, _id: i64) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn update_knowledge_grouping_check(
        &self,
        _knowledge_id: i64,
        _doc_count: i64,
    ) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn get_knowledge_needing_grouping(
        &self,
        _min_docs: i64,
    ) -> Result<Vec<Knowledge>, MchactError> {
        Err(not_impl())
    }

    fn add_document_to_knowledge(
        &self,
        _knowledge_id: i64,
        _doc_extraction_id: i64,
    ) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn remove_document_from_knowledge(
        &self,
        _knowledge_id: i64,
        _doc_extraction_id: i64,
    ) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn list_knowledge_documents(&self, _knowledge_id: i64) -> Result<Vec<i64>, MchactError> {
        Err(not_impl())
    }

    fn count_knowledge_documents(&self, _knowledge_id: i64) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn add_knowledge_chat_access(
        &self,
        _knowledge_id: i64,
        _chat_id: i64,
    ) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn has_knowledge_chat_access(
        &self,
        _knowledge_id: i64,
        _chat_id: i64,
    ) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn list_knowledge_for_chat(&self, _chat_id: i64) -> Result<Vec<Knowledge>, MchactError> {
        Err(not_impl())
    }

    fn list_knowledge_chat_ids(&self, _knowledge_id: i64) -> Result<Vec<i64>, MchactError> {
        Err(not_impl())
    }

    fn insert_document_chunk(
        &self,
        _doc_extraction_id: i64,
        _page_number: i64,
        _text: &str,
        _token_count: Option<i64>,
    ) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn get_chunks_by_status(
        &self,
        _embedding_status: &str,
        _limit: i64,
    ) -> Result<Vec<DocumentChunk>, MchactError> {
        Err(not_impl())
    }

    fn get_chunks_for_observation(&self, _limit: i64) -> Result<Vec<DocumentChunk>, MchactError> {
        Err(not_impl())
    }

    fn update_chunk_embedding(
        &self,
        _chunk_id: i64,
        _embedding_bytes: &[u8],
        _status: &str,
    ) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn update_chunk_observation_status(
        &self,
        _chunk_id: i64,
        _status: &str,
    ) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn get_chunks_for_document(
        &self,
        _doc_extraction_id: i64,
    ) -> Result<Vec<DocumentChunk>, MchactError> {
        Err(not_impl())
    }

    fn reset_failed_chunks(&self, _older_than_mins: i64) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn get_knowledge_chunk_stats(
        &self,
        _knowledge_id: i64,
    ) -> Result<(i64, i64, i64, i64, i64, i64), MchactError> {
        Err(not_impl())
    }
}
