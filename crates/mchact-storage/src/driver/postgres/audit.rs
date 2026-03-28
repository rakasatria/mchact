use mchact_core::error::MchactError;

use crate::db::types::AuditLogRecord;
use crate::traits::AuditStore;

use super::{not_impl, PgDriver};

impl AuditStore for PgDriver {
    fn log_audit_event(
        &self,
        _kind: &str,
        _actor: &str,
        _action: &str,
        _target: Option<&str>,
        _status: &str,
        _detail: Option<&str>,
    ) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn list_audit_logs(
        &self,
        _kind: Option<&str>,
        _limit: usize,
    ) -> Result<Vec<AuditLogRecord>, MchactError> {
        Err(not_impl())
    }
}
