use mchact_core::error::MchactError;

use crate::db::types::{SessionMetaRow, SessionSettings, SessionTreeRow};

pub trait SessionStore {
    fn save_session(&self, chat_id: i64, messages_json: &str) -> Result<(), MchactError>;

    fn save_session_with_meta(
        &self,
        chat_id: i64,
        messages_json: &str,
        parent_session_key: Option<&str>,
        fork_point: Option<i64>,
        skill_envs_json: Option<&str>,
    ) -> Result<(), MchactError>;

    fn save_session_skill_envs(
        &self,
        chat_id: i64,
        skill_envs_json: &str,
    ) -> Result<(), MchactError>;

    fn load_session(&self, chat_id: i64) -> Result<Option<(String, String)>, MchactError>;

    fn load_session_skill_envs(&self, chat_id: i64) -> Result<Option<String>, MchactError>;

    fn save_session_settings(
        &self,
        chat_id: i64,
        settings: &SessionSettings,
    ) -> Result<(), MchactError>;

    fn load_session_settings(
        &self,
        chat_id: i64,
    ) -> Result<Option<SessionSettings>, MchactError>;

    fn load_session_meta(
        &self,
        chat_id: i64,
    ) -> Result<Option<SessionMetaRow>, MchactError>;

    fn list_session_meta(&self, limit: usize) -> Result<Vec<SessionTreeRow>, MchactError>;

    fn delete_session(&self, chat_id: i64) -> Result<bool, MchactError>;

    fn clear_chat_context(&self, chat_id: i64) -> Result<bool, MchactError>;

    fn clear_chat_conversation(&self, chat_id: i64) -> Result<bool, MchactError>;

    fn clear_chat_memory(&self, chat_id: i64) -> Result<bool, MchactError>;

    fn delete_chat_data(&self, chat_id: i64) -> Result<bool, MchactError>;
}
