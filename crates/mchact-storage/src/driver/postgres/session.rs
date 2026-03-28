use mchact_core::error::MchactError;

use crate::db::types::{SessionMetaRow, SessionSettings, SessionTreeRow};
use crate::traits::SessionStore;

use super::{not_impl, PgDriver};

impl SessionStore for PgDriver {
    fn save_session(&self, _chat_id: i64, _messages_json: &str) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn save_session_with_meta(
        &self,
        _chat_id: i64,
        _messages_json: &str,
        _parent_session_key: Option<&str>,
        _fork_point: Option<i64>,
        _skill_envs_json: Option<&str>,
    ) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn save_session_skill_envs(
        &self,
        _chat_id: i64,
        _skill_envs_json: &str,
    ) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn load_session(&self, _chat_id: i64) -> Result<Option<(String, String)>, MchactError> {
        Err(not_impl())
    }

    fn load_session_skill_envs(&self, _chat_id: i64) -> Result<Option<String>, MchactError> {
        Err(not_impl())
    }

    fn save_session_settings(
        &self,
        _chat_id: i64,
        _settings: &SessionSettings,
    ) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn load_session_settings(
        &self,
        _chat_id: i64,
    ) -> Result<Option<SessionSettings>, MchactError> {
        Err(not_impl())
    }

    fn load_session_meta(&self, _chat_id: i64) -> Result<Option<SessionMetaRow>, MchactError> {
        Err(not_impl())
    }

    fn list_session_meta(&self, _limit: usize) -> Result<Vec<SessionTreeRow>, MchactError> {
        Err(not_impl())
    }

    fn delete_session(&self, _chat_id: i64) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn clear_chat_context(&self, _chat_id: i64) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn clear_chat_conversation(&self, _chat_id: i64) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn clear_chat_memory(&self, _chat_id: i64) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn delete_chat_data(&self, _chat_id: i64) -> Result<bool, MchactError> {
        Err(not_impl())
    }
}
