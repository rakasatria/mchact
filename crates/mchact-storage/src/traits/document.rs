use mchact_core::error::MchactError;

use crate::db::types::DocumentExtraction;

pub trait DocumentStore {
    fn insert_document_extraction(
        &self,
        chat_id: i64,
        file_hash: &str,
        filename: &str,
        mime_type: Option<&str>,
        file_size: i64,
        extracted_text: &str,
    ) -> Result<i64, MchactError>;

    fn get_document_extraction(
        &self,
        chat_id: i64,
        file_hash: &str,
    ) -> Result<Option<DocumentExtraction>, MchactError>;

    fn search_document_extractions(
        &self,
        chat_id: Option<i64>,
        query: &str,
        limit: usize,
    ) -> Result<Vec<DocumentExtraction>, MchactError>;

    fn list_document_extractions(
        &self,
        chat_id: i64,
        limit: usize,
    ) -> Result<Vec<DocumentExtraction>, MchactError>;

    fn get_document_extraction_by_id(
        &self,
        id: i64,
    ) -> Result<Option<DocumentExtraction>, MchactError>;

    fn set_document_extraction_media_id(
        &self,
        extraction_id: i64,
        media_object_id: i64,
    ) -> Result<(), MchactError>;

    fn get_document_extraction_id_by_media_object_id(
        &self,
        media_object_id: i64,
    ) -> Result<Option<i64>, MchactError>;
}
