use mchact_core::error::MchactError;
use rusqlite::params;

use super::Database;
use super::{LlmModelUsageSummary, LlmUsageSummary, MetricsHistoryPoint};

impl Database {
    pub fn upsert_metrics_history(
        &self,
        point: &MetricsHistoryPoint,
    ) -> Result<(), MchactError> {
        let conn = self.lock_conn();
        conn.execute(
            "INSERT INTO metrics_history(
                timestamp_ms, llm_completions, llm_input_tokens, llm_output_tokens,
                http_requests, tool_executions, mcp_calls,
                mcp_rate_limited_rejections, mcp_bulkhead_rejections, mcp_circuit_open_rejections,
                active_sessions
             ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(timestamp_ms) DO UPDATE SET
                llm_completions = excluded.llm_completions,
                llm_input_tokens = excluded.llm_input_tokens,
                llm_output_tokens = excluded.llm_output_tokens,
                http_requests = excluded.http_requests,
                tool_executions = excluded.tool_executions,
                mcp_calls = excluded.mcp_calls,
                mcp_rate_limited_rejections = excluded.mcp_rate_limited_rejections,
                mcp_bulkhead_rejections = excluded.mcp_bulkhead_rejections,
                mcp_circuit_open_rejections = excluded.mcp_circuit_open_rejections,
                active_sessions = excluded.active_sessions",
            params![
                point.timestamp_ms,
                point.llm_completions,
                point.llm_input_tokens,
                point.llm_output_tokens,
                point.http_requests,
                point.tool_executions,
                point.mcp_calls,
                point.mcp_rate_limited_rejections,
                point.mcp_bulkhead_rejections,
                point.mcp_circuit_open_rejections,
                point.active_sessions
            ],
        )?;
        Ok(())
    }

