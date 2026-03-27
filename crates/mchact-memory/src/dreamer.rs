// ---------------------------------------------------------------------------
// Dreamer Agent — derives higher-order knowledge from stored observations
// ---------------------------------------------------------------------------

use serde::{Deserialize, Serialize};

use crate::{
    deriver::LlmClient,
    quality::{normalize_content, validate_observation},
    types::{NewObservation, Observation, ObservationLevel, SearchScope},
    ObservationStore,
};

// ---------------------------------------------------------------------------
// LLM prompts
// ---------------------------------------------------------------------------

pub const DEDUCTION_PROMPT: &str = r#"Given these observations about a person, what can you logically infer?

Return a JSON array where each element has:
- "content": the inferred fact (concise, max 100 chars)
- "premise_ids": array of observation IDs that support this deduction

Return only the JSON array, nothing else."#;

pub const INDUCTION_PROMPT: &str = r#"What patterns do you see across these observations about a person?

Return a JSON array where each element has:
- "content": the identified pattern (concise, max 100 chars)
- "confidence": "high", "medium", or "low" based on how well-supported the pattern is

Return only the JSON array, nothing else."#;

pub const CONTRADICTION_PROMPT: &str = r#"Do any of these observations about a person contradict each other?

Return a JSON array where each element has:
- "content": a brief description of the contradiction (max 100 chars)
- "conflicting_ids": array of observation IDs that are in conflict

Only include genuine logical contradictions. Return only the JSON array, nothing else."#;

