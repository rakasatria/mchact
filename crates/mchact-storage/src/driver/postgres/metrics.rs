use mchact_core::error::MchactError;

use crate::db::types::{LlmModelUsageSummary, LlmUsageSummary, MetricsHistoryPoint};
use crate::traits::MetricsStore;

use super::{not_impl, PgDriver};

impl MetricsStore for PgDriver {
    fn upsert_metrics_history(&self, _point: &MetricsHistoryPoint) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn get_metrics_history(
        &self,
        _since_ts_ms: i64,
        _limit: usize,
    ) -> Result<Vec<MetricsHistoryPoint>, MchactError> {
        Err(not_impl())
    }

    fn cleanup_metrics_history_before(&self, _before_ts_ms: i64) -> Result<usize, MchactError> {
        Err(not_impl())
    }

    fn log_llm_usage(
        &self,
        _chat_id: i64,
        _caller_channel: &str,
        _provider: &str,
        _model: &str,
        _input_tokens: i64,
        _output_tokens: i64,
        _request_kind: &str,
    ) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn get_llm_usage_summary(
        &self,
        _chat_id: Option<i64>,
    ) -> Result<LlmUsageSummary, MchactError> {
        Err(not_impl())
    }

    fn get_llm_usage_summary_since(
        &self,
        _chat_id: Option<i64>,
        _since: Option<&str>,
    ) -> Result<LlmUsageSummary, MchactError> {
        Err(not_impl())
    }

    fn get_llm_usage_by_model(
        &self,
        _chat_id: Option<i64>,
        _since: Option<&str>,
        _limit: Option<usize>,
    ) -> Result<Vec<LlmModelUsageSummary>, MchactError> {
        Err(not_impl())
    }
}
