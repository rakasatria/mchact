use mchact_core::error::MchactError;

use crate::db::types::{LlmModelUsageSummary, LlmUsageSummary, MetricsHistoryPoint};
use crate::traits::MetricsStore;

use super::PgDriver;

fn pg_err(e: tokio_postgres::Error) -> MchactError {
    MchactError::ToolExecution(format!("postgres: {e}"))
}

fn pool_err(e: deadpool_postgres::PoolError) -> MchactError {
    MchactError::ToolExecution(format!("pool: {e}"))
}

impl MetricsStore for PgDriver {
    fn upsert_metrics_history(&self, point: &MetricsHistoryPoint) -> Result<(), MchactError> {
        let pool = self.pool.clone();
        let point = point.clone();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            client
                .execute(
                    "INSERT INTO metrics_history(
                        timestamp_ms, llm_completions, llm_input_tokens, llm_output_tokens,
                        http_requests, tool_executions, mcp_calls,
                        mcp_rate_limited_rejections, mcp_bulkhead_rejections, mcp_circuit_open_rejections,
                        active_sessions
                     ) VALUES($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                     ON CONFLICT(timestamp_ms) DO UPDATE SET
                        llm_completions = EXCLUDED.llm_completions,
                        llm_input_tokens = EXCLUDED.llm_input_tokens,
                        llm_output_tokens = EXCLUDED.llm_output_tokens,
                        http_requests = EXCLUDED.http_requests,
                        tool_executions = EXCLUDED.tool_executions,
                        mcp_calls = EXCLUDED.mcp_calls,
                        mcp_rate_limited_rejections = EXCLUDED.mcp_rate_limited_rejections,
                        mcp_bulkhead_rejections = EXCLUDED.mcp_bulkhead_rejections,
                        mcp_circuit_open_rejections = EXCLUDED.mcp_circuit_open_rejections,
                        active_sessions = EXCLUDED.active_sessions",
                    &[
                        &point.timestamp_ms,
                        &point.llm_completions,
                        &point.llm_input_tokens,
                        &point.llm_output_tokens,
                        &point.http_requests,
                        &point.tool_executions,
                        &point.mcp_calls,
                        &point.mcp_rate_limited_rejections,
                        &point.mcp_bulkhead_rejections,
                        &point.mcp_circuit_open_rejections,
                        &point.active_sessions,
                    ],
                )
                .await
                .map_err(pg_err)?;
            Ok(())
        })
    }

    fn get_metrics_history(
        &self,
        since_ts_ms: i64,
        limit: usize,
    ) -> Result<Vec<MetricsHistoryPoint>, MchactError> {
        let pool = self.pool.clone();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            let rows = client
                .query(
                    "SELECT
                        timestamp_ms, llm_completions, llm_input_tokens, llm_output_tokens,
                        http_requests, tool_executions, mcp_calls,
                        mcp_rate_limited_rejections, mcp_bulkhead_rejections, mcp_circuit_open_rejections,
                        active_sessions
                     FROM metrics_history
                     WHERE timestamp_ms >= $1
                     ORDER BY timestamp_ms ASC
                     LIMIT $2",
                    &[&since_ts_ms, &(limit as i64)],
                )
                .await
                .map_err(pg_err)?;
            let points = rows
                .iter()
                .map(|row| MetricsHistoryPoint {
                    timestamp_ms: row.get("timestamp_ms"),
                    llm_completions: row.get("llm_completions"),
                    llm_input_tokens: row.get("llm_input_tokens"),
                    llm_output_tokens: row.get("llm_output_tokens"),
                    http_requests: row.get("http_requests"),
                    tool_executions: row.get("tool_executions"),
                    mcp_calls: row.get("mcp_calls"),
                    mcp_rate_limited_rejections: row.get("mcp_rate_limited_rejections"),
                    mcp_bulkhead_rejections: row.get("mcp_bulkhead_rejections"),
                    mcp_circuit_open_rejections: row.get("mcp_circuit_open_rejections"),
                    active_sessions: row.get("active_sessions"),
                })
                .collect();
            Ok(points)
        })
    }

    fn cleanup_metrics_history_before(&self, before_ts_ms: i64) -> Result<usize, MchactError> {
        let pool = self.pool.clone();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            let n = client
                .execute(
                    "DELETE FROM metrics_history WHERE timestamp_ms < $1",
                    &[&before_ts_ms],
                )
                .await
                .map_err(pg_err)?;
            Ok(n as usize)
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn log_llm_usage(
        &self,
        chat_id: i64,
        caller_channel: &str,
        provider: &str,
        model: &str,
        input_tokens: i64,
        output_tokens: i64,
        request_kind: &str,
    ) -> Result<i64, MchactError> {
        let pool = self.pool.clone();
        let now = chrono::Utc::now().to_rfc3339();
        let total_tokens = input_tokens.saturating_add(output_tokens);
        let caller_channel = caller_channel.to_string();
        let provider = provider.to_string();
        let model = model.to_string();
        let request_kind = request_kind.to_string();
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            let row = client
                .query_one(
                    "INSERT INTO llm_usage_logs
                        (chat_id, caller_channel, provider, model, input_tokens, output_tokens,
                         total_tokens, request_kind, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                     RETURNING id",
                    &[
                        &chat_id,
                        &caller_channel,
                        &provider,
                        &model,
                        &input_tokens,
                        &output_tokens,
                        &total_tokens,
                        &request_kind,
                        &now,
                    ],
                )
                .await
                .map_err(pg_err)?;
            Ok(row.get::<_, i64>("id"))
        })
    }

    fn get_llm_usage_summary(
        &self,
        chat_id: Option<i64>,
    ) -> Result<LlmUsageSummary, MchactError> {
        self.get_llm_usage_summary_since(chat_id, None)
    }

    fn get_llm_usage_summary_since(
        &self,
        chat_id: Option<i64>,
        since: Option<&str>,
    ) -> Result<LlmUsageSummary, MchactError> {
        let pool = self.pool.clone();
        let since = since.map(|s| s.to_string());
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            let select = "SELECT
                COUNT(*),
                COALESCE(SUM(input_tokens), 0),
                COALESCE(SUM(output_tokens), 0),
                COALESCE(SUM(total_tokens), 0),
                MAX(created_at)
             FROM llm_usage_logs";
            let row = match (chat_id, since) {
                (Some(id), Some(since_ts)) => {
                    let q = format!("{select} WHERE chat_id = $1 AND created_at >= $2");
                    client.query_one(&q as &str, &[&id, &since_ts]).await.map_err(pg_err)?
                }
                (Some(id), None) => {
                    let q = format!("{select} WHERE chat_id = $1");
                    client.query_one(&q as &str, &[&id]).await.map_err(pg_err)?
                }
                (None, Some(since_ts)) => {
                    let q = format!("{select} WHERE created_at >= $1");
                    client.query_one(&q as &str, &[&since_ts]).await.map_err(pg_err)?
                }
                (None, None) => {
                    client.query_one(select, &[]).await.map_err(pg_err)?
                }
            };
            Ok(LlmUsageSummary {
                requests: row.get(0),
                input_tokens: row.get(1),
                output_tokens: row.get(2),
                total_tokens: row.get(3),
                last_request_at: row.get(4),
            })
        })
    }

    fn get_llm_usage_by_model(
        &self,
        chat_id: Option<i64>,
        since: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<LlmModelUsageSummary>, MchactError> {
        let pool = self.pool.clone();
        let since = since.map(|s| s.to_string());
        tokio::runtime::Handle::current().block_on(async move {
            let client = pool.get().await.map_err(pool_err)?;
            let base = "SELECT
                model,
                COUNT(*) AS requests,
                COALESCE(SUM(input_tokens), 0) AS input_tokens,
                COALESCE(SUM(output_tokens), 0) AS output_tokens,
                COALESCE(SUM(total_tokens), 0) AS total_tokens
             FROM llm_usage_logs";

            // Build dynamic query with positional params
            let mut conditions: Vec<String> = Vec::new();
            let mut param_idx = 1usize;
            let mut chat_id_param = 0usize;
            let mut since_param = 0usize;
            let mut limit_param = 0usize;

            if chat_id.is_some() {
                conditions.push(format!("chat_id = ${param_idx}"));
                chat_id_param = param_idx;
                param_idx += 1;
            }
            if since.is_some() {
                conditions.push(format!("created_at >= ${param_idx}"));
                since_param = param_idx;
                param_idx += 1;
            }

            let mut query = base.to_string();
            if !conditions.is_empty() {
                query.push_str(" WHERE ");
                query.push_str(&conditions.join(" AND "));
            }
            query.push_str(" GROUP BY model ORDER BY total_tokens DESC");
            if limit.is_some() {
                query.push_str(&format!(" LIMIT ${param_idx}"));
                limit_param = param_idx;
            }
            let _ = (chat_id_param, since_param, limit_param); // used via index below

            // Build params slice dynamically
            let chat_id_val: Option<i64> = chat_id;
            let since_val: Option<String> = since;
            let limit_val: Option<i64> = limit.map(|n| n as i64);

            let rows = match (chat_id_val, since_val.as_deref(), limit_val) {
                (Some(id), Some(s), Some(l)) => {
                    client.query(&query as &str, &[&id, &s, &l]).await.map_err(pg_err)?
                }
                (Some(id), Some(s), None) => {
                    client.query(&query as &str, &[&id, &s]).await.map_err(pg_err)?
                }
                (Some(id), None, Some(l)) => {
                    client.query(&query as &str, &[&id, &l]).await.map_err(pg_err)?
                }
                (Some(id), None, None) => {
                    client.query(&query as &str, &[&id]).await.map_err(pg_err)?
                }
                (None, Some(s), Some(l)) => {
                    client.query(&query as &str, &[&s, &l]).await.map_err(pg_err)?
                }
                (None, Some(s), None) => {
                    client.query(&query as &str, &[&s]).await.map_err(pg_err)?
                }
                (None, None, Some(l)) => {
                    client.query(&query as &str, &[&l]).await.map_err(pg_err)?
                }
                (None, None, None) => {
                    client.query(&query as &str, &[]).await.map_err(pg_err)?
                }
            };

            let summaries = rows
                .iter()
                .map(|row| LlmModelUsageSummary {
                    model: row.get("model"),
                    requests: row.get("requests"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                })
                .collect();
            Ok(summaries)
        })
    }
}
