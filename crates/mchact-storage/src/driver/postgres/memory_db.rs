use mchact_core::error::MchactError;

use crate::db::types::{Memory, MemoryInjectionLog, MemoryObservabilitySummary, MemoryReflectorRun};
use crate::traits::MemoryDbStore;

use super::{not_impl, PgDriver};

impl MemoryDbStore for PgDriver {
    fn insert_memory(
        &self,
        _chat_id: Option<i64>,
        _content: &str,
        _category: &str,
    ) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn insert_memory_with_metadata(
        &self,
        _chat_id: Option<i64>,
        _content: &str,
        _category: &str,
        _source: &str,
        _confidence: f64,
    ) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn get_memory_by_id(&self, _id: i64) -> Result<Option<Memory>, MchactError> {
        Err(not_impl())
    }

    fn get_memories_for_context(
        &self,
        _chat_id: i64,
        _limit: usize,
    ) -> Result<Vec<Memory>, MchactError> {
        Err(not_impl())
    }

    fn get_all_memories_for_chat(
        &self,
        _chat_id: Option<i64>,
    ) -> Result<Vec<Memory>, MchactError> {
        Err(not_impl())
    }

    fn get_active_chat_ids_since(&self, _since: &str) -> Result<Vec<i64>, MchactError> {
        Err(not_impl())
    }

    fn delete_memory(&self, _id: i64) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn search_memories(
        &self,
        _chat_id: i64,
        _query: &str,
        _limit: usize,
    ) -> Result<Vec<Memory>, MchactError> {
        Err(not_impl())
    }

    fn search_memories_with_options(
        &self,
        _chat_id: i64,
        _query: &str,
        _limit: usize,
        _include_archived: bool,
        _broad_recall: bool,
    ) -> Result<Vec<Memory>, MchactError> {
        Err(not_impl())
    }

    fn update_memory_content(
        &self,
        _id: i64,
        _content: &str,
        _category: &str,
    ) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn update_memory_with_metadata(
        &self,
        _id: i64,
        _content: &str,
        _category: &str,
        _confidence: f64,
        _source: &str,
    ) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn update_memory_embedding_model(&self, _id: i64, _model: &str) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn touch_memory_last_seen(
        &self,
        _id: i64,
        _confidence_floor: Option<f64>,
    ) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn archive_memory(&self, _id: i64) -> Result<bool, MchactError> {
        Err(not_impl())
    }

    fn archive_stale_memories(&self, _stale_days: i64) -> Result<usize, MchactError> {
        Err(not_impl())
    }

    fn supersede_memory(
        &self,
        _from_memory_id: i64,
        _new_content: &str,
        _category: &str,
        _source: &str,
        _confidence: f64,
        _reason: Option<&str>,
    ) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn get_memories_without_embedding(
        &self,
        _chat_id: Option<i64>,
        _limit: usize,
    ) -> Result<Vec<Memory>, MchactError> {
        Err(not_impl())
    }

    fn get_all_active_memories(&self) -> Result<Vec<(i64, String)>, MchactError> {
        Err(not_impl())
    }

    fn get_reflector_cursor(&self, _chat_id: i64) -> Result<Option<String>, MchactError> {
        Err(not_impl())
    }

    fn set_reflector_cursor(&self, _chat_id: i64, _last_reflected_ts: &str) -> Result<(), MchactError> {
        Err(not_impl())
    }

    fn log_reflector_run(
        &self,
        _chat_id: i64,
        _started_at: &str,
        _finished_at: &str,
        _extracted_count: usize,
        _inserted_count: usize,
        _updated_count: usize,
        _skipped_count: usize,
        _dedup_method: &str,
        _parse_ok: bool,
        _error_text: Option<&str>,
    ) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn log_memory_injection(
        &self,
        _chat_id: i64,
        _retrieval_method: &str,
        _candidate_count: usize,
        _selected_count: usize,
        _omitted_count: usize,
        _tokens_est: usize,
    ) -> Result<i64, MchactError> {
        Err(not_impl())
    }

    fn get_memory_observability_summary(
        &self,
        _chat_id: Option<i64>,
    ) -> Result<MemoryObservabilitySummary, MchactError> {
        Err(not_impl())
    }

    fn get_memory_reflector_runs(
        &self,
        _chat_id: Option<i64>,
        _since: Option<&str>,
        _limit: usize,
        _offset: usize,
    ) -> Result<Vec<MemoryReflectorRun>, MchactError> {
        Err(not_impl())
    }

    fn get_memory_injection_logs(
        &self,
        _chat_id: Option<i64>,
        _since: Option<&str>,
        _limit: usize,
        _offset: usize,
    ) -> Result<Vec<MemoryInjectionLog>, MchactError> {
        Err(not_impl())
    }
}
