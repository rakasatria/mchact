use serde_json::Value;

use super::hermes::gen_call_id;
use super::{ParseResult, ToolCall, ToolCallParser, ToolResponse};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Try to extract balanced JSON objects from `text` starting at each `{`.
/// Returns a list of `(start, end)` byte ranges (inclusive of `}`).
fn find_json_objects(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut results = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'{' {
            let start = i;
            let mut depth = 0usize;
            let mut in_string = false;
            let mut escape = false;
            let mut j = i;

            while j < bytes.len() {
                let b = bytes[j];
                if escape {
                    escape = false;
                } else if in_string {
                    if b == b'\\' {
                        escape = true;
                    } else if b == b'"' {
                        in_string = false;
                    }
                } else {
                    match b {
                        b'"' => in_string = true,
                        b'{' => depth += 1,
                        b'}' => {
                            depth -= 1;
                            if depth == 0 {
                                results.push((start, j));
                                i = j + 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                j += 1;
            }
            if depth > 0 {
                // Unbalanced — skip past this `{`
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    results
}

/// Strip a leading `<|python_tag|>` prefix (if present) and return the rest.
fn strip_python_tag(text: &str) -> &str {
    let prefix = "<|python_tag|>";
    if text.trim_start().starts_with(prefix) {
        text.trim_start()[prefix.len()..].trim_start()
    } else {
        text
    }
}

/// Parse a single JSON object into a `ToolCall`, accepting both
/// `"arguments"` and `"parameters"` as the args key.
fn json_to_tool_call(json: &Value) -> Option<ToolCall> {
    let name = json.get("name").and_then(Value::as_str)?.to_owned();
    let arguments = json
        .get("arguments")
        .or_else(|| json.get("parameters"))
        .cloned()
        .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
    Some(ToolCall {
        id: gen_call_id(),
        name,
        arguments,
    })
}

// ---------------------------------------------------------------------------
// LlamaParser
// ---------------------------------------------------------------------------

/// Parses raw JSON tool-call objects produced by Llama 3/4 models.
///
/// Supports an optional `<|python_tag|>` prefix and accepts `"parameters"`
/// as an alias for `"arguments"`.
#[derive(Debug, Clone)]
pub struct LlamaParser;

impl ToolCallParser for LlamaParser {
    fn names(&self) -> Vec<String> {
        vec!["llama3_json".to_owned(), "llama4_json".to_owned()]
    }

    fn format_tool_calls(&self, calls: &[ToolCall]) -> String {
        calls
            .iter()
            .map(|c| {
                let obj = serde_json::json!({
                    "name": c.name,
                    "arguments": c.arguments,
                });
                obj.to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn format_tool_response(&self, response: &ToolResponse) -> String {
        let obj = serde_json::json!({
            "tool_call_id": response.tool_call_id,
            "name": response.name,
            "content": response.content,
        });
        obj.to_string()
    }

    fn parse(&self, text: &str) -> ParseResult {
        let clean = strip_python_tag(text);
        let ranges = find_json_objects(clean);

        let mut calls: Vec<ToolCall> = Vec::new();
        let mut consumed_ranges: Vec<(usize, usize)> = Vec::new();

        for (start, end) in ranges {
            let slice = &clean[start..=end];
            if let Ok(json) = serde_json::from_str::<Value>(slice) {
                // Only treat as a tool call if it has a "name" field.
                if json.get("name").is_some() {
                    if let Some(call) = json_to_tool_call(&json) {
                        calls.push(call);
                        consumed_ranges.push((start, end));
                    }
                }
            }
        }

        // Build text by removing consumed JSON object regions.
        let text_part = if consumed_ranges.is_empty() {
            let t = clean.trim().to_owned();
            if t.is_empty() { None } else { Some(t) }
        } else {
            // Remove consumed slices from the cleaned text.
            let mut result = String::new();
            let mut pos = 0usize;
            for (start, end) in &consumed_ranges {
                if pos < *start {
                    result.push_str(&clean[pos..*start]);
                }
                pos = end + 1;
            }
            if pos < clean.len() {
                result.push_str(&clean[pos..]);
            }
            let trimmed = result.trim().to_owned();
            if trimmed.is_empty() { None } else { Some(trimmed) }
        };

        let calls_part = if calls.is_empty() { None } else { Some(calls) };
        (text_part, calls_part)
    }

    fn clone_box(&self) -> Box<dyn ToolCallParser> {
        Box::new(self.clone())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn parser() -> LlamaParser {
        LlamaParser
    }

    #[test]
    fn test_llama_parse_basic() {
        let input = r#"{"name":"get_weather","arguments":{"city":"Paris"}}"#;
        let (text, calls) = parser().parse(input);
        assert!(text.is_none(), "no text expected");
        let calls = calls.expect("expected a call");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "get_weather");
        assert_eq!(calls[0].arguments, json!({"city": "Paris"}));
    }

    #[test]
    fn test_llama_parse_with_parameters_alias() {
        let input = r#"{"name":"search","parameters":{"query":"rust"}}"#;
        let (_text, calls) = parser().parse(input);
        let calls = calls.expect("expected a call");
        assert_eq!(calls[0].name, "search");
        assert_eq!(calls[0].arguments, json!({"query": "rust"}));
    }

    #[test]
    fn test_llama_parse_with_python_tag_prefix() {
        let input = r#"<|python_tag|>{"name":"run","arguments":{"code":"print(1)"}}"#;
        let (_text, calls) = parser().parse(input);
        let calls = calls.expect("expected a call");
        assert_eq!(calls[0].name, "run");
    }

    #[test]
    fn test_llama_parse_with_content_before() {
        let input = r#"Sure! {"name":"lookup","arguments":{"id":42}}"#;
        let (text, calls) = parser().parse(input);
        let calls = calls.expect("expected a call");
        assert_eq!(calls[0].name, "lookup");
        // text before the JSON should be present
        assert!(text.is_some());
    }

    #[test]
    fn test_llama_no_tool_call() {
        let input = "Hello, how can I help?";
        let (text, calls) = parser().parse(input);
        assert_eq!(text.as_deref(), Some("Hello, how can I help?"));
        assert!(calls.is_none());
    }

    #[test]
    fn test_llama_format_tool_calls() {
        let calls = vec![ToolCall {
            id: "call_1".to_owned(),
            name: "ping".to_owned(),
            arguments: json!({}),
        }];
        let output = parser().format_tool_calls(&calls);
        assert!(output.contains("\"ping\""));
        assert!(output.contains("\"arguments\""));
    }

    #[test]
    fn test_llama_format_tool_response() {
        let resp = ToolResponse {
            tool_call_id: "call_1".to_owned(),
            name: "ping".to_owned(),
            content: json!({"ok": true}),
        };
        let output = parser().format_tool_response(&resp);
        assert!(output.contains("call_1"));
        assert!(output.contains("ping"));
    }

    #[test]
    fn test_llama_names() {
        let names = parser().names();
        assert!(names.contains(&"llama3_json".to_owned()));
        assert!(names.contains(&"llama4_json".to_owned()));
    }
}
