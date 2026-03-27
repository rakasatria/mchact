// ---------------------------------------------------------------------------
// Deriver Agent — extracts explicit and deductive observations from messages
// ---------------------------------------------------------------------------

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{
    quality::{normalize_content, validate_observation},
    types::{NewObservation, ObservationLevel},
    ObservationStore,
};

// ---------------------------------------------------------------------------
// LLM abstraction
// ---------------------------------------------------------------------------

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn complete(
        &self,
        system: &str,
        user: &str,
    ) -> std::result::Result<String, String>;
}

// ---------------------------------------------------------------------------
// Extracted observation from LLM response
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedObservation {
    pub content: String,
    pub level: String,
    pub category: Option<String>,
    #[serde(default)]
    pub source_message_ids: Vec<i64>,
    #[serde(default)]
    pub premises: Vec<String>,
}

// ---------------------------------------------------------------------------
// Deriver system prompt
// ---------------------------------------------------------------------------

pub const DERIVER_SYSTEM_PROMPT: &str = r#"You are a memory extraction assistant. Given a conversation transcript, extract meaningful observations about the user.

Return a JSON array (no markdown, no explanation) where each element has:
- "content": the observation as a concise factual statement (max 100 chars)
- "level": "explicit" if the user directly stated it, or "deductive" if it can be logically inferred
- "category": optional string grouping (e.g. "preference", "skill", "goal", "behavior")
- "source_message_ids": array of message IDs that support this observation (use empty array if unknown)
- "premises": for deductive observations, array of supporting premises (use empty array for explicit)

Only include observations that are:
1. Factual and verifiable from the transcript
2. Non-trivial (skip greetings, filler phrases)
3. About the user's preferences, skills, goals, or behaviors

Return only the JSON array, nothing else."#;

// ---------------------------------------------------------------------------
// Main function
// ---------------------------------------------------------------------------

/// Derive observations from a conversation transcript.
///
/// Returns `(explicit_count, deductive_count)`.
pub async fn derive_observations(
    store: &dyn ObservationStore,
    llm: &dyn LlmClient,
    observer_peer_id: i64,
    observed_peer_id: i64,
    chat_id: Option<String>,
    workspace: &str,
    messages_text: &str,
) -> crate::Result<(i64, i64)> {
    let response = llm
        .complete(DERIVER_SYSTEM_PROMPT, messages_text)
        .await
        .map_err(|e| crate::MemoryError::Embedding(format!("LLM call failed: {e}")))?;

    let extractions = parse_extractions(&response);

    let mut explicit_count: i64 = 0;
    let mut deductive_count: i64 = 0;

    for extraction in extractions {
        let normalized = match normalize_content(&extraction.content, 100) {
            Some(c) => c,
            None => continue,
        };

        if validate_observation(&normalized).is_err() {
            continue;
        }

        let (level, confidence) = match extraction.level.as_str() {
            "deductive" => (ObservationLevel::Deductive, 0.70_f64),
            _ => (ObservationLevel::Explicit, 0.85_f64),
        };

        let is_explicit = matches!(level, ObservationLevel::Explicit);

        let new_obs = NewObservation {
            workspace: workspace.to_string(),
            observer_peer_id,
            observed_peer_id,
            chat_id: chat_id.clone(),
            level,
            content: normalized,
            category: extraction.category,
            confidence,
            source: Some("deriver".to_string()),
            source_ids: vec![],
            message_ids: extraction.source_message_ids,
        };

        store.create_observation(new_obs).await?;

        if is_explicit {
            explicit_count += 1;
        } else {
            deductive_count += 1;
        }
    }

    Ok((explicit_count, deductive_count))
}

// ---------------------------------------------------------------------------
// Parse LLM response into extractions
// ---------------------------------------------------------------------------

/// Parse a JSON array from the LLM response.
/// Handles optional markdown code fences (```json...```).
pub fn parse_extractions(response: &str) -> Vec<ExtractedObservation> {
    let trimmed = response.trim();

    // Strip markdown code fences if present
    let json_str = if trimmed.starts_with("```") {
        let without_open = trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```");
        let without_close = without_open.trim_end_matches("```");
        without_close.trim()
    } else {
        trimmed
    };

    serde_json::from_str::<Vec<ExtractedObservation>>(json_str).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_extractions_valid_json() {
        let json = r#"[
            {
                "content": "User prefers Rust for systems programming",
                "level": "explicit",
                "category": "preference",
                "source_message_ids": [1, 2],
                "premises": []
            }
        ]"#;

        let extractions = parse_extractions(json);
        assert_eq!(extractions.len(), 1);
        assert_eq!(extractions[0].content, "User prefers Rust for systems programming");
        assert_eq!(extractions[0].level, "explicit");
        assert_eq!(extractions[0].category, Some("preference".to_string()));
        assert_eq!(extractions[0].source_message_ids, vec![1, 2]);
    }

    #[test]
    fn test_parse_extractions_with_markdown_fences() {
        let json = r#"```json
[
    {
        "content": "User works on backend services",
        "level": "deductive",
        "category": "skill",
        "source_message_ids": [],
        "premises": ["User mentioned writing APIs"]
    }
]
```"#;

        let extractions = parse_extractions(json);
        assert_eq!(extractions.len(), 1);
        assert_eq!(extractions[0].level, "deductive");
        assert_eq!(extractions[0].premises, vec!["User mentioned writing APIs"]);
    }

    #[test]
    fn test_parse_extractions_invalid() {
        let invalid = "this is not json at all";
        let extractions = parse_extractions(invalid);
        assert!(extractions.is_empty());
    }

    #[test]
    fn test_parse_extractions_missing_optional_fields() {
        let json = r#"[{"content": "User likes coffee", "level": "explicit"}]"#;
        let extractions = parse_extractions(json);
        assert_eq!(extractions.len(), 1);
        assert_eq!(extractions[0].category, None);
        assert!(extractions[0].source_message_ids.is_empty());
        assert!(extractions[0].premises.is_empty());
    }

    #[test]
    fn test_parse_extractions_empty_array() {
        let extractions = parse_extractions("[]");
        assert!(extractions.is_empty());
    }

    #[test]
    fn test_parse_extractions_plain_fences() {
        let json = "```\n[{\"content\": \"likes tea\", \"level\": \"explicit\"}]\n```";
        let extractions = parse_extractions(json);
        assert_eq!(extractions.len(), 1);
    }

    #[test]
    fn test_deriver_system_prompt_not_empty() {
        assert!(!DERIVER_SYSTEM_PROMPT.is_empty());
        assert!(DERIVER_SYSTEM_PROMPT.contains("JSON array"));
    }
}
