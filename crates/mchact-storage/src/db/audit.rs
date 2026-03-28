use mchact_core::error::MchactError;
use rusqlite::params;

use super::Database;
use super::AuditLogRecord;

impl Database {
    pub fn log_audit_event(
        &self,
        kind: &str,
        actor: &str,
        action: &str,
        target: Option<&str>,
        status: &str,
        detail: Option<&str>,
    ) -> Result<i64, MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO audit_logs(kind, actor, action, target, status, detail, created_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![kind, actor, action, target, status, detail, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_audit_logs(
        &self,
        kind: Option<&str>,
        limit: usize,
    ) -> Result<Vec<AuditLogRecord>, MchactError> {
        let conn = self.lock_conn();
        let mut rows = Vec::new();
        if let Some(k) = kind {
            let mut stmt = conn.prepare(
                "SELECT id, kind, actor, action, target, status, detail, created_at
                 FROM audit_logs
                 WHERE kind = ?1
                 ORDER BY id DESC
                 LIMIT ?2",
            )?;
            let iter = stmt.query_map(params![k, limit as i64], |row| {
                Ok(AuditLogRecord {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    actor: row.get(2)?,
                    action: row.get(3)?,
                    target: row.get(4)?,
                    status: row.get(5)?,
                    detail: row.get(6)?,
                    created_at: row.get(7)?,
                })
            })?;
            for item in iter {
                rows.push(item?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, kind, actor, action, target, status, detail, created_at
                 FROM audit_logs
                 ORDER BY id DESC
                 LIMIT ?1",
            )?;
            let iter = stmt.query_map(params![limit as i64], |row| {
                Ok(AuditLogRecord {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    actor: row.get(2)?,
                    action: row.get(3)?,
                    target: row.get(4)?,
                    status: row.get(5)?,
                    detail: row.get(6)?,
                    created_at: row.get(7)?,
                })
            })?;
            for item in iter {
                rows.push(item?);
            }
        }
        Ok(rows)
    }
}
