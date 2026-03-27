#[cfg(feature = "sqlite")]
mod tests {
    use mchact_memory::{ObservationStore, Result};
    use mchact_memory::driver::sqlite::SqliteDriver;
    use mchact_memory::types::{
        NewObservation, ObservationLevel, ObservationUpdate, PeerKind, SearchScope,
    };

    fn make_driver() -> SqliteDriver {
        SqliteDriver::open_in_memory().expect("failed to open in-memory sqlite")
    }

    // -----------------------------------------------------------------------
    // Peer tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_peer_create_and_get() -> Result<()> {
        let store = make_driver();

        let peer = store
            .upsert_peer("ws1", "alice", PeerKind::User, None)
            .await?;

        assert!(peer.id > 0);
        assert_eq!(peer.workspace, "ws1");
        assert_eq!(peer.name, "alice");
        assert_eq!(peer.kind, PeerKind::User);

        // Get by name
        let fetched = store.get_peer_by_name("ws1", "alice").await?;
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.id, peer.id);
        assert_eq!(fetched.name, "alice");

        // Get by id
        let by_id = store.get_peer_by_id(peer.id).await?;
        assert!(by_id.is_some());
        assert_eq!(by_id.unwrap().name, "alice");

        // Idempotent second call
        let peer2 = store
            .upsert_peer("ws1", "alice", PeerKind::User, None)
            .await?;
        assert_eq!(peer2.id, peer.id, "upsert should return same id");

        Ok(())
    }

    #[tokio::test]
    async fn test_peer_list() -> Result<()> {
        let store = make_driver();

        store.upsert_peer("ws2", "bob", PeerKind::User, None).await?;
        store.upsert_peer("ws2", "carol", PeerKind::Agent, None).await?;
        store.upsert_peer("other_ws", "dave", PeerKind::User, None).await?;

        let peers = store.list_peers("ws2").await?;
        assert_eq!(peers.len(), 2);

        let names: Vec<&str> = peers.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"bob"));
        assert!(names.contains(&"carol"));

        Ok(())
    }

    #[tokio::test]
    async fn test_peer_card_update() -> Result<()> {
        let store = make_driver();

        let peer = store
            .upsert_peer("ws3", "eve", PeerKind::User, None)
            .await?;

        let facts = vec![
            "Likes Rust".to_string(),
            "Works remotely".to_string(),
        ];

        let card = store.update_peer_card(peer.id, facts.clone()).await?;
        assert_eq!(card.peer_id, peer.id);
        assert_eq!(card.peer_name, "eve");
        assert_eq!(card.facts, facts);

        // Get card back
        let fetched_card = store.get_peer_card(peer.id).await?;
        assert!(fetched_card.is_some());
        let fetched_card = fetched_card.unwrap();
        assert_eq!(fetched_card.facts, facts);

        // Verify peer_card field is also populated
        let updated_peer = store.get_peer_by_id(peer.id).await?.unwrap();
        assert_eq!(updated_peer.peer_card, Some(facts));

        Ok(())
    }

    #[tokio::test]
    async fn test_get_peer_card_no_card() -> Result<()> {
        let store = make_driver();
        let peer = store.upsert_peer("ws4", "frank", PeerKind::User, None).await?;

        let card = store.get_peer_card(peer.id).await?;
        assert!(card.is_none(), "new peer should have no card");

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Observation tests
    // -----------------------------------------------------------------------

    fn make_new_obs(
        workspace: &str,
        observer_id: i64,
        observed_id: i64,
        content: &str,
    ) -> NewObservation {
        NewObservation {
            workspace: workspace.to_string(),
            observer_peer_id: observer_id,
            observed_peer_id: observed_id,
            chat_id: None,
            level: ObservationLevel::Explicit,
            content: content.to_string(),
            category: Some("test".to_string()),
            confidence: 0.9,
            source: Some("test".to_string()),
            source_ids: vec![],
            message_ids: vec![1, 2, 3],
        }
    }

    #[tokio::test]
    async fn test_store_and_get_observation() -> Result<()> {
        let store = make_driver();

        let observer = store.upsert_peer("ws5", "obs_agent", PeerKind::Agent, None).await?;
        let observed = store.upsert_peer("ws5", "user1", PeerKind::User, None).await?;

        let new_obs = make_new_obs("ws5", observer.id, observed.id, "user likes cats");
        let created = store.create_observation(new_obs).await?;

        assert!(created.id > 0);
        assert_eq!(created.workspace, "ws5");
        assert_eq!(created.observer_peer_id, observer.id);
        assert_eq!(created.observed_peer_id, observed.id);
        assert_eq!(created.content, "user likes cats");
        assert_eq!(created.level, ObservationLevel::Explicit);
        assert!((created.confidence - 0.9).abs() < 1e-9);
        assert_eq!(created.message_ids, vec![1i64, 2, 3]);
        assert!(!created.is_archived);
        assert!(created.archived_at.is_none());

        // Retrieve by id
        let fetched = store.get_observation(created.id).await?;
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.content, "user likes cats");

        // Non-existent id returns None
        let missing = store.get_observation(999_999).await?;
        assert!(missing.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_archive_observation() -> Result<()> {
        let store = make_driver();

        let observer = store.upsert_peer("ws6", "agent", PeerKind::Agent, None).await?;
        let observed = store.upsert_peer("ws6", "user", PeerKind::User, None).await?;

        let new_obs = make_new_obs("ws6", observer.id, observed.id, "user hates mondays");
        let created = store.create_observation(new_obs).await?;
        assert!(!created.is_archived);

        let archived = store.archive_observation(created.id).await?;
        assert!(archived.is_archived);
        assert!(archived.archived_at.is_some());

        // Verify persisted
        let fetched = store.get_observation(created.id).await?.unwrap();
        assert!(fetched.is_archived);
        assert!(fetched.archived_at.is_some());

        // list_observations by default excludes archived
        let scope = SearchScope {
            workspace: "ws6".to_string(),
            ..Default::default()
        };
        let visible = store.list_observations(scope, 100, 0).await?;
        assert!(visible.iter().all(|o| !o.is_archived));

        Ok(())
    }

    #[tokio::test]
    async fn test_update_observation() -> Result<()> {
        let store = make_driver();

        let observer = store.upsert_peer("ws7", "agent", PeerKind::Agent, None).await?;
        let observed = store.upsert_peer("ws7", "user", PeerKind::User, None).await?;

        let new_obs = make_new_obs("ws7", observer.id, observed.id, "original content");
        let created = store.create_observation(new_obs).await?;

        let update = ObservationUpdate {
            content: Some("updated content".to_string()),
            confidence: Some(0.5),
            ..Default::default()
        };

        let updated = store.update_observation(created.id, update).await?;
        assert_eq!(updated.content, "updated content");
        assert!((updated.confidence - 0.5).abs() < 1e-9);
        assert_eq!(updated.level, ObservationLevel::Explicit, "level unchanged");

        // Verify persisted
        let fetched = store.get_observation(created.id).await?.unwrap();
        assert_eq!(fetched.content, "updated content");
        assert!((fetched.confidence - 0.5).abs() < 1e-9);

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_observation() -> Result<()> {
        let store = make_driver();

        let observer = store.upsert_peer("ws8", "agent", PeerKind::Agent, None).await?;
        let observed = store.upsert_peer("ws8", "user", PeerKind::User, None).await?;

        let new_obs = make_new_obs("ws8", observer.id, observed.id, "to be deleted");
        let created = store.create_observation(new_obs).await?;

        store.delete_observation(created.id).await?;

        let fetched = store.get_observation(created.id).await?;
        assert!(fetched.is_none(), "deleted observation should not be found");

        Ok(())
    }

    #[tokio::test]
    async fn test_list_observations_scope_filter() -> Result<()> {
        let store = make_driver();

        let agent = store.upsert_peer("ws9", "agent", PeerKind::Agent, None).await?;
        let user_a = store.upsert_peer("ws9", "user_a", PeerKind::User, None).await?;
        let user_b = store.upsert_peer("ws9", "user_b", PeerKind::User, None).await?;

        store.create_observation(make_new_obs("ws9", agent.id, user_a.id, "obs about user_a")).await?;
        store.create_observation(make_new_obs("ws9", agent.id, user_b.id, "obs about user_b")).await?;

        // Filter by observed_peer_id
        let scope = SearchScope {
            workspace: "ws9".to_string(),
            observed_peer_id: Some(user_a.id),
            ..Default::default()
        };
        let results = store.list_observations(scope, 100, 0).await?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].observed_peer_id, user_a.id);

        Ok(())
    }
}
