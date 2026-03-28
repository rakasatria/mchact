use mchact_core::error::MchactError;

use crate::db::types::AuthApiKeyRecord;
use crate::traits::AuthStore;

use super::{not_impl, PgDriver};

impl AuthStore for PgDriver {
    fn upsert_auth_password_hash(&self, _password_hash: &str) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn get_auth_password_hash(&self) -> Result<Option<String>, MchactError> {
        Err(not_impl())
    }

    fn clear_auth_password_hash(&self) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn create_auth_session(
        &self,
        _session_id: &str,
        _label: Option<&str>,
        _expires_at: &str,
    ) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn validate_auth_session(&self, _session_id: &str) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn revoke_auth_session(&self, _session_id: &str) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn revoke_all_auth_sessions(&self) -> Result<usize, MchactError> {
        Err(not_impl())
    }

    fn create_api_key(
        &self,
        _label: &str,
        _key_hash: &str,
        _prefix: &str,
        _scopes: &[String],
        _expires_at: Option<&str>,
        _rotated_from_key_id: Option<i64>,
    ) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn list_api_keys(&self) -> Result<Vec<AuthApiKeyRecord>, MchactError> {
        Err(not_impl())
    }

    fn rotate_api_key_revoke_old(&self, _old_key_id: i64) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn revoke_api_key(&self, _key_id: i64) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn validate_api_key_hash(
        &self,
        _key_hash: &str,
    ) -> Result<Option<(i64, Vec<String>)>, MchactError> {
        Err(not_impl())
    }
}
