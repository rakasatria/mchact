use mchact_core::error::MchactError;

use crate::db::types::{ChatSummary};

pub trait ChatStore {
    fn upsert_chat(
        &self,
        chat_id: i64,
        chat_title: Option<&str>,
        chat_type: &str,
    ) -> Result<(), MchactError>;

    fn resolve_or_create_chat_id(
        &self,
        channel: &str,
        external_chat_id: &str,
        chat_title: Option<&str>,
        chat_type: &str,
    ) -> Result<i64, MchactError>;

    fn get_chat_type(&self, chat_id: i64) -> Result<Option<String>, MchactError>;

    fn get_chat_id_by_channel_and_title(
        &self,
        channel: &str,
        chat_title: &str,
    ) -> Result<Option<i64>, MchactError>;

    fn get_chat_channel(&self, chat_id: i64) -> Result<Option<String>, MchactError>;

    fn get_chat_external_id(&self, chat_id: i64) -> Result<Option<String>, MchactError>;

    fn get_recent_chats(&self, limit: usize) -> Result<Vec<ChatSummary>, MchactError>;

    fn get_chats_by_type(
        &self,
        chat_type: &str,
        limit: usize,
    ) -> Result<Vec<ChatSummary>, MchactError>;
}
