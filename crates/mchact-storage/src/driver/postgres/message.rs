use mchact_core::error::MchactError;

use crate::db::types::{FtsSearchResult, StoredMessage};
use crate::traits::MessageStore;

use super::{not_impl, PgDriver};

impl MessageStore for PgDriver {
    fn store_message(&self, _msg: &StoredMessage) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn store_message_if_new(&self, _msg: &StoredMessage) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn message_exists(&self, _chat_id: i64, _message_id: &str) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn search_messages_fts(
        &self,
        _query: &str,
        _chat_id: Option<i64>,
        _limit: usize,
    ) -> Result<Vec<FtsSearchResult>, MchactError> {
        Err(not_impl())
    }

    fn get_message_context(
        &self,
        _chat_id: i64,
        _timestamp: &str,
        _window: usize,
    ) -> Result<Vec<StoredMessage>, MchactError> {
        Err(not_impl())
    }

    fn rebuild_fts_index(&self) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn get_recent_messages(
        &self,
        _chat_id: i64,
        _limit: usize,
    ) -> Result<Vec<StoredMessage>, MchactError> {
        Err(not_impl())
    }

    fn get_all_messages(&self, _chat_id: i64) -> Result<Vec<StoredMessage>, MchactError> {
        Err(not_impl())
    }

    fn get_messages_since_last_bot_response(
        &self,
        _chat_id: i64,
        _max: usize,
        _fallback: usize,
    ) -> Result<Vec<StoredMessage>, MchactError> {
        Err(not_impl())
    }

    fn get_new_user_messages_since(
        &self,
        _chat_id: i64,
        _since: &str,
    ) -> Result<Vec<StoredMessage>, MchactError> {
        Err(not_impl())
    }

    fn get_messages_since(
        &self,
        _chat_id: i64,
        _since: &str,
        _limit: usize,
    ) -> Result<Vec<StoredMessage>, MchactError> {
        Err(not_impl())
    }
}
