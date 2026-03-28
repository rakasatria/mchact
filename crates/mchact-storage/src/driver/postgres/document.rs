use mchact_core::error::MchactError;

use crate::db::types::DocumentExtraction;
use crate::traits::DocumentStore;

use super::{not_impl, PgDriver};

impl DocumentStore for PgDriver {
    fn insert_document_extraction(
        &self,
        _chat_id: i64,
        _file_hash: &str,
        _filename: &str,
        _mime_type: Option<&str>,
        _file_size: i64,
        _extracted_text: &str,
    ) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn get_document_extraction(
        &self,
        _chat_id: i64,
        _file_hash: &str,
    ) -> Result<Option<DocumentExtraction>, MchactError> {
        Err(not_impl())
    }

    fn search_document_extractions(
        &self,
        _chat_id: Option<i64>,
        _query: &str,
        _limit: usize,
    ) -> Result<Vec<DocumentExtraction>, MchactError> {
        Err(not_impl())
    }

    fn list_document_extractions(
        &self,
        _chat_id: i64,
        _limit: usize,
    ) -> Result<Vec<DocumentExtraction>, MchactError> {
        Err(not_impl())
    }

    fn get_document_extraction_by_id(
        &self,
        _id: i64,
    ) -> Result<Option<DocumentExtraction>, MchactError> {
        Err(not_impl())
    }

    fn set_document_extraction_media_id(
        &self,
        _extraction_id: i64,
        _media_object_id: i64,
    ) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn get_document_extraction_id_by_media_object_id(
        &self,
        _media_object_id: i64,
    ) -> Result<Option<i64>, MchactError> {
        Err(not_impl())
    }
}
