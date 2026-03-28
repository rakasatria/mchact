use mchact_core::error::MchactError;

use crate::db::types::{LlmModelUsageSummary, LlmUsageSummary, MetricsHistoryPoint};

pub trait MetricsStore {
    fn upsert_metrics_history(
        &self,
        point: &MetricsHistoryPoint,
    ) -> Result<(), MchactError>;

    fn get_metrics_history(
        &self,
        since_ts_ms: i64,
        limit: usize,
    ) -> Result<Vec<MetricsHistoryPoint>, MchactError>;

    fn cleanup_metrics_history_before(
        &self,
        before_ts_ms: i64,
    ) -> Result<usize, MchactError>;

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
    ) -> Result<i64, MchactError>;

    fn get_llm_usage_summary(
        &self,
        chat_id: Option<i64>,
    ) -> Result<LlmUsageSummary, MchactError>;

    fn get_llm_usage_summary_since(
        &self,
        chat_id: Option<i64>,
        since: Option<&str>,
    ) -> Result<LlmUsageSummary, MchactError>;

    fn get_llm_usage_by_model(
        &self,
        chat_id: Option<i64>,
        since: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<LlmModelUsageSummary>, MchactError>;
}
