use mchact_core::error::MchactError;

use crate::db::types::MediaObject;

pub trait MediaObjectStore {
    #[allow(clippy::too_many_arguments)]
    fn insert_media_object(
        &self,
        key: &str,
        backend: &str,
        chat_id: i64,
        mime_type: Option<&str>,
        size_bytes: Option<i64>,
        hash: Option<&str>,
        source: &str,
    ) -> Result<i64, MchactError>;

    fn get_media_object(&self, id: i64) -> Result<Option<MediaObject>, MchactError>;

    fn get_media_object_by_hash(&self, hash: &str) -> Result<Option<MediaObject>, MchactError>;

    fn list_media_objects_for_chat(
        &self,
        chat_id: i64,
    ) -> Result<Vec<MediaObject>, MchactError>;

    fn delete_media_object(&self, id: i64) -> Result<(), MchactError>;
}
