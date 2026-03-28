use mchact_core::error::MchactError;

use crate::db::types::AuditLogRecord;
use crate::traits::AuditStore;

use super::PgDriver;

fn pg_err(e: tokio_postgres::Error) -> MchactError {
    MchactError::ToolExecution(format!("postgres: {e}"))
}

fn pool_err(e: deadpool_postgres::PoolError) -> MchactError {
    MchactError::ToolExecution(format!("pool: {e}"))
}

impl AuditStore for PgDriver {
    fn log_audit_event(
        &self,
        kind: &str,
        actor: &str,
        action: &str,
        target: Option<&str>,
        status: &str,
        detail: Option<&str>,
    ) -> Result<i64, MchactError> {
        let pool = self.pool.clone();
        let now = chrono::Utc::now().to_rfc3339();
        let kind = kind.to_string();
        let actor = actor.to_string();
        let action = action.to_string();
        let target = target.map(|s| s.to_string());
        let status = status.to_string();
        let detail = detail.map(|s| s.to_string());
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            let row = client
                .query_one(
                    "INSERT INTO audit_logs(kind, actor, action, target, status, detail, created_at)
                     VALUES($1, $2, $3, $4, $5, $6, $7)
                     RETURNING id",
                    &[&kind, &actor, &action, &target, &status, &detail, &now],
                )
                .await
                .map_err(pg_err)?;
            Ok(row.get::<_, i64>("id"))
        })
    }

    fn list_audit_logs(
        &self,
        kind: Option<&str>,
        limit: usize,
    ) -> Result<Vec<AuditLogRecord>, MchactError> {
        let pool = self.pool.clone();
        let kind = kind.map(|s| s.to_string());
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            let rows = if let Some(k) = kind {
                client
                    .query(
                        "SELECT id, kind, actor, action, target, status, detail, created_at
                         FROM audit_logs
                         WHERE kind = $1
                         ORDER BY id DESC
                         LIMIT $2",
                        &[&k, &(limit as i64)],
                    )
                    .await
                    .map_err(pg_err)?
            } else {
                client
                    .query(
                        "SELECT id, kind, actor, action, target, status, detail, created_at
                         FROM audit_logs
                         ORDER BY id DESC
                         LIMIT $1",
                        &[&(limit as i64)],
                    )
                    .await
                    .map_err(pg_err)?
            };
            let records = rows
                .iter()
                .map(|row| AuditLogRecord {
                    id: row.get("id"),
                    kind: row.get("kind"),
                    actor: row.get("actor"),
                    action: row.get("action"),
                    target: row.get("target"),
                    status: row.get("status"),
                    detail: row.get("detail"),
                    created_at: row.get("created_at"),
                })
                .collect();
            Ok(records)
        })
    }
}
