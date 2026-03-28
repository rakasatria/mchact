use mchact_core::error::MchactError;

use crate::db::types::ChatSummary;
use crate::traits::ChatStore;

use super::{not_impl, PgDriver};

impl ChatStore for PgDriver {
    fn upsert_chat(
        &self,
        _chat_id: i64,
        _chat_title: Option<&str>,
        _chat_type: &str,
    ) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn resolve_or_create_chat_id(
        &self,
        _channel: &str,
        _external_chat_id: &str,
        _chat_title: Option<&str>,
        _chat_type: &str,
    ) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn get_chat_type(&self, _chat_id: i64) -> Result<Option<String>, MchactError> {
        Err(not_impl())
    }

    fn get_chat_id_by_channel_and_title(
        &self,
        _channel: &str,
        _chat_title: &str,
    ) -> Result<Option<i64>, MchactError> {
        Err(not_impl())
    }

    fn get_chat_channel(&self, _chat_id: i64) -> Result<Option<String>, MchactError> {
        Err(not_impl())
    }

    fn get_chat_external_id(&self, _chat_id: i64) -> Result<Option<String>, MchactError> {
        Err(not_impl())
    }

    fn get_recent_chats(&self, _limit: usize) -> Result<Vec<ChatSummary>, MchactError> {
        Err(not_impl())
    }

    fn get_chats_by_type(
        &self,
        _chat_type: &str,
        _limit: usize,
    ) -> Result<Vec<ChatSummary>, MchactError> {
        Err(not_impl())
    }
}
