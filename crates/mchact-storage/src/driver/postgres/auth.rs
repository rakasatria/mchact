use mchact_core::error::MchactError;

use crate::db::types::AuthApiKeyRecord;
use crate::traits::AuthStore;

use super::PgDriver;

fn pg_err(e: tokio_postgres::Error) -> MchactError {
    MchactError::ToolExecution(format!("postgres: {e}"))
}

fn pool_err(e: deadpool_postgres::PoolError) -> MchactError {
    MchactError::ToolExecution(format!("pool: {e}"))
}

impl AuthStore for PgDriver {
    fn upsert_auth_password_hash(&self, password_hash: &str) -> Result<(), MchactError> {
        let pool = self.pool.clone();
        let now = chrono::Utc::now().to_rfc3339();
        let password_hash = password_hash.to_string();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            client
                .execute(
                    "INSERT INTO auth_passwords(id, password_hash, created_at, updated_at)
                     VALUES(1, $1, $2, $2)
                     ON CONFLICT(id) DO UPDATE SET
                        password_hash = EXCLUDED.password_hash,
                        updated_at = EXCLUDED.updated_at",
                    &[&password_hash, &now],
                )
                .await
                .map_err(pg_err)?;
            Ok(())
        })
    }

    fn get_auth_password_hash(&self) -> Result<Option<String>, MchactError> {
        let pool = self.pool.clone();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            let row = client
                .query_opt(
                    "SELECT password_hash FROM auth_passwords WHERE id = 1",
                    &[],
                )
                .await
                .map_err(pg_err)?;
            Ok(row.map(|r| r.get::<_, String>("password_hash")))
        })
    }

    fn clear_auth_password_hash(&self) -> Result<bool, MchactError> {
        let pool = self.pool.clone();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            let n = client
                .execute("DELETE FROM auth_passwords WHERE id = 1", &[])
                .await
                .map_err(pg_err)?;
            Ok(n > 0)
        })
    }

    fn create_auth_session(
        &self,
        session_id: &str,
        label: Option<&str>,
        expires_at: &str,
    ) -> Result<(), MchactError> {
        let pool = self.pool.clone();
        let now = chrono::Utc::now().to_rfc3339();
        let session_id = session_id.to_string();
        let label = label.map(|s| s.to_string());
        let expires_at = expires_at.to_string();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            client
                .execute(
                    "INSERT INTO auth_sessions(session_id, label, created_at, expires_at, last_seen_at, revoked_at)
                     VALUES($1, $2, $3, $4, $3, NULL)",
                    &[&session_id, &label, &now, &expires_at],
                )
                .await
                .map_err(pg_err)?;
            Ok(())
        })
    }

    fn validate_auth_session(&self, session_id: &str) -> Result<bool, MchactError> {
        let pool = self.pool.clone();
        let now = chrono::Utc::now().to_rfc3339();
        let session_id = session_id.to_string();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            let row = client
                .query_opt(
                    "SELECT 1 FROM auth_sessions
                     WHERE session_id = $1
                       AND revoked_at IS NULL
                       AND expires_at > $2
                     LIMIT 1",
                    &[&session_id, &now],
                )
                .await
                .map_err(pg_err)?;
            let valid = row.is_some();
            if valid {
                let _ = client
                    .execute(
                        "UPDATE auth_sessions SET last_seen_at = $2 WHERE session_id = $1",
                        &[&session_id, &now],
                    )
                    .await;
            }
            Ok(valid)
        })
    }

    fn revoke_auth_session(&self, session_id: &str) -> Result<bool, MchactError> {
        let pool = self.pool.clone();
        let now = chrono::Utc::now().to_rfc3339();
        let session_id = session_id.to_string();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            let n = client
                .execute(
                    "UPDATE auth_sessions
                     SET revoked_at = COALESCE(revoked_at, $2)
                     WHERE session_id = $1",
                    &[&session_id, &now],
                )
                .await
                .map_err(pg_err)?;
            Ok(n > 0)
        })
    }

    fn revoke_all_auth_sessions(&self) -> Result<usize, MchactError> {
        let pool = self.pool.clone();
        let now = chrono::Utc::now().to_rfc3339();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            let n = client
                .execute(
                    "UPDATE auth_sessions
                     SET revoked_at = COALESCE(revoked_at, $1)
                     WHERE revoked_at IS NULL",
                    &[&now],
                )
                .await
                .map_err(pg_err)?;
            Ok(n as usize)
        })
    }

    fn create_api_key(
        &self,
        label: &str,
        key_hash: &str,
        prefix: &str,
        scopes: &[String],
        expires_at: Option<&str>,
        rotated_from_key_id: Option<i64>,
    ) -> Result<i64, MchactError> {
        let pool = self.pool.clone();
        let now = chrono::Utc::now().to_rfc3339();
        let label = label.to_string();
        let key_hash = key_hash.to_string();
        let prefix = prefix.to_string();
        let scopes: Vec<String> = scopes.to_vec();
        let expires_at = expires_at.map(|s| s.to_string());
        tokio::runtime::Handle::current().block_on(async move {
            let mut client = pool.get().await.map_err(pool_err)?;
            let tx = client.transaction().await.map_err(pg_err)?;
            let row = tx
                .query_one(
                    "INSERT INTO api_keys(label, key_hash, prefix, created_at, expires_at, rotated_from_key_id)
                     VALUES($1, $2, $3, $4, $5, $6)
                     RETURNING id",
                    &[&label, &key_hash, &prefix, &now, &expires_at, &rotated_from_key_id],
                )
                .await
                .map_err(pg_err)?;
            let key_id: i64 = row.get("id");
            for scope in &scopes {
                tx.execute(
                    "INSERT INTO api_key_scopes(api_key_id, scope) VALUES($1, $2)
                     ON CONFLICT DO NOTHING",
                    &[&key_id, scope],
                )
                .await
                .map_err(pg_err)?;
            }
            tx.commit().await.map_err(pg_err)?;
            Ok(key_id)
        })
    }

    fn list_api_keys(&self) -> Result<Vec<AuthApiKeyRecord>, MchactError> {
        let pool = self.pool.clone();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            let rows = client
                .query(
                    "SELECT id, label, prefix, created_at, revoked_at, expires_at,
                            last_used_at, rotated_from_key_id
                     FROM api_keys
                     ORDER BY id DESC",
                    &[],
                )
                .await
                .map_err(pg_err)?;
            let mut out = Vec::new();
            for row in &rows {
                let id: i64 = row.get("id");
                let scope_rows = client
                    .query(
                        "SELECT scope FROM api_key_scopes WHERE api_key_id = $1 ORDER BY scope ASC",
                        &[&id],
                    )
                    .await
                    .map_err(pg_err)?;
                let scopes: Vec<String> = scope_rows.iter().map(|r| r.get("scope")).collect();
                out.push(AuthApiKeyRecord {
                    id,
                    label: row.get("label"),
                    prefix: row.get("prefix"),
                    created_at: row.get("created_at"),
                    revoked_at: row.get("revoked_at"),
                    expires_at: row.get("expires_at"),
                    last_used_at: row.get("last_used_at"),
                    rotated_from_key_id: row.get("rotated_from_key_id"),
                    scopes,
                });
            }
            Ok(out)
        })
    }

    fn rotate_api_key_revoke_old(&self, old_key_id: i64) -> Result<bool, MchactError> {
        self.revoke_api_key(old_key_id)
    }

    fn revoke_api_key(&self, key_id: i64) -> Result<bool, MchactError> {
        let pool = self.pool.clone();
        let now = chrono::Utc::now().to_rfc3339();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            let n = client
                .execute(
                    "UPDATE api_keys
                     SET revoked_at = COALESCE(revoked_at, $2)
                     WHERE id = $1",
                    &[&key_id, &now],
                )
                .await
                .map_err(pg_err)?;
            Ok(n > 0)
        })
    }

    fn validate_api_key_hash(
        &self,
        key_hash: &str,
    ) -> Result<Option<(i64, Vec<String>)>, MchactError> {
        let pool = self.pool.clone();
        let key_hash = key_hash.to_string();
        let now = chrono::Utc::now().to_rfc3339();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            let row = client
                .query_opt(
                    "SELECT id FROM api_keys
                     WHERE key_hash = $1
                       AND revoked_at IS NULL
                       AND (expires_at IS NULL OR expires_at > $2)
                     LIMIT 1",
                    &[&key_hash, &now],
                )
                .await
                .map_err(pg_err)?;
            let Some(key_row) = row else {
                return Ok(None);
            };
            let key_id: i64 = key_row.get("id");
            let _ = client
                .execute(
                    "UPDATE api_keys SET last_used_at = $2 WHERE id = $1",
                    &[&key_id, &now],
                )
                .await;
            let scope_rows = client
                .query(
                    "SELECT scope FROM api_key_scopes WHERE api_key_id = $1 ORDER BY scope ASC",
                    &[&key_id],
                )
                .await
                .map_err(pg_err)?;
            let scopes: Vec<String> = scope_rows.iter().map(|r| r.get("scope")).collect();
            Ok(Some((key_id, scopes)))
        })
    }
}
