use mchact_core::error::MchactError;

use crate::db::types::{Memory, MemoryInjectionLog, MemoryObservabilitySummary, MemoryReflectorRun};

pub trait MemoryDbStore {
    fn insert_memory(
        &self,
        chat_id: Option<i64>,
        content: &str,
        category: &str,
    ) -> Result<i64, MchactError>;

    fn insert_memory_with_metadata(
        &self,
        chat_id: Option<i64>,
        content: &str,
        category: &str,
        source: &str,
        confidence: f64,
    ) -> Result<i64, MchactError>;

    fn get_memory_by_id(&self, id: i64) -> Result<Option<Memory>, MchactError>;

    fn get_memories_for_context(
        &self,
        chat_id: i64,
        limit: usize,
    ) -> Result<Vec<Memory>, MchactError>;

    fn get_all_memories_for_chat(
        &self,
        chat_id: Option<i64>,
    ) -> Result<Vec<Memory>, MchactError>;

    fn get_active_chat_ids_since(&self, since: &str) -> Result<Vec<i64>, MchactError>;

    fn delete_memory(&self, id: i64) -> Result<bool, MchactError>;

    fn search_memories(
        &self,
        chat_id: i64,
        query: &str,
        limit: usize,
    ) -> Result<Vec<Memory>, MchactError>;

    fn search_memories_with_options(
        &self,
        chat_id: i64,
        query: &str,
        limit: usize,
        include_archived: bool,
        broad_recall: bool,
    ) -> Result<Vec<Memory>, MchactError>;

    fn update_memory_content(
        &self,
        id: i64,
        content: &str,
        category: &str,
    ) -> Result<bool, MchactError>;

    fn update_memory_with_metadata(
        &self,
        id: i64,
        content: &str,
        category: &str,
        confidence: f64,
        source: &str,
    ) -> Result<bool, MchactError>;

    fn update_memory_embedding_model(
        &self,
        id: i64,
        model: &str,
    ) -> Result<bool, MchactError>;

    fn touch_memory_last_seen(
        &self,
        id: i64,
        confidence_floor: Option<f64>,
    ) -> Result<bool, MchactError>;

    fn archive_memory(&self, id: i64) -> Result<bool, MchactError>;

    fn archive_stale_memories(&self, stale_days: i64) -> Result<usize, MchactError>;

    fn supersede_memory(
        &self,
        from_memory_id: i64,
        new_content: &str,
        category: &str,
        source: &str,
        confidence: f64,
        reason: Option<&str>,
    ) -> Result<i64, MchactError>;

    fn get_memories_without_embedding(
        &self,
        chat_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<Memory>, MchactError>;

    #[cfg(feature = "vector-search")]
    fn prepare_vector_index(&self, dimension: usize) -> Result<(), MchactError>;

    #[cfg(feature = "vector-search")]
    fn upsert_memory_vec(
        &self,
        memory_id: i64,
        embedding: &[f32],
    ) -> Result<(), MchactError>;

    fn get_all_active_memories(&self) -> Result<Vec<(i64, String)>, MchactError>;

    #[cfg(feature = "vector-search")]
    fn knn_memories(
        &self,
        chat_id: i64,
        query_vec: &[f32],
        k: usize,
    ) -> Result<Vec<(i64, f32)>, MchactError>;

    fn get_reflector_cursor(&self, chat_id: i64) -> Result<Option<String>, MchactError>;

    fn set_reflector_cursor(
        &self,
        chat_id: i64,
        last_reflected_ts: &str,
    ) -> Result<(), MchactError>;

    fn log_reflector_run(
        &self,
        chat_id: i64,
        started_at: &str,
        finished_at: &str,
        extracted_count: usize,
        inserted_count: usize,
        updated_count: usize,
        skipped_count: usize,
        dedup_method: &str,
        parse_ok: bool,
        error_text: Option<&str>,
    ) -> Result<i64, MchactError>;

    fn log_memory_injection(
        &self,
        chat_id: i64,
        retrieval_method: &str,
        candidate_count: usize,
        selected_count: usize,
        omitted_count: usize,
        tokens_est: usize,
    ) -> Result<i64, MchactError>;

    fn get_memory_observability_summary(
        &self,
        chat_id: Option<i64>,
    ) -> Result<MemoryObservabilitySummary, MchactError>;

    fn get_memory_reflector_runs(
        &self,
        chat_id: Option<i64>,
        since: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<MemoryReflectorRun>, MchactError>;

    fn get_memory_injection_logs(
        &self,
        chat_id: Option<i64>,
        since: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<MemoryInjectionLog>, MchactError>;
}
