use mchact_core::error::MchactError;

use crate::db::types::MediaObject;
use crate::traits::MediaObjectStore;

use super::{not_impl, PgDriver};

impl MediaObjectStore for PgDriver {
    fn insert_media_object(
        &self,
        _key: &str,
        _backend: &str,
        _chat_id: i64,
        _mime_type: Option<&str>,
        _size_bytes: Option<i64>,
        _hash: Option<&str>,
        _source: &str,
    ) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn get_media_object(&self, _id: i64) -> Result<Option<MediaObject>, MchactError> {
        Err(not_impl())
    }

    fn get_media_object_by_hash(&self, _hash: &str) -> Result<Option<MediaObject>, MchactError> {
        Err(not_impl())
    }

    fn list_media_objects_for_chat(
        &self,
        _chat_id: i64,
    ) -> Result<Vec<MediaObject>, MchactError> {
        Err(not_impl())
    }

    fn delete_media_object(&self, _id: i64) -> Result<(), MchactError> {
        Err(not_impl())
    }
}
