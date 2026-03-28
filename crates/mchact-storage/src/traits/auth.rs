use mchact_core::error::MchactError;

use crate::db::types::AuthApiKeyRecord;

pub trait AuthStore {
    fn upsert_auth_password_hash(&self, password_hash: &str) -> Result<(), MchactError>;

    fn get_auth_password_hash(&self) -> Result<Option<String>, MchactError>;

    fn clear_auth_password_hash(&self) -> Result<bool, MchactError>;

    fn create_auth_session(
        &self,
        session_id: &str,
        label: Option<&str>,
        expires_at: &str,
    ) -> Result<(), MchactError>;

    fn validate_auth_session(&self, session_id: &str) -> Result<bool, MchactError>;

    fn revoke_auth_session(&self, session_id: &str) -> Result<bool, MchactError>;

    fn revoke_all_auth_sessions(&self) -> Result<usize, MchactError>;

    fn create_api_key(
        &self,
        label: &str,
        key_hash: &str,
        prefix: &str,
        scopes: &[String],
        expires_at: Option<&str>,
        rotated_from_key_id: Option<i64>,
    ) -> Result<i64, MchactError>;

    fn list_api_keys(&self) -> Result<Vec<AuthApiKeyRecord>, MchactError>;

    fn rotate_api_key_revoke_old(&self, old_key_id: i64) -> Result<bool, MchactError>;

    fn revoke_api_key(&self, key_id: i64) -> Result<bool, MchactError>;

    fn validate_api_key_hash(
        &self,
        key_hash: &str,
    ) -> Result<Option<(i64, Vec<String>)>, MchactError>;
}
