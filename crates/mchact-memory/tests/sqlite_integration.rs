#[cfg(feature = "sqlite")]
mod tests {
    use mchact_memory::{ObservationStore, Result};
    use mchact_memory::driver::sqlite::SqliteDriver;
    use mchact_memory::types::{
        DeriverRun, DreamerRun, Finding, InjectionLog, NewObservation, ObservationLevel,
        ObservationUpdate, PeerKind, QueueTask, SearchScope,
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

    // -----------------------------------------------------------------------
    // Queue tests (Task 5)
    // -----------------------------------------------------------------------

    fn make_queue_task(workspace: &str, observer_id: i64, observed_id: i64) -> QueueTask {
        QueueTask {
            task_type: "derive".to_string(),
            workspace: workspace.to_string(),
            chat_id: Some("chat-1".to_string()),
            observer_peer_id: observer_id,
            observed_peer_id: observed_id,
            payload: serde_json::json!({"messages": 5}),
        }
    }

    #[tokio::test]
    async fn test_enqueue_and_dequeue() -> Result<()> {
        let store = make_driver();

        let agent = store.upsert_peer("wsQ1", "agent", PeerKind::Agent, None).await?;
        let user = store.upsert_peer("wsQ1", "user", PeerKind::User, None).await?;

        let task = make_queue_task("wsQ1", agent.id, user.id);
        let item = store.enqueue(task).await?;

        assert!(item.id > 0);
        assert_eq!(item.task_type, "derive");
        assert_eq!(item.workspace, "wsQ1");
        assert_eq!(item.chat_id, Some("chat-1".to_string()));

        // dequeue should return the item
        let items = store.dequeue(10).await?;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, item.id);

        Ok(())
    }

    #[tokio::test]
    async fn test_ack_removes_from_dequeue() -> Result<()> {
        let store = make_driver();

        let agent = store.upsert_peer("wsQ2", "agent", PeerKind::Agent, None).await?;
        let user = store.upsert_peer("wsQ2", "user", PeerKind::User, None).await?;

        let item = store.enqueue(make_queue_task("wsQ2", agent.id, user.id)).await?;

        store.ack_queue_item(item.id).await?;

        // After ack, dequeue should return nothing.
        let items = store.dequeue(10).await?;
        assert!(items.is_empty(), "acked item should not appear in dequeue");

        Ok(())
    }

    #[tokio::test]
    async fn test_nack_requeues_item() -> Result<()> {
        let store = make_driver();

        let agent = store.upsert_peer("wsQ3", "agent", PeerKind::Agent, None).await?;
        let user = store.upsert_peer("wsQ3", "user", PeerKind::User, None).await?;

        let item = store.enqueue(make_queue_task("wsQ3", agent.id, user.id)).await?;

        // Ack it first, then nack.
        store.ack_queue_item(item.id).await?;
        store.nack_queue_item(item.id).await?;

        // Should appear in dequeue again.
        let items = store.dequeue(10).await?;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, item.id);

