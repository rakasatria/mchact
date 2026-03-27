use crate::types::{Observation, ObservationLevel, PeerCard};

// ---------------------------------------------------------------------------
// Memory context builder
// ---------------------------------------------------------------------------

/// Build an XML-formatted memory context string suitable for injection into a
/// system prompt.
///
/// Format:
/// ```xml
/// <peer_card peer="alice">
/// fact 1
/// fact 2
/// </peer_card>
///
/// <observations>
/// [EXPLICIT] content here
/// [DEDUCTIVE] content here (from: #1, #2)
/// [INDUCTIVE] pattern content
/// (+5 observations omitted)
/// </observations>
/// ```
///
/// `omitted_count` is the number of observations that were excluded (e.g. due
/// to token budget). When zero, the omission line is not emitted.
pub fn build_memory_context(
    peer_card: Option<&PeerCard>,
    observations: &[Observation],
    omitted_count: usize,
) -> String {
    let mut parts: Vec<String> = Vec::new();

    // --- Peer card block ---
    if let Some(card) = peer_card {
        let peer_name = &card.peer_name;
        let facts_text = card.facts.join("\n");
        parts.push(format!(
            "<peer_card peer=\"{peer_name}\">\n{facts_text}\n</peer_card>"
        ));
    }

    // --- Observations block ---
    if !observations.is_empty() || omitted_count > 0 {
        let mut obs_lines: Vec<String> = Vec::new();

        for obs in observations {
            let tag = level_tag(&obs.level);
            let source_suffix = if obs.source_ids.is_empty() {
                String::new()
            } else {
                let ids: Vec<String> = obs.source_ids.iter().map(|id| format!("#{id}")).collect();
                format!(" (from: {})", ids.join(", "))
            };
            obs_lines.push(format!("[{tag}] {}{source_suffix}", obs.content));
        }

        if omitted_count > 0 {
            obs_lines.push(format!("(+{omitted_count} observations omitted)"));
        }

        parts.push(format!("<observations>\n{}\n</observations>", obs_lines.join("\n")));
    }

    parts.join("\n\n")
}

fn level_tag(level: &ObservationLevel) -> &'static str {
    match level {
        ObservationLevel::Explicit => "EXPLICIT",
        ObservationLevel::Deductive => "DEDUCTIVE",
        ObservationLevel::Inductive => "INDUCTIVE",
        ObservationLevel::Contradiction => "CONTRADICTION",
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::types::{Observation, ObservationLevel, PeerCard};

    fn make_obs(id: i64, level: ObservationLevel, content: &str, source_ids: Vec<i64>) -> Observation {
        Observation {
            id,
            workspace: "ws".to_string(),
            observer_peer_id: 1,
            observed_peer_id: 2,
            chat_id: None,
            level,
            content: content.to_string(),
            category: None,
            confidence: 0.9,
            source: None,
            source_ids,
            message_ids: vec![],
            times_derived: 0,
            is_archived: false,
            archived_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_empty_context() {
        let result = build_memory_context(None, &[], 0);
        assert!(result.is_empty(), "empty inputs should produce empty string");
    }

    #[test]
    fn test_peer_card_only() {
        let card = PeerCard {
            peer_id: 1,
            peer_name: "alice".to_string(),
            facts: vec!["Likes Rust".to_string(), "Works remotely".to_string()],
        };

        let result = build_memory_context(Some(&card), &[], 0);
        assert!(result.contains("<peer_card peer=\"alice\">"));
        assert!(result.contains("Likes Rust"));
        assert!(result.contains("Works remotely"));
        assert!(result.contains("</peer_card>"));
        assert!(!result.contains("<observations>"));
    }

    #[test]
    fn test_observations_without_source_ids() {
        let obs = vec![
            make_obs(1, ObservationLevel::Explicit, "user likes cats", vec![]),
            make_obs(2, ObservationLevel::Inductive, "user is an animal person", vec![]),
        ];

        let result = build_memory_context(None, &obs, 0);
        assert!(result.contains("<observations>"));
        assert!(result.contains("[EXPLICIT] user likes cats"));
        assert!(result.contains("[INDUCTIVE] user is an animal person"));
        assert!(!result.contains("(from:"));
        assert!(!result.contains("omitted"));
    }

    #[test]
    fn test_observations_with_source_ids() {
        let obs = vec![
            make_obs(1, ObservationLevel::Explicit, "fact A", vec![]),
            make_obs(2, ObservationLevel::Explicit, "fact B", vec![]),
            make_obs(3, ObservationLevel::Deductive, "conclusion C", vec![1, 2]),
        ];

        let result = build_memory_context(None, &obs, 0);
        assert!(result.contains("[DEDUCTIVE] conclusion C (from: #1, #2)"));
    }

    #[test]
    fn test_omitted_count_appended() {
        let obs = vec![make_obs(1, ObservationLevel::Explicit, "single fact", vec![])];

        let result = build_memory_context(None, &obs, 5);
        assert!(result.contains("(+5 observations omitted)"));
    }

    #[test]
    fn test_omitted_zero_not_shown() {
        let obs = vec![make_obs(1, ObservationLevel::Explicit, "fact", vec![])];

        let result = build_memory_context(None, &obs, 0);
        assert!(!result.contains("omitted"));
    }

    #[test]
    fn test_full_context_structure() {
        let card = PeerCard {
            peer_id: 10,
            peer_name: "bob".to_string(),
            facts: vec!["Enjoys hiking".to_string()],
        };

        let obs = vec![
            make_obs(1, ObservationLevel::Explicit, "bob mentioned mountains", vec![]),
            make_obs(2, ObservationLevel::Deductive, "bob likes outdoors", vec![1]),
        ];

        let result = build_memory_context(Some(&card), &obs, 3);

        // Both blocks present
        assert!(result.contains("<peer_card peer=\"bob\">"));
        assert!(result.contains("</peer_card>"));
        assert!(result.contains("<observations>"));
        assert!(result.contains("</observations>"));

        // Content correct
        assert!(result.contains("[EXPLICIT] bob mentioned mountains"));
        assert!(result.contains("[DEDUCTIVE] bob likes outdoors (from: #1)"));
        assert!(result.contains("(+3 observations omitted)"));

        // Blocks separated by blank line
        assert!(result.contains("</peer_card>\n\n<observations>"));
    }

    #[test]
    fn test_contradiction_level_tag() {
        let obs = vec![make_obs(1, ObservationLevel::Contradiction, "conflicting info", vec![])];
        let result = build_memory_context(None, &obs, 0);
        assert!(result.contains("[CONTRADICTION] conflicting info"));
    }
}