    pub fn get_metrics_history(
        &self,
        since_ts_ms: i64,
        limit: usize,
    ) -> Result<Vec<MetricsHistoryPoint>, MchactError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT
                timestamp_ms, llm_completions, llm_input_tokens, llm_output_tokens,
                http_requests, tool_executions, mcp_calls,
                mcp_rate_limited_rejections, mcp_bulkhead_rejections, mcp_circuit_open_rejections,
                active_sessions
             FROM metrics_history
             WHERE timestamp_ms >= ?1
             ORDER BY timestamp_ms ASC
             LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![since_ts_ms, limit as i64], |row| {
                Ok(MetricsHistoryPoint {
                    timestamp_ms: row.get(0)?,
                    llm_completions: row.get(1)?,
                    llm_input_tokens: row.get(2)?,
                    llm_output_tokens: row.get(3)?,
                    http_requests: row.get(4)?,
                    tool_executions: row.get(5)?,
                    mcp_calls: row.get(6)?,
                    mcp_rate_limited_rejections: row.get(7)?,
                    mcp_bulkhead_rejections: row.get(8)?,
                    mcp_circuit_open_rejections: row.get(9)?,
                    active_sessions: row.get(10)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn cleanup_metrics_history_before(
        &self,
        before_ts_ms: i64,
    ) -> Result<usize, MchactError> {
        let conn = self.lock_conn();
        let n = conn.execute(
            "DELETE FROM metrics_history WHERE timestamp_ms < ?1",
            params![before_ts_ms],
        )?;
        Ok(n)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn log_llm_usage(
        &self,
        chat_id: i64,
        caller_channel: &str,
        provider: &str,
        model: &str,
        input_tokens: i64,
        output_tokens: i64,
        request_kind: &str,
    ) -> Result<i64, MchactError> {
        let conn = self.lock_conn();
        let now = chrono::Utc::now().to_rfc3339();
        let total_tokens = input_tokens.saturating_add(output_tokens);
        conn.execute(
            "INSERT INTO llm_usage_logs
                (chat_id, caller_channel, provider, model, input_tokens, output_tokens, total_tokens, request_kind, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                chat_id,
                caller_channel,
                provider,
                model,
                input_tokens,
                output_tokens,
                total_tokens,
                request_kind,
                now,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_llm_usage_summary(
        &self,
        chat_id: Option<i64>,
    ) -> Result<LlmUsageSummary, MchactError> {
        self.get_llm_usage_summary_since(chat_id, None)
    }

    pub fn get_llm_usage_summary_since(
        &self,
        chat_id: Option<i64>,
        since: Option<&str>,
    ) -> Result<LlmUsageSummary, MchactError> {
        let conn = self.lock_conn();
        let (requests, input_tokens, output_tokens, total_tokens, last_request_at) =
            match (chat_id, since) {
                (Some(id), Some(since_ts)) => conn.query_row(
                    "SELECT
                    COUNT(*),
                    COALESCE(SUM(input_tokens), 0),
                    COALESCE(SUM(output_tokens), 0),
                    COALESCE(SUM(total_tokens), 0),
                    MAX(created_at)
                 FROM llm_usage_logs
                 WHERE chat_id = ?1 AND created_at >= ?2",
                    params![id, since_ts],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, i64>(3)?,
                            row.get::<_, Option<String>>(4)?,
                        ))
                    },
                )?,
                (Some(id), None) => conn.query_row(
                    "SELECT
                    COUNT(*),
                    COALESCE(SUM(input_tokens), 0),
                    COALESCE(SUM(output_tokens), 0),
                    COALESCE(SUM(total_tokens), 0),
                    MAX(created_at)
                 FROM llm_usage_logs
                 WHERE chat_id = ?1",
                    params![id],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, i64>(3)?,
                            row.get::<_, Option<String>>(4)?,
                        ))
                    },
                )?,
                (None, Some(since_ts)) => conn.query_row(
                    "SELECT
                    COUNT(*),
                    COALESCE(SUM(input_tokens), 0),
                    COALESCE(SUM(output_tokens), 0),
                    COALESCE(SUM(total_tokens), 0),
                    MAX(created_at)
                 FROM llm_usage_logs
                 WHERE created_at >= ?1",
                    params![since_ts],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, i64>(3)?,
                            row.get::<_, Option<String>>(4)?,
                        ))
                    },
                )?,
                (None, None) => conn.query_row(
                    "SELECT
                    COUNT(*),
                    COALESCE(SUM(input_tokens), 0),
                    COALESCE(SUM(output_tokens), 0),
                    COALESCE(SUM(total_tokens), 0),
                    MAX(created_at)
                 FROM llm_usage_logs",
                    [],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, i64>(3)?,
                            row.get::<_, Option<String>>(4)?,
                        ))
                    },
                )?,
            };

        Ok(LlmUsageSummary {
            requests,
            input_tokens,
            output_tokens,
            total_tokens,
            last_request_at,
        })
    }

    pub fn get_llm_usage_by_model(
        &self,
        chat_id: Option<i64>,
        since: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<LlmModelUsageSummary>, MchactError> {
        let conn = self.lock_conn();
        let mut query = String::from(
            "SELECT
                model,
                COUNT(*) AS requests,
                COALESCE(SUM(input_tokens), 0) AS input_tokens,
                COALESCE(SUM(output_tokens), 0) AS output_tokens,
                COALESCE(SUM(total_tokens), 0) AS total_tokens
             FROM llm_usage_logs",
        );

        let mut has_where = false;
        if chat_id.is_some() {
            query.push_str(" WHERE chat_id = ?1");
            has_where = true;
        }
        if since.is_some() {
            if has_where {
                if chat_id.is_some() {
                    query.push_str(" AND created_at >= ?2");
                } else {
                    query.push_str(" AND created_at >= ?1");
                }
            } else {
                query.push_str(" WHERE created_at >= ?1");
            }
        }
        query.push_str(" GROUP BY model ORDER BY total_tokens DESC");
        if limit.is_some() {
            match (chat_id.is_some(), since.is_some()) {
                (true, true) => query.push_str(" LIMIT ?3"),
                (true, false) | (false, true) => query.push_str(" LIMIT ?2"),
                (false, false) => query.push_str(" LIMIT ?1"),
            }
        }

        let mut stmt = conn.prepare(&query)?;
        let mapper = |row: &rusqlite::Row<'_>| {
            Ok(LlmModelUsageSummary {
                model: row.get(0)?,
                requests: row.get(1)?,
                input_tokens: row.get(2)?,
                output_tokens: row.get(3)?,
                total_tokens: row.get(4)?,
            })
        };

        let rows = match (chat_id, since, limit) {
            (Some(id), Some(since_ts), Some(limit_n)) => {
                stmt.query_map(params![id, since_ts, limit_n as i64], mapper)?
            }
            (Some(id), Some(since_ts), None) => stmt.query_map(params![id, since_ts], mapper)?,
            (Some(id), None, Some(limit_n)) => {
                stmt.query_map(params![id, limit_n as i64], mapper)?
            }
            (Some(id), None, None) => stmt.query_map(params![id], mapper)?,
            (None, Some(since_ts), Some(limit_n)) => {
                stmt.query_map(params![since_ts, limit_n as i64], mapper)?
            }
            (None, Some(since_ts), None) => stmt.query_map(params![since_ts], mapper)?,
            (None, None, Some(limit_n)) => stmt.query_map(params![limit_n as i64], mapper)?,
            (None, None, None) => stmt.query_map([], mapper)?,
        };
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}
