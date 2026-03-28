use rand::Rng;
use serde_json::Value;

use super::{ParseResult, ToolCall, ToolCallParser, ToolResponse};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const TOOL_CALLS_TOKEN: &str = "[TOOL_CALLS]";

/// Generate a 9-character random alphanumeric string for a tool call ID.
fn gen_mistral_id() -> String {
    rand::thread_rng()
        .sample_iter(rand::distributions::Alphanumeric)
        .take(9)
        .map(char::from)
        .collect()
}

/// Parse pre-v11 Mistral format: `[TOOL_CALLS][{"name":"...", "arguments":{...}}]`
/// The token has been stripped; `rest` starts at `[` or `{`.
fn parse_pre_v11(rest: &str) -> Vec<ToolCall> {
    let mut calls = Vec::new();
    let json_str = rest.trim();

    // Could be a JSON array or a single object.
    let items: Vec<Value> = if json_str.starts_with('[') {
        serde_json::from_str(json_str).unwrap_or_default()
    } else if json_str.starts_with('{') {
        serde_json::from_str::<Value>(json_str)
            .ok()
            .map(|v| vec![v])
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    for item in items {
        let name = match item.get("name").and_then(Value::as_str) {
            Some(n) => n.to_owned(),
            None => continue,
        };
        let arguments = item
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
        calls.push(ToolCall {
            id: gen_mistral_id(),
            name,
            arguments,
        });
    }

    calls
}

/// Parse v11+ Mistral format: `[TOOL_CALLS]func_name{"arg":"val"}func2{...}`
/// `rest` is everything after the `[TOOL_CALLS]` token (no leading `[` or `{`).
fn parse_v11(rest: &str) -> Vec<ToolCall> {
    let mut calls = Vec::new();
    let mut s = rest.trim();

    while !s.is_empty() {
        // Find the first `{` — everything before it is the function name.
        let brace_pos = match s.find('{') {
            Some(p) => p,
            None => break,
        };

        let name = s[..brace_pos].trim().to_owned();
        if name.is_empty() {
            break;
        }

        // Extract balanced JSON starting at `brace_pos`.
        let json_slice = &s[brace_pos..];
        let (json_end, arguments) = match extract_json_object(json_slice) {
            Some(x) => x,
            None => break,
        };

        calls.push(ToolCall {
            id: gen_mistral_id(),
            name,
            arguments,
        });

        s = s[brace_pos + json_end..].trim_start();
    }

    calls
}

/// Extract a balanced JSON object from the start of `s`.
/// Returns `(consumed_bytes, parsed_value)`.
fn extract_json_object(s: &str) -> Option<(usize, Value)> {
    let bytes = s.as_bytes();
    if bytes.first() != Some(&b'{') {
        return None;
    }

    let mut depth = 0usize;
    let mut in_string = false;
    let mut escape = false;

    for (i, &b) in bytes.iter().enumerate() {
        if escape {
            escape = false;
            continue;
        }
        if in_string {
            if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    let slice = &s[..=i];
                    let val: Value = serde_json::from_str(slice).ok()?;
                    return Some((i + 1, val));
                }
            }
            _ => {}
        }
    }
    None
}

// ---------------------------------------------------------------------------
// MistralParser
// ---------------------------------------------------------------------------

/// Parses Mistral tool-call formats (pre-v11 JSON array and v11+ inline).
#[derive(Debug, Clone)]
pub struct MistralParser;

impl ToolCallParser for MistralParser {
    fn names(&self) -> Vec<String> {
        vec!["mistral".to_owned()]
    }

    fn format_tool_calls(&self, calls: &[ToolCall]) -> String {
        // Use pre-v11 JSON array format for output.
        let items: Vec<Value> = calls
            .iter()
            .map(|c| {
                serde_json::json!({
                    "name": c.name,
                    "arguments": c.arguments,
                })
            })
            .collect();
        format!("[TOOL_CALLS]{}", serde_json::to_string(&items).unwrap_or_default())
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
        let token_pos = match text.find(TOOL_CALLS_TOKEN) {
            Some(p) => p,
            None => {
                let t = text.trim().to_owned();
                return (if t.is_empty() { None } else { Some(t) }, None);
            }
        };

        let prefix = text[..token_pos].trim().to_owned();
        let after_token = text[token_pos + TOOL_CALLS_TOKEN.len()..].trim_start();

        let calls = if after_token.starts_with('[') || after_token.starts_with('{') {
            parse_pre_v11(after_token)
        } else {
            parse_v11(after_token)
        };

        let text_part = if prefix.is_empty() { None } else { Some(prefix) };
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

    fn parser() -> MistralParser {
        MistralParser
    }

    #[test]
    fn test_mistral_parse_pre_v11_array() {
        let input = r#"[TOOL_CALLS][{"name":"get_weather","arguments":{"city":"London"}}]"#;
        let (text, calls) = parser().parse(input);
        assert!(text.is_none(), "no text expected");
        let calls = calls.expect("expected a call");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "get_weather");
        assert_eq!(calls[0].arguments, json!({"city": "London"}));
    }

    #[test]
    fn test_mistral_parse_pre_v11_multiple() {
        let input = r#"[TOOL_CALLS][{"name":"a","arguments":{"x":1}},{"name":"b","arguments":{"y":2}}]"#;
        let (_text, calls) = parser().parse(input);
        let calls = calls.expect("expected two calls");
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "a");
        assert_eq!(calls[1].name, "b");
    }

    #[test]
    fn test_mistral_parse_v11() {
        let input = r#"[TOOL_CALLS]get_weather{"city":"Paris"}"#;
        let (text, calls) = parser().parse(input);
        assert!(text.is_none());
        let calls = calls.expect("expected a call");
        assert_eq!(calls[0].name, "get_weather");
        assert_eq!(calls[0].arguments["city"], "Paris");
    }

    #[test]
    fn test_mistral_parse_with_content_before() {
        let input = r#"Sure thing! [TOOL_CALLS][{"name":"lookup","arguments":{"id":1}}]"#;
        let (text, calls) = parser().parse(input);
        assert_eq!(text.as_deref(), Some("Sure thing!"));
        assert!(calls.is_some());
    }

    #[test]
    fn test_mistral_no_tool_call() {
        let input = "Just a plain reply.";
        let (text, calls) = parser().parse(input);
        assert_eq!(text.as_deref(), Some("Just a plain reply."));
        assert!(calls.is_none());
    }

    #[test]
    fn test_mistral_format_tool_calls() {
        let calls = vec![ToolCall {
            id: "abc".to_owned(),
            name: "ping".to_owned(),
            arguments: json!({}),
        }];
        let output = parser().format_tool_calls(&calls);
        assert!(output.starts_with("[TOOL_CALLS]"));
        assert!(output.contains("\"ping\""));
    }

    #[test]
    fn test_mistral_format_tool_response() {
        let resp = ToolResponse {
            tool_call_id: "abc".to_owned(),
            name: "ping".to_owned(),
            content: json!("pong"),
        };
        let output = parser().format_tool_response(&resp);
        assert!(output.contains("abc"));
        assert!(output.contains("ping"));
    }
}
