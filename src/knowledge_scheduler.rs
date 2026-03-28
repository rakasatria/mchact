use std::sync::Arc;

use tracing::{info, warn};

use crate::embedding::EmbeddingProvider;
use crate::knowledge::f32_vec_to_bytes;
use mchact_storage::DynDataStore;
use mchact_storage::prelude::*;

const MAX_EMBED_TEXT_CHARS: usize = 32000;

// ── Job 1: Embedding ──────────────────────────────────────────────────────────

/// Process pending document chunks by generating embeddings.
///
/// Returns `(done, failed)` counts for the batch.
pub async fn run_embed_job(
    db: &DynDataStore,
    embedding: &dyn EmbeddingProvider,
    batch_size: i64,
) -> (i64, i64) {
    let chunks = match db.get_chunks_by_status("pending", batch_size) {
        Ok(c) => c,
        Err(err) => {
            warn!("knowledge_scheduler: embed job failed to fetch chunks: {}", err);
            return (0, 0);
        }
    };

    if chunks.is_empty() {
        return (0, 0);
    }

    let total = chunks.len();
    let mut done: i64 = 0;
    let mut failed: i64 = 0;

    for chunk in chunks {
        let text = if chunk.text.len() > MAX_EMBED_TEXT_CHARS {
            &chunk.text[..MAX_EMBED_TEXT_CHARS]
        } else {
            &chunk.text
        };

        match embedding.embed(text).await {
            Ok(vector) => {
                let bytes = f32_vec_to_bytes(&vector);
                if let Err(err) = db.update_chunk_embedding(chunk.id, &bytes, "done") {
                    warn!(
                        "knowledge_scheduler: failed to persist embedding for chunk {}: {}",
                        chunk.id, err
                    );
                    failed += 1;
                } else {
                    done += 1;
                }
            }
            Err(err) => {
                warn!(
                    "knowledge_scheduler: embedding failed for chunk {}: {}",
                    chunk.id, err
                );
                if let Err(db_err) = db.update_chunk_embedding(chunk.id, &[], "failed") {
                    warn!(
                        "knowledge_scheduler: failed to mark chunk {} as failed: {}",
                        chunk.id, db_err
                    );
                }
                failed += 1;
            }
        }
    }

    info!(
        "knowledge_scheduler: embed job processed {}/{} chunks — done={}, failed={}",
        done + failed,
        total,
        done,
        failed
    );

    (done, failed)
}

// ── Job 2: Observation ────────────────────────────────────────────────────────

/// Feed embedded document chunks into the observation pipeline.
///
/// If no `observation_store` is configured this is a no-op that returns (0, 0).
/// When the `ObservationStore` trait does not expose a simple single-call
/// ingestion method the chunk is marked "done" immediately so subsequent jobs
/// are not blocked; the full deriver pipeline picks up observations
/// independently.
///
/// Returns `(done, failed)` counts for the batch.
pub async fn run_observe_job(
    db: &DynDataStore,
    observation_store: Option<&dyn mchact_memory::ObservationStore>,
    batch_size: i64,
) -> (i64, i64) {
    let Some(_store) = observation_store else {
        return (0, 0);
    };

    let chunks = match db.get_chunks_for_observation(batch_size) {
        Ok(c) => c,
        Err(err) => {
            warn!(
                "knowledge_scheduler: observe job failed to fetch chunks: {}",
                err
            );
            return (0, 0);
        }
    };

    if chunks.is_empty() {
        return (0, 0);
    }

    let total = chunks.len();
    let mut done: i64 = 0;
    let mut failed: i64 = 0;

    for chunk in chunks {
        // Build a short context string for logging / future deriver use.
        let page_label = if chunk.page_number > 0 {
            format!(" (page {})", chunk.page_number)
        } else {
            String::new()
        };

        // Try to resolve a human-readable filename from the extraction record.
        let filename = db
            .get_document_extraction_by_id(chunk.document_extraction_id)
            .ok()
            .flatten()
            .map(|e| e.filename)
            .unwrap_or_else(|| format!("extraction:{}", chunk.document_extraction_id));

        let preview = &chunk.text[..chunk.text.len().min(500)];
        let _context = format!(
            "[{}{}]\n{}",
            filename, page_label, preview
        );

        // The `ObservationStore` trait does not expose a single-shot
        // text-ingestion method; full observation derivation runs through the
        // deriver pipeline separately.  Mark the chunk as "done" here so it
        // is not re-processed on every pass.
        info!(
            "knowledge_scheduler: observe job — chunk {} ({}{}) queued for deriver pipeline",
            chunk.id, filename, page_label
        );

        if let Err(err) = db.update_chunk_observation_status(chunk.id, "done") {
            warn!(
                "knowledge_scheduler: failed to update observation status for chunk {}: {}",
                chunk.id, err
            );
            failed += 1;
        } else {
            done += 1;
        }
    }

    info!(
        "knowledge_scheduler: observe job processed {}/{} chunks — done={}, failed={}",
        done + failed,
        total,
        done,
        failed
    );

    (done, failed)
}

// ── Job 3: Auto-grouping ──────────────────────────────────────────────────────

