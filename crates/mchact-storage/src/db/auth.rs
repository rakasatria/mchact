use mchact_core::error::MchactError;
use rusqlite::params;
use rusqlite::OptionalExtension;

use super::Database;
use super::AuthApiKeyRecord;

impl Database {
    pub fn upsert_auth_password_hash(&self, password_hash: &str) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO auth_passwords(id, password_hash, created_at, updated_at)
             VALUES(1, ?1, ?2, ?2)
             ON CONFLICT(id) DO UPDATE SET
                password_hash = excluded.password_hash,
                updated_at = excluded.updated_at",
            params![password_hash, now],
        )?;
        Ok(())
    }

    pub fn get_auth_password_hash(&self) -> Result<Option<String>, MchactError> {
        let conn = self.lock_conn();
        let value = conn
            .query_row(
                "SELECT password_hash FROM auth_passwords WHERE id = 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(value)
    }

    pub fn clear_auth_password_hash(&self) -> Result<bool, MchactError> {
        let conn = self.lock_conn();
        let rows = conn.execute("DELETE FROM auth_passwords WHERE id = 1", [])?;
        Ok(rows > 0)
    }

    pub fn create_auth_session(
        &self,
        session_id: &str,
        label: Option<&str>,
        expires_at: &str,
    ) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO auth_sessions(session_id, label, created_at, expires_at, last_seen_at, revoked_at)
             VALUES(?1, ?2, ?3, ?4, ?3, NULL)",
            params![session_id, label, now, expires_at],
        )?;
        Ok(())
    }

    pub fn validate_auth_session(&self, session_id: &str) -> Result<bool, MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        let valid = conn
            .query_row(
                "SELECT 1
                 FROM auth_sessions
                 WHERE session_id = ?1
                   AND revoked_at IS NULL
                   AND expires_at > ?2
                 LIMIT 1",
                params![session_id, now],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if valid {
            let _ = conn.execute(
                "UPDATE auth_sessions SET last_seen_at = ?2 WHERE session_id = ?1",
                params![session_id, now],
            );
        }
        Ok(valid)
    }

    pub fn revoke_auth_session(&self, session_id: &str) -> Result<bool, MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        let rows = conn.execute(
            "UPDATE auth_sessions
             SET revoked_at = COALESCE(revoked_at, ?2)
             WHERE session_id = ?1",
            params![session_id, now],
        )?;
        Ok(rows > 0)
    }

    pub fn revoke_all_auth_sessions(&self) -> Result<usize, MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        let rows = conn.execute(
            "UPDATE auth_sessions
             SET revoked_at = COALESCE(revoked_at, ?1)
             WHERE revoked_at IS NULL",
            params![now],
        )?;
        Ok(rows)
    }

    pub fn create_api_key(
        &self,
        label: &str,
        key_hash: &str,
        prefix: &str,
        scopes: &[String],
        expires_at: Option<&str>,
        rotated_from_key_id: Option<i64>,
    ) -> Result<i64, MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO api_keys(label, key_hash, prefix, created_at, expires_at, rotated_from_key_id)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
            params![label, key_hash, prefix, now, expires_at, rotated_from_key_id],
        )?;
        let key_id = tx.last_insert_rowid();
        for scope in scopes {
            tx.execute(
                "INSERT OR IGNORE INTO api_key_scopes(api_key_id, scope) VALUES(?1, ?2)",
                params![key_id, scope],
            )?;
        }
        tx.commit()?;
        Ok(key_id)
    }

    pub fn list_api_keys(&self) -> Result<Vec<AuthApiKeyRecord>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, label, prefix, created_at, revoked_at, expires_at, last_used_at, rotated_from_key_id
             FROM api_keys
             ORDER BY id DESC",
        )?;
        let mut rows = stmt.query([])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let mut scopes_stmt = conn.prepare(
                "SELECT scope FROM api_key_scopes WHERE api_key_id = ?1 ORDER BY scope ASC",
            )?;
            let scopes = scopes_stmt
                .query_map(params![id], |r| r.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            out.push(AuthApiKeyRecord {
                id,
                label: row.get(1)?,
                prefix: row.get(2)?,
                created_at: row.get(3)?,
                revoked_at: row.get(4)?,
                expires_at: row.get(5)?,
                last_used_at: row.get(6)?,
                rotated_from_key_id: row.get(7)?,
                scopes,
            });
        }
        Ok(out)
    }

    pub fn rotate_api_key_revoke_old(&self, old_key_id: i64) -> Result<bool, MchactError> {
        self.revoke_api_key(old_key_id)
    }

    pub fn revoke_api_key(&self, key_id: i64) -> Result<bool, MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        let rows = conn.execute(
            "UPDATE api_keys
             SET revoked_at = COALESCE(revoked_at, ?2)
             WHERE id = ?1",
            params![key_id, now],
        )?;
        Ok(rows > 0)
    }

    pub fn validate_api_key_hash(
        &self,
        key_hash: &str,
    ) -> Result<Option<(i64, Vec<String>)>, MchactError> {
        let conn = self.lock_conn();
        let row = conn
            .query_row(
                "SELECT id FROM api_keys
                 WHERE key_hash = ?1
                   AND revoked_at IS NULL
                   AND (expires_at IS NULL OR expires_at > ?2)
                 LIMIT 1",
                params![key_hash, chrono::Utc::now().to_rfc3339()],
                |r| r.get::<_, i64>(0),
            )
            .optional()?;
        let Some(key_id) = row else {
            return Ok(None);
        };
        let now = chrono::Utc::now().to_rfc3339();
        let _ = conn.execute(
            "UPDATE api_keys SET last_used_at = ?2 WHERE id = ?1",
            params![key_id, now],
        );
        let mut stmt = conn
            .prepare("SELECT scope FROM api_key_scopes WHERE api_key_id = ?1 ORDER BY scope ASC")?;
        let scopes = stmt
            .query_map(params![key_id], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some((key_id, scopes)))
    }
}
