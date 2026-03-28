use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchPrompt {
    pub prompt_index: u64,
    pub prompt: String,
    #[serde(default)]
    pub toolsets: Option<Vec<String>>,
    #[serde(default)]
    pub image: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolStat {
    pub count: u64,
    pub success: u64,
    pub failure: u64,
}

impl ToolStat {
    pub fn zero() -> Self {
        Self {
            count: 0,
            success: 0,
            failure: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningStats {
    pub total_assistant_turns: u64,
    pub turns_with_reasoning: u64,
    pub turns_without_reasoning: u64,
    pub has_any_reasoning: bool,
}

// ---------------------------------------------------------------------------
// Tool success detection
// ---------------------------------------------------------------------------

/// Determines if a tool response content indicates success or failure.
///
/// Rules (in order):
/// 1. Empty content → false
/// 2. Content starts with "Error:" (case-insensitive) → false
/// 3. JSON dict with non-null `"error"` field → false
/// 4. JSON dict with nested `"content"` sub-dict containing non-null `"error"` → false
/// 5. JSON dict with `"success": false` → false
/// 6. Otherwise → true
pub fn is_tool_success(content: &str) -> bool {
    let trimmed = content.trim();

    // Rule 1: empty
    if trimmed.is_empty() {
        return false;
    }

    // Rule 2: starts with "Error:" (case-insensitive)
    if trimmed.len() >= 6 && trimmed[..6].eq_ignore_ascii_case("error:") {
        return false;
    }

    // Try to parse as JSON object for rules 3-5
    if let Ok(serde_json::Value::Object(map)) = serde_json::from_str(trimmed) {
        // Rule 3: top-level non-null "error" field
        if let Some(err_val) = map.get("error") {
            if !err_val.is_null() {
                return false;
            }
        }

        // Rule 4: nested "content" sub-dict with non-null "error"
        if let Some(serde_json::Value::Object(content_map)) = map.get("content") {
            if let Some(err_val) = content_map.get("error") {
                if !err_val.is_null() {
                    return false;
                }
            }
        }

        // Rule 5: "success": false
        if let Some(serde_json::Value::Bool(false)) = map.get("success") {
            return false;
        }
    }

    true
}

// ---------------------------------------------------------------------------
// Tool stats extraction
// ---------------------------------------------------------------------------

/// Walk OpenAI-format messages and accumulate per-tool call counts.
///
/// - Assistant messages with `tool_calls` array: count each tool call, map call ID → tool name.
/// - Tool messages: look up tool name by `tool_call_id`, increment success or failure.
pub fn extract_tool_stats(messages: &[serde_json::Value]) -> HashMap<String, ToolStat> {
    // First pass: build call_id → tool_name index and count all tool calls
    let mut call_id_to_name: HashMap<String, String> = HashMap::new();
    let mut stats: HashMap<String, ToolStat> = HashMap::new();

    for msg in messages {
        let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");

        if role == "assistant" {
            if let Some(tool_calls) = msg.get("tool_calls").and_then(|v| v.as_array()) {
                for tc in tool_calls {
                    let id = tc
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_owned();
                    let name = tc
                        .get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_owned();

                    if !name.is_empty() {
                        call_id_to_name.insert(id, name.clone());
                        let entry = stats.entry(name).or_insert_with(ToolStat::zero);
                        entry.count += 1;
                    }
                }
            }
        }
    }

    // Second pass: process tool result messages
    for msg in messages {
        let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");

        if role == "tool" {
            let call_id = msg
                .get("tool_call_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let content = msg
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if let Some(tool_name) = call_id_to_name.get(call_id) {
                let entry = stats.entry(tool_name.clone()).or_insert_with(ToolStat::zero);
                if is_tool_success(content) {
                    entry.success += 1;
                } else {
                    entry.failure += 1;
                }
            }
        }
    }

    stats
}

// ---------------------------------------------------------------------------
// Reasoning stats extraction
// ---------------------------------------------------------------------------

/// Count assistant turns with vs without reasoning.
///
/// Reasoning is present if the message content contains `<REASONING_SCRATCHPAD>`
/// OR the message has a non-empty `reasoning` field.
pub fn extract_reasoning_stats(messages: &[serde_json::Value]) -> ReasoningStats {
    let mut total_assistant_turns: u64 = 0;
    let mut turns_with_reasoning: u64 = 0;

    for msg in messages {
        let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");
        if role != "assistant" {
            continue;
        }

        total_assistant_turns += 1;

        let has_scratchpad = msg
            .get("content")
            .and_then(|v| v.as_str())
            .map(|s| s.contains("<REASONING_SCRATCHPAD>"))
            .unwrap_or(false);

        let has_reasoning_field = msg
            .get("reasoning")
            .map(|v| match v {
                serde_json::Value::String(s) => !s.is_empty(),
                serde_json::Value::Null => false,
                _ => true,
            })
            .unwrap_or(false);

        if has_scratchpad || has_reasoning_field {
            turns_with_reasoning += 1;
        }
    }

    let turns_without_reasoning = total_assistant_turns.saturating_sub(turns_with_reasoning);
    let has_any_reasoning = turns_with_reasoning > 0;

    ReasoningStats {
        total_assistant_turns,
        turns_with_reasoning,
        turns_without_reasoning,
        has_any_reasoning,
    }
}

// ---------------------------------------------------------------------------
// Normalization utilities
// ---------------------------------------------------------------------------

/// Ensure ALL registered tools appear in the stats map (unused tools get zero counts).
/// Also include any unexpected tools found in `raw`.
pub fn normalize_tool_stats(
    raw: &HashMap<String, ToolStat>,
    all_tools: &[String],
) -> HashMap<String, ToolStat> {
    let mut result: HashMap<String, ToolStat> = raw.clone();

    for tool in all_tools {
        result.entry(tool.clone()).or_insert_with(ToolStat::zero);
    }

    result
}

/// Simple mapping: tool_name → failure count. All registered tools included.
/// Also includes any unexpected tools found in `raw`.
pub fn normalize_error_counts(
    raw: &HashMap<String, ToolStat>,
    all_tools: &[String],
) -> HashMap<String, u64> {
    let mut result: HashMap<String, u64> = HashMap::new();

    // Seed all registered tools with 0
    for tool in all_tools {
        result.insert(tool.clone(), 0);
    }

    // Overlay actual failure counts from raw (covers both registered and unexpected tools)
    for (name, stat) in raw {
        result.insert(name.clone(), stat.failure);
    }

    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_is_tool_success_empty() {
        assert!(!is_tool_success(""));
        assert!(!is_tool_success("   "));
    }

    #[test]
    fn test_is_tool_success_error_prefix() {
        assert!(!is_tool_success("Error: something went wrong"));
        assert!(!is_tool_success("error: lowercase also fails"));
        assert!(!is_tool_success("ERROR: all caps fails too"));
    }

    #[test]
    fn test_is_tool_success_json_error() {
        assert!(!is_tool_success(r#"{"error": "some error message"}"#));
        assert!(!is_tool_success(r#"{"error": 42}"#));
        // null error should not fail
        assert!(is_tool_success(r#"{"error": null, "result": "ok"}"#));
    }

    #[test]
    fn test_is_tool_success_nested_error() {
        assert!(!is_tool_success(
            r#"{"content": {"error": "nested error"}}"#
        ));
        assert!(!is_tool_success(r#"{"content": {"error": true}}"#));
        // null nested error should not fail
        assert!(is_tool_success(
            r#"{"content": {"error": null, "value": "ok"}}"#
        ));
    }

    #[test]
    fn test_is_tool_success_false() {
        assert!(!is_tool_success(r#"{"success": false}"#));
        // success: true should pass
        assert!(is_tool_success(r#"{"success": true}"#));
    }

    #[test]
    fn test_is_tool_success_normal() {
        assert!(is_tool_success("ok"));
        assert!(is_tool_success("file written successfully"));
        assert!(is_tool_success(r#"{"result": "data"}"#));
        assert!(is_tool_success(r#"{"output": "hello world"}"#));
    }

    #[test]
    fn test_extract_tool_stats() {
        let messages = vec![
            json!({
                "role": "assistant",
                "tool_calls": [
                    {
                        "id": "call_1",
                        "function": { "name": "read_file", "arguments": "{}" }
                    },
                    {
                        "id": "call_2",
                        "function": { "name": "write_file", "arguments": "{}" }
                    }
                ]
            }),
            json!({
                "role": "tool",
                "tool_call_id": "call_1",
                "content": "file contents here"
            }),
            json!({
                "role": "tool",
                "tool_call_id": "call_2",
                "content": "Error: permission denied"
            }),
            json!({
                "role": "assistant",
                "tool_calls": [
                    {
                        "id": "call_3",
                        "function": { "name": "read_file", "arguments": "{}" }
                    }
                ]
            }),
            json!({
                "role": "tool",
                "tool_call_id": "call_3",
                "content": "more file contents"
            }),
        ];

        let stats = extract_tool_stats(&messages);

        let read_stat = stats.get("read_file").expect("read_file should be present");
        assert_eq!(read_stat.count, 2);
        assert_eq!(read_stat.success, 2);
        assert_eq!(read_stat.failure, 0);

        let write_stat = stats
            .get("write_file")
            .expect("write_file should be present");
        assert_eq!(write_stat.count, 1);
        assert_eq!(write_stat.success, 0);
        assert_eq!(write_stat.failure, 1);
    }

    #[test]
    fn test_extract_reasoning_stats() {
        let messages = vec![
            json!({ "role": "user", "content": "Hello" }),
            json!({
                "role": "assistant",
                "content": "<REASONING_SCRATCHPAD>thinking...</REASONING_SCRATCHPAD>\nActual response"
            }),
            json!({ "role": "user", "content": "Follow-up" }),
            json!({
                "role": "assistant",
                "content": "No reasoning here"
            }),
            json!({
                "role": "assistant",
                "content": "Some reply",
                "reasoning": "I reasoned about this"
            }),
        ];

        let stats = extract_reasoning_stats(&messages);
        assert_eq!(stats.total_assistant_turns, 3);
        assert_eq!(stats.turns_with_reasoning, 2);
        assert_eq!(stats.turns_without_reasoning, 1);
        assert!(stats.has_any_reasoning);
    }

    #[test]
    fn test_normalize_tool_stats() {
        let mut raw: HashMap<String, ToolStat> = HashMap::new();
        raw.insert(
            "existing_tool".to_owned(),
            ToolStat {
                count: 5,
                success: 4,
                failure: 1,
            },
        );

        let all_tools = vec![
            "existing_tool".to_owned(),
            "unused_tool".to_owned(),
            "another_tool".to_owned(),
        ];

        let normalized = normalize_tool_stats(&raw, &all_tools);

        // Existing tool preserves its stats
        let existing = normalized.get("existing_tool").unwrap();
        assert_eq!(existing.count, 5);
        assert_eq!(existing.success, 4);
        assert_eq!(existing.failure, 1);

        // Unused tools get zero stats
        let unused = normalized.get("unused_tool").unwrap();
        assert_eq!(unused.count, 0);
        assert_eq!(unused.success, 0);
        assert_eq!(unused.failure, 0);

        assert!(normalized.contains_key("another_tool"));
    }

    #[test]
    fn test_normalize_error_counts() {
        let mut raw: HashMap<String, ToolStat> = HashMap::new();
        raw.insert(
            "tool_a".to_owned(),
            ToolStat {
                count: 3,
                success: 1,
                failure: 2,
            },
        );
        raw.insert(
            "unexpected_tool".to_owned(),
            ToolStat {
                count: 1,
                success: 0,
                failure: 1,
            },
        );

        let all_tools = vec!["tool_a".to_owned(), "tool_b".to_owned()];

        let error_counts = normalize_error_counts(&raw, &all_tools);

        assert_eq!(*error_counts.get("tool_a").unwrap(), 2);
        assert_eq!(*error_counts.get("tool_b").unwrap(), 0);
        // Unexpected tool from raw is also included
        assert_eq!(*error_counts.get("unexpected_tool").unwrap(), 1);
    }
}
