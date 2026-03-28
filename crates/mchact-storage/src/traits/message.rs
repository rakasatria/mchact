use mchact_core::error::MchactError;

use crate::db::types::{FtsSearchResult, StoredMessage};

pub trait MessageStore {
    fn store_message(&self, msg: &StoredMessage) -> Result<(), MchactError>;

    fn store_message_if_new(&self, msg: &StoredMessage) -> Result<bool, MchactError>;

    fn message_exists(&self, chat_id: i64, message_id: &str) -> Result<bool, MchactError>;

    fn search_messages_fts(
        &self,
        query: &str,
        chat_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<FtsSearchResult>, MchactError>;

    fn get_message_context(
        &self,
        chat_id: i64,
        timestamp: &str,
        window: usize,
    ) -> Result<Vec<StoredMessage>, MchactError>;

    fn rebuild_fts_index(&self) -> Result<(), MchactError>;

    fn get_recent_messages(
        &self,
        chat_id: i64,
        limit: usize,
    ) -> Result<Vec<StoredMessage>, MchactError>;

    fn get_all_messages(&self, chat_id: i64) -> Result<Vec<StoredMessage>, MchactError>;

    fn get_messages_since_last_bot_response(
        &self,
        chat_id: i64,
        max: usize,
        fallback: usize,
    ) -> Result<Vec<StoredMessage>, MchactError>;

    fn get_new_user_messages_since(
        &self,
        chat_id: i64,
        since: &str,
    ) -> Result<Vec<StoredMessage>, MchactError>;

    fn get_messages_since(
        &self,
        chat_id: i64,
        since: &str,
        limit: usize,
    ) -> Result<Vec<StoredMessage>, MchactError>;
}