// ---------------------------------------------------------------------------
// LLM response types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeductionResult {
    content: String,
    #[serde(default)]
    premise_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InductionResult {
    content: String,
    #[serde(default)]
    confidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ContradictionResult {
    content: String,
    #[serde(default)]
    conflicting_ids: Vec<i64>,
}

// ---------------------------------------------------------------------------
// DreamStats
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DreamStats {
    pub deductions_created: i64,
    pub inductions_created: i64,
    pub contradictions_created: i64,
}

// ---------------------------------------------------------------------------
// Main function
// ---------------------------------------------------------------------------

/// Run a full dream cycle for the given peer pair.
///
/// Phases:
/// 1. Fetch existing observations
/// 2. Skip if fewer than 3
/// 3. Deduction pass
/// 4. Induction pass
/// 5. Contradiction pass
/// 6. Consolidation (skipped)
/// 7. Peer card update
pub async fn run_dream_cycle(
    store: &dyn ObservationStore,
    llm: &dyn LlmClient,
    observer_peer_id: i64,
    observed_peer_id: i64,
    workspace: &str,
) -> crate::Result<DreamStats> {
    // Phase 1: fetch observations
    let scope = SearchScope {
        workspace: workspace.to_string(),
        observer_peer_id: Some(observer_peer_id),
        observed_peer_id: Some(observed_peer_id),
        chat_id: None,
        min_confidence: None,
        include_archived: false,
    };

    let observations = store.list_observations(scope, 200, 0).await?;

    // Phase 2: skip if insufficient data
    if observations.len() < 3 {
        return Ok(DreamStats::default());
    }

    let obs_text = format_observations_for_llm(&observations);
    let mut stats = DreamStats::default();

    // Phase 3: Deduction
    let deduction_user = format!(
        "Observations:\n{obs_text}\n\nWhat can you logically infer from these?"
    );
    if let Ok(response) = llm.complete(DEDUCTION_PROMPT, &deduction_user).await {
        let results = parse_deductions(&response);
        for result in results {
            if let Some(content) = normalize_content(&result.content, 100) {
                if validate_observation(&content).is_ok() {
                    let new_obs = NewObservation {
                        workspace: workspace.to_string(),
                        observer_peer_id,
                        observed_peer_id,
                        chat_id: None,
                        level: ObservationLevel::Deductive,
                        content,
                        category: Some("deduction".to_string()),
                        confidence: 0.70,
                        source: Some("dreamer".to_string()),
                        source_ids: result.premise_ids,
                        message_ids: vec![],
                    };
                    store.create_observation(new_obs).await?;
                    stats.deductions_created += 1;
                }
            }
        }
    }

    // Phase 4: Induction
    let induction_user = format!(
        "Observations:\n{obs_text}\n\nWhat patterns do you see?"
    );
    if let Ok(response) = llm.complete(INDUCTION_PROMPT, &induction_user).await {
        let results = parse_inductions(&response);
        for result in results {
            if let Some(content) = normalize_content(&result.content, 100) {
                if validate_observation(&content).is_ok() {
                    let confidence = match result.confidence.as_str() {
                        "high" => 0.75_f64,
                        "low" => 0.50_f64,
                        _ => 0.62_f64, // medium or unknown
                    };
                    let new_obs = NewObservation {
                        workspace: workspace.to_string(),
                        observer_peer_id,
                        observed_peer_id,
                        chat_id: None,
                        level: ObservationLevel::Inductive,
                        content,
                        category: Some("induction".to_string()),
                        confidence,
                        source: Some("dreamer".to_string()),
                        source_ids: vec![],
                        message_ids: vec![],
                    };
                    store.create_observation(new_obs).await?;
                    stats.inductions_created += 1;
                }
            }
        }
    }

    // Phase 5: Contradiction
    let contradiction_user = format!(
        "Observations:\n{obs_text}\n\nAre there any contradictions?"
    );
    if let Ok(response) = llm.complete(CONTRADICTION_PROMPT, &contradiction_user).await {
        let results = parse_contradictions(&response);
        for result in results {
            if let Some(content) = normalize_content(&result.content, 100) {
                if validate_observation(&content).is_ok() {
                    let new_obs = NewObservation {
                        workspace: workspace.to_string(),
                        observer_peer_id,
                        observed_peer_id,
                        chat_id: None,
                        level: ObservationLevel::Contradiction,
                        content,
                        category: Some("contradiction".to_string()),
                        confidence: 0.90,
                        source: Some("dreamer".to_string()),
                        source_ids: result.conflicting_ids,
                        message_ids: vec![],
                    };
                    store.create_observation(new_obs).await?;
                    stats.contradictions_created += 1;
                }
            }
        }
    }

    // Phase 6: Consolidation — skipped for now

    // Phase 7: Peer card update — extract top stable facts
    let top_facts: Vec<String> = observations
        .iter()
        .filter(|o| {
            o.confidence >= 0.75
                && matches!(o.level, ObservationLevel::Explicit | ObservationLevel::Deductive)
        })
        .take(crate::types::PeerCard::MAX_FACTS)
        .map(|o| o.content.clone())
        .collect();

    if !top_facts.is_empty() {
        store.update_peer_card(observed_peer_id, top_facts).await?;
    }

    Ok(stats)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Format observations for LLM input as `[id=N] [LEVEL] content`.
pub fn format_observations_for_llm(observations: &[Observation]) -> String {
    observations
        .iter()
        .map(|o| {
            format!(
                "[id={}] [{}] {}",
                o.id,
                o.level.as_str().to_uppercase(),
                o.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_deductions(response: &str) -> Vec<DeductionResult> {
    let json_str = strip_fences(response);
    serde_json::from_str::<Vec<DeductionResult>>(json_str).unwrap_or_default()
}

fn parse_inductions(response: &str) -> Vec<InductionResult> {
    let json_str = strip_fences(response);
    serde_json::from_str::<Vec<InductionResult>>(json_str).unwrap_or_default()
}

fn parse_contradictions(response: &str) -> Vec<ContradictionResult> {
    let json_str = strip_fences(response);
    serde_json::from_str::<Vec<ContradictionResult>>(json_str).unwrap_or_default()
}

fn strip_fences(s: &str) -> &str {
    let trimmed = s.trim();
    if trimmed.starts_with("```") {
        let without_open = trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```");
        without_open.trim_end_matches("```").trim()
    } else {
        trimmed
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_observations_for_llm() {
        use crate::types::ObservationLevel;
        use chrono::Utc;

        let obs = vec![Observation {
            id: 42,
            workspace: "ws".to_string(),
            observer_peer_id: 1,
            observed_peer_id: 2,
            chat_id: None,
            level: ObservationLevel::Explicit,
            content: "User prefers Rust".to_string(),
            category: None,
            confidence: 0.85,
            source: None,
            source_ids: vec![],
            message_ids: vec![],
            times_derived: 0,
            is_archived: false,
            archived_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }];

        let result = format_observations_for_llm(&obs);
        assert_eq!(result, "[id=42] [EXPLICIT] User prefers Rust");
    }

    #[test]
    fn test_format_observations_empty() {
        let result = format_observations_for_llm(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_deduction_prompt_not_empty() {
        assert!(!DEDUCTION_PROMPT.is_empty());
        assert!(DEDUCTION_PROMPT.contains("logically infer"));
    }

    #[test]
    fn test_induction_prompt_not_empty() {
        assert!(!INDUCTION_PROMPT.is_empty());
        assert!(INDUCTION_PROMPT.contains("patterns"));
    }

    #[test]
    fn test_contradiction_prompt_not_empty() {
        assert!(!CONTRADICTION_PROMPT.is_empty());
        assert!(CONTRADICTION_PROMPT.contains("contradict"));
    }

    #[test]
    fn test_parse_deductions_valid() {
        let json = r#"[{"content": "User is a developer", "premise_ids": [1, 2]}]"#;
        let results = parse_deductions(json);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "User is a developer");
        assert_eq!(results[0].premise_ids, vec![1, 2]);
    }

    #[test]
    fn test_parse_inductions_valid() {
        let json = r#"[{"content": "User consistently chooses typed languages", "confidence": "high"}]"#;
        let results = parse_inductions(json);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].confidence, "high");
    }

    #[test]
    fn test_parse_contradictions_valid() {
        let json = r#"[{"content": "Claims both novice and expert", "conflicting_ids": [3, 7]}]"#;
        let results = parse_contradictions(json);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].conflicting_ids, vec![3, 7]);
    }

    #[test]
    fn test_parse_invalid_json_returns_empty() {
        assert!(parse_deductions("not json").is_empty());
        assert!(parse_inductions("not json").is_empty());
        assert!(parse_contradictions("not json").is_empty());
    }

    #[test]
    fn test_dream_stats_default() {
        let stats = DreamStats::default();
        assert_eq!(stats.deductions_created, 0);
        assert_eq!(stats.inductions_created, 0);
        assert_eq!(stats.contradictions_created, 0);
    }
}
