// ---------------------------------------------------------------------------
// Legacy migration — import old memory records into the observation store
// ---------------------------------------------------------------------------

use crate::{
    types::{NewObservation, ObservationLevel, PeerKind},
    ObservationStore,
};

// ---------------------------------------------------------------------------
// Legacy memory type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LegacyMemory {
    pub id: i64,
    pub chat_id: Option<String>,
    pub content: String,
    pub category: String,
    pub confidence: f64,
    pub source: String,
}

// ---------------------------------------------------------------------------
// Migration function
// ---------------------------------------------------------------------------

/// Migrate legacy memory records into the observation store.
///
/// For each memory a peer is upserted using `chat_id` (or "legacy_global" when
/// absent) as the peer name, and an `Explicit` observation is created owned
/// by `bot_peer_id` observing that peer.
///
/// Returns the number of observations successfully created.
pub async fn migrate_legacy_memories(
    store: &dyn ObservationStore,
    memories: Vec<LegacyMemory>,
    bot_peer_id: i64,
    workspace: &str,
) -> crate::Result<usize> {
    let mut created = 0usize;

    for memory in memories {
        // Skip empty content
        if memory.content.trim().is_empty() {
            continue;
        }

        // Derive a peer name from chat_id or fall back to a global sentinel
        let peer_name = memory
            .chat_id
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("legacy_global");

        // Upsert the peer so we have a stable id
        let peer = store
            .upsert_peer(workspace, peer_name, PeerKind::User, None)
            .await?;

        let new_obs = NewObservation {
            workspace: workspace.to_string(),
            observer_peer_id: bot_peer_id,
            observed_peer_id: peer.id,
            chat_id: memory.chat_id.clone(),
            level: ObservationLevel::Explicit,
            content: memory.content.clone(),
            category: Some(memory.category.clone()),
            confidence: memory.confidence,
            source: Some(memory.source.clone()),
            source_ids: vec![],
            message_ids: vec![],
        };

        store.create_observation(new_obs).await?;
        created += 1;
    }

    Ok(created)
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_legacy_memory_fields() {
        let m = LegacyMemory {
            id: 1,
            chat_id: Some("chat_abc".to_string()),
            content: "User prefers async Rust".to_string(),
            category: "preference".to_string(),
            confidence: 0.9,
            source: "v1_memory".to_string(),
        };
        assert_eq!(m.id, 1);
        assert_eq!(m.chat_id.as_deref(), Some("chat_abc"));
        assert_eq!(m.confidence, 0.9);
    }

    #[test]
    fn test_legacy_memory_no_chat_id() {
        let m = LegacyMemory {
            id: 2,
            chat_id: None,
            content: "Global preference noted".to_string(),
            category: "misc".to_string(),
            confidence: 0.7,
            source: "v1_memory".to_string(),
        };
        // Verify the fallback logic we'd apply
        let peer_name = m
            .chat_id
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("legacy_global");
        assert_eq!(peer_name, "legacy_global");
    }
}