        Ok(())
    }

    #[tokio::test]
    async fn test_dequeue_limit() -> Result<()> {
        let store = make_driver();

        let agent = store.upsert_peer("wsQ4", "agent", PeerKind::Agent, None).await?;
        let user = store.upsert_peer("wsQ4", "user", PeerKind::User, None).await?;

        for _ in 0..5 {
            store.enqueue(make_queue_task("wsQ4", agent.id, user.id)).await?;
        }

        let items = store.dequeue(3).await?;
        assert_eq!(items.len(), 3);

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Findings tests (Task 5)
    // -----------------------------------------------------------------------

    fn make_finding(orchestration_id: &str, run_id: &str, text: &str) -> Finding {
        Finding {
            id: 0,
            orchestration_id: orchestration_id.to_string(),
            run_id: run_id.to_string(),
            finding: text.to_string(),
            category: Some("test".to_string()),
            created_at: chrono::Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_save_and_list_findings() -> Result<()> {
        let store = make_driver();

        let f1 = store.save_finding(make_finding("orch-1", "run-1", "finding one")).await?;
        let f2 = store.save_finding(make_finding("orch-1", "run-1", "finding two")).await?;
        let _f3 = store.save_finding(make_finding("orch-2", "run-2", "other orch")).await?;

        assert!(f1.id > 0);
        assert!(f2.id > 0);
        assert_ne!(f1.id, f2.id);
        assert_eq!(f1.orchestration_id, "orch-1");

        let findings = store.list_findings("orch-1", 100, 0).await?;
        assert_eq!(findings.len(), 2);

        let other = store.list_findings("orch-2", 100, 0).await?;
        assert_eq!(other.len(), 1);
        assert_eq!(other[0].finding, "other orch");

        Ok(())
    }

    #[tokio::test]
    async fn test_list_findings_pagination() -> Result<()> {
        let store = make_driver();

        for i in 0..5 {
            store.save_finding(make_finding("orch-pag", "run", &format!("finding {i}"))).await?;
        }

        let page1 = store.list_findings("orch-pag", 3, 0).await?;
        let page2 = store.list_findings("orch-pag", 3, 3).await?;

        assert_eq!(page1.len(), 3);
        assert_eq!(page2.len(), 2);

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Observability tests (Task 5)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_save_and_list_deriver_runs() -> Result<()> {
        let store = make_driver();

        let run = DeriverRun {
            id: 0,
            orchestration_id: "orch-dr-1".to_string(),
            workspace: "ws-obs".to_string(),
            observer_peer_id: 1,
            observed_peer_id: 2,
            chat_id: Some("chat-x".to_string()),
            observations_in: 10,
            observations_out: 3,
            duration_ms: 250,
            created_at: chrono::Utc::now(),
        };

        let saved = store.save_deriver_run(run).await?;
        assert!(saved.id > 0);
        assert_eq!(saved.workspace, "ws-obs");
        assert_eq!(saved.observations_in, 10);
        assert_eq!(saved.observations_out, 3);

        let runs = store.list_deriver_runs("ws-obs", 10, 0).await?;
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].id, saved.id);

        // Different workspace should return nothing.
        let empty = store.list_deriver_runs("other-ws", 10, 0).await?;
        assert!(empty.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_save_and_list_dreamer_runs() -> Result<()> {
        let store = make_driver();

        let run = DreamerRun {
            id: 0,
            orchestration_id: "orch-dream-1".to_string(),
            workspace: "ws-dream".to_string(),
            observer_peer_id: 1,
            observed_peer_id: 2,
            observations_in: 20,
            findings_out: 5,
            duration_ms: 1200,
            created_at: chrono::Utc::now(),
        };

        let saved = store.save_dreamer_run(run).await?;
        assert!(saved.id > 0);
        assert_eq!(saved.findings_out, 5);

        let runs = store.list_dreamer_runs("ws-dream", 10, 0).await?;
        assert_eq!(runs.len(), 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_save_and_list_injection_logs() -> Result<()> {
        let store = make_driver();

        let log = InjectionLog {
            id: 0,
            orchestration_id: "orch-inj-1".to_string(),
            workspace: "ws-inj".to_string(),
            chat_id: "chat-abc".to_string(),
            observer_peer_id: 1,
            observed_peer_id: 2,
            observations_injected: 7,
            token_estimate: 450,
            created_at: chrono::Utc::now(),
        };

        let saved = store.save_injection_log(log).await?;
        assert!(saved.id > 0);
        assert_eq!(saved.observations_injected, 7);
        assert_eq!(saved.token_estimate, 450);

        // List all for workspace.
        let all = store.list_injection_logs("ws-inj", None, 10, 0).await?;
        assert_eq!(all.len(), 1);

        // Filter by chat_id.
        let filtered = store.list_injection_logs("ws-inj", Some("chat-abc"), 10, 0).await?;
        assert_eq!(filtered.len(), 1);

        let no_match = store.list_injection_logs("ws-inj", Some("chat-other"), 10, 0).await?;
        assert!(no_match.is_empty());

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Keyword search tests (Task 6)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_keyword_search_basic() -> Result<()> {
        let store = make_driver();

        let agent = store.upsert_peer("wsKW1", "agent", PeerKind::Agent, None).await?;
        let user = store.upsert_peer("wsKW1", "user", PeerKind::User, None).await?;

        store.create_observation(make_new_obs("wsKW1", agent.id, user.id, "user loves hiking in mountains")).await?;
        store.create_observation(make_new_obs("wsKW1", agent.id, user.id, "user prefers coffee over tea")).await?;
        store.create_observation(make_new_obs("wsKW1", agent.id, user.id, "user went hiking last weekend")).await?;

        let scope = SearchScope {
            workspace: "wsKW1".to_string(),
            ..Default::default()
        };

        let results = store.keyword_search(&scope, "hiking", 10).await?;
        assert_eq!(results.len(), 2, "should find two hiking observations");

        let contents: Vec<&str> = results.iter().map(|o| o.content.as_str()).collect();
        assert!(contents.iter().any(|c| c.contains("hiking in mountains")));
        assert!(contents.iter().any(|c| c.contains("hiking last weekend")));

        Ok(())
    }

    #[tokio::test]
    async fn test_keyword_search_no_results() -> Result<()> {
        let store = make_driver();

        let agent = store.upsert_peer("wsKW2", "agent", PeerKind::Agent, None).await?;
        let user = store.upsert_peer("wsKW2", "user", PeerKind::User, None).await?;

        store.create_observation(make_new_obs("wsKW2", agent.id, user.id, "user likes pizza")).await?;

        let scope = SearchScope {
            workspace: "wsKW2".to_string(),
            ..Default::default()
        };

        let results = store.keyword_search(&scope, "quantum physics", 10).await?;
        assert!(results.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_keyword_search_empty_query_returns_empty() -> Result<()> {
        let store = make_driver();

        let agent = store.upsert_peer("wsKW3", "agent", PeerKind::Agent, None).await?;
        let user = store.upsert_peer("wsKW3", "user", PeerKind::User, None).await?;

        store.create_observation(make_new_obs("wsKW3", agent.id, user.id, "some content")).await?;

        let scope = SearchScope {
            workspace: "wsKW3".to_string(),
            ..Default::default()
        };

        // Empty or purely-special-char query should return empty rather than error.
        let results = store.keyword_search(&scope, "\"\"", 10).await?;
        assert!(results.is_empty());

        Ok(())
    }

    // -----------------------------------------------------------------------
    // DAG traversal tests (Task 7)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_link_and_get_children_parents() -> Result<()> {
        let store = make_driver();

        let agent = store.upsert_peer("wsDAG1", "agent", PeerKind::Agent, None).await?;
        let user = store.upsert_peer("wsDAG1", "user", PeerKind::User, None).await?;

        let parent = store.create_observation(make_new_obs("wsDAG1", agent.id, user.id, "explicit fact")).await?;
        let child = store.create_observation(make_new_obs("wsDAG1", agent.id, user.id, "deduced from fact")).await?;

        store.link_observations(parent.id, child.id).await?;

        let children = store.get_children(parent.id).await?;
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].id, child.id);

        let parents = store.get_parents(child.id).await?;
        assert_eq!(parents.len(), 1);
        assert_eq!(parents[0].id, parent.id);

        Ok(())
    }

    #[tokio::test]
    async fn test_unlink_observations() -> Result<()> {
        let store = make_driver();

        let agent = store.upsert_peer("wsDAG2", "agent", PeerKind::Agent, None).await?;
        let user = store.upsert_peer("wsDAG2", "user", PeerKind::User, None).await?;

        let parent = store.create_observation(make_new_obs("wsDAG2", agent.id, user.id, "parent fact")).await?;
        let child = store.create_observation(make_new_obs("wsDAG2", agent.id, user.id, "child fact")).await?;

        store.link_observations(parent.id, child.id).await?;
        store.unlink_observations(parent.id, child.id).await?;

        let children = store.get_children(parent.id).await?;
        assert!(children.is_empty(), "should have no children after unlink");

        Ok(())
    }

    #[tokio::test]
    async fn test_trace_reasoning_bfs() -> Result<()> {
        use mchact_memory::dag::trace_reasoning;

        let store = make_driver();

        let agent = store.upsert_peer("wsDAG3", "agent", PeerKind::Agent, None).await?;
        let user = store.upsert_peer("wsDAG3", "user", PeerKind::User, None).await?;

        // Create a chain: fact_a -> deduction_b -> deeper_c
        let fact_a = store.create_observation(make_new_obs("wsDAG3", agent.id, user.id, "fact A")).await?;
        let deduction_b = store.create_observation(make_new_obs("wsDAG3", agent.id, user.id, "deduction B")).await?;
        let deeper_c = store.create_observation(make_new_obs("wsDAG3", agent.id, user.id, "deeper C")).await?;

        store.link_observations(fact_a.id, deduction_b.id).await?;
        store.link_observations(deduction_b.id, deeper_c.id).await?;

        // trace_reasoning from deeper_c should traverse to deduction_b and fact_a.
        let chain = trace_reasoning(&store, deeper_c.id).await?;
        let ids: Vec<i64> = chain.iter().map(|o| o.id).collect();

        assert!(ids.contains(&deeper_c.id));
        assert!(ids.contains(&deduction_b.id));
        assert!(ids.contains(&fact_a.id));
        assert_eq!(ids.len(), 3);

        Ok(())
    }

    #[tokio::test]
    async fn test_trace_reasoning_cycle_protection() -> Result<()> {
        use mchact_memory::dag::trace_reasoning;

        let store = make_driver();

        let agent = store.upsert_peer("wsDAG4", "agent", PeerKind::Agent, None).await?;
        let user = store.upsert_peer("wsDAG4", "user", PeerKind::User, None).await?;

        // Create a cycle: a -> b -> a (both reference each other in source_ids).
        let obs_a = store.create_observation(make_new_obs("wsDAG4", agent.id, user.id, "obs A")).await?;
        let obs_b = store.create_observation(make_new_obs("wsDAG4", agent.id, user.id, "obs B")).await?;

        store.link_observations(obs_a.id, obs_b.id).await?;
        store.link_observations(obs_b.id, obs_a.id).await?;

        // Should terminate without stack overflow.
        let chain = trace_reasoning(&store, obs_a.id).await?;
        assert_eq!(chain.len(), 2, "cycle should be visited only once each");

        Ok(())
    }
}
