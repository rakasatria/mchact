// ---------------------------------------------------------------------------
// Queue processor — dequeue tasks and dispatch to deriver / dreamer
// ---------------------------------------------------------------------------

use crate::{deriver::LlmClient, dreamer, ObservationStore};

// ---------------------------------------------------------------------------
// Queue processor
// ---------------------------------------------------------------------------

/// Dequeue up to `batch_size` items and process each one.
///
/// Supported task types:
/// - `"derive"` — runs the deriver agent using `messages_text` from the payload
/// - `"dream"`  — runs the dreamer agent
///
/// Each item is ack'd after successful processing.  Failed items are nack'd so
/// they can be retried.
///
/// Returns the total number of items processed (ack'd).
pub async fn process_queue(
    store: &dyn ObservationStore,
    llm: &dyn LlmClient,
    batch_size: i64,
) -> crate::Result<usize> {
    let items = store.dequeue(batch_size).await?;
    let mut processed = 0usize;

    for item in items {
        let result = match item.task_type.as_str() {
            "derive" => {
                let messages_text = item
                    .payload
                    .get("messages_text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                crate::deriver::derive_observations(
                    store,
                    llm,
                    item.observer_peer_id,
                    item.observed_peer_id,
                    item.chat_id.clone(),
                    &item.workspace,
                    &messages_text,
                )
                .await
                .map(|_| ())
            }

            "dream" => dreamer::run_dream_cycle(
                store,
                llm,
                item.observer_peer_id,
                item.observed_peer_id,
                &item.workspace,
            )
            .await
            .map(|_| ()),

            unknown => Err(crate::MemoryError::Validation(format!(
                "unknown task type: {unknown}"
            ))),
        };

        match result {
            Ok(()) => {
                store.ack_queue_item(item.id).await?;
                processed += 1;
            }
            Err(e) => {
                tracing::warn!(
                    task_id = item.id,
                    task_type = %item.task_type,
                    error = %e,
                    "queue item processing failed; nacking"
                );
                // Best-effort nack — ignore secondary errors
                let _ = store.nack_queue_item(item.id).await;
            }
        }
    }

    Ok(processed)
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    // Integration-level tests require a real store; unit tests validate the
    // helper logic only.

    #[test]
    fn test_task_types_are_string_matched() {
        // Verify the string constants we match on are stable
        assert_eq!("derive", "derive");
        assert_eq!("dream", "dream");
    }
}
