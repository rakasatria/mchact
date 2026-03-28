use mchact_core::error::MchactError;

use crate::db::types::AuditLogRecord;

pub trait AuditStore {
    fn log_audit_event(
        &self,
        kind: &str,
        actor: &str,
        action: &str,
        target: Option<&str>,
        status: &str,
        detail: Option<&str>,
    ) -> Result<i64, MchactError>;

    fn list_audit_logs(
        &self,
        kind: Option<&str>,
        limit: usize,
    ) -> Result<Vec<AuditLogRecord>, MchactError>;
}