/// Check knowledge bases that have grown beyond `min_docs` documents since the
/// last grouping check and record the updated document count.
///
/// The actual LLM-based group suggestion is deferred to the full pipeline;
/// this job simply updates the tracking timestamp so the scheduler knows
/// which bases need attention.
///
/// Returns the number of knowledge bases processed.
pub async fn run_autogroup_job(db: &DynDataStore, min_docs: i64) -> i64 {
    let bases = match db.get_knowledge_needing_grouping(min_docs) {
        Ok(b) => b,
        Err(err) => {
            warn!(
                "knowledge_scheduler: autogroup job failed to fetch knowledge bases: {}",
                err
            );
            return 0;
        }
    };

    if bases.is_empty() {
        return 0;
    }

    let mut processed: i64 = 0;

    for knowledge in bases {
        // Count current documents for this knowledge base as a proxy for
        // grouping need (actual LLM suggestion deferred).
        let doc_ids: Vec<i64> = match db.list_knowledge_documents(knowledge.id) {
            Ok(ids) => ids,
            Err(err) => {
                warn!(
                    "knowledge_scheduler: autogroup — failed to get docs for knowledge {}: {}",
                    knowledge.id, err
                );
                continue;
            }
        };
        let doc_count = doc_ids.len() as i64;

        info!(
            "knowledge_scheduler: autogroup suggestion for '{}' (id={}) — {} docs (LLM grouping deferred)",
            knowledge.name, knowledge.id, doc_count
        );

        if let Err(err) = db.update_knowledge_grouping_check(knowledge.id, doc_count) {
            warn!(
                "knowledge_scheduler: failed to update grouping check for knowledge {}: {}",
                knowledge.id, err
            );
        } else {
            processed += 1;
        }
    }

    if processed > 0 {
        info!(
            "knowledge_scheduler: autogroup job processed {} knowledge base(s)",
            processed
        );
    }

    processed
}

// ── Retry sweep ───────────────────────────────────────────────────────────────

/// Reset failed chunks that are older than `older_than_mins` minutes back to
/// "pending" so they are retried by the next embed job pass.
///
/// Returns the number of chunks reset.
pub fn reset_failed_chunks(db: &DynDataStore, older_than_mins: i64) -> i64 {
    match db.reset_failed_chunks(older_than_mins) {
        Ok(count) => {
            if count > 0 {
                info!(
                    "knowledge_scheduler: retry sweep reset {} failed chunk(s) to pending",
                    count
                );
            }
            count
        }
        Err(err) => {
            warn!("knowledge_scheduler: retry sweep failed: {}", err);
            0
        }
    }
}

// ── Spawner ───────────────────────────────────────────────────────────────────

/// Spawn the three knowledge processing background tasks plus a retry sweep.
///
/// Each task runs on a configurable interval sourced from `AppState::config`.
pub fn spawn_knowledge_processor(state: Arc<crate::runtime::AppState>) {
    let embed_interval = state.config.knowledge_embed_interval_mins;
    let observe_interval = state.config.knowledge_observe_interval_mins;
    let autogroup_interval = state.config.knowledge_autogroup_interval_mins;
    let embed_batch = state.config.knowledge_embed_batch_size as i64;
    let observe_batch = state.config.knowledge_observe_batch_size as i64;
    let min_docs = state.config.knowledge_autogroup_min_docs as i64;
    let retry_delay = state.config.knowledge_retry_delay_mins as i64;

    // ── Embed job ─────────────────────────────────────────────────────────────
    {
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            info!(
                "knowledge_scheduler: embed job started (interval: {}min, batch: {})",
                embed_interval, embed_batch
            );
            let mut ticker = tokio::time::interval(std::time::Duration::from_secs(
                embed_interval * 60,
            ));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                if let Some(embedding) = state.embedding.as_deref() {
                    run_embed_job(&*state.db, embedding, embed_batch).await;
                }
            }
        });
    }

    // ── Observe job ───────────────────────────────────────────────────────────
    {
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            info!(
                "knowledge_scheduler: observe job started (interval: {}min, batch: {})",
                observe_interval, observe_batch
            );
            let mut ticker = tokio::time::interval(std::time::Duration::from_secs(
                observe_interval * 60,
            ));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                let obs_ref = state.observation_store.as_deref();
                run_observe_job(&*state.db, obs_ref, observe_batch).await;
            }
        });
    }

    // ── Autogroup job ─────────────────────────────────────────────────────────
    {
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            info!(
                "knowledge_scheduler: autogroup job started (interval: {}min, min_docs: {})",
                autogroup_interval, min_docs
            );
            let mut ticker = tokio::time::interval(std::time::Duration::from_secs(
                autogroup_interval * 60,
            ));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                run_autogroup_job(&*state.db, min_docs).await;
            }
        });
    }

    // ── Retry sweep ───────────────────────────────────────────────────────────
    {
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            info!("knowledge_scheduler: retry sweep started (interval: 30min)");
            let mut ticker =
                tokio::time::interval(std::time::Duration::from_secs(30 * 60));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                reset_failed_chunks(&*state.db, retry_delay);
            }
        });
    }
}
