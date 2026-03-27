use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::hermes::gen_call_id;
use super::{ParseResult, ToolCall, ToolCallParser, ToolResponse};

// ---------------------------------------------------------------------------
// Qwen3-Coder format constants
// ---------------------------------------------------------------------------

// Format: `<tool_call><function=name><parameter=key>value</parameter></function></tool_call>`

// ---------------------------------------------------------------------------
// Compiled patterns
// ---------------------------------------------------------------------------

/// Outermost: capture everything inside `<tool_call>…</tool_call>`.
static OUTER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<tool_call>([\s\S]*?)</tool_call>").expect("qwen3_coder outer regex")
});

/// Function block: `<function=name>…</function>`.
static FUNC_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<function=([^>]+)>([\s\S]*?)</function>").expect("qwen3_coder func regex")
});

/// Parameter: `<parameter=key>value</parameter>`.
static PARAM_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<parameter=([^>]+)>([\s\S]*?)</parameter>").expect("qwen3_coder param regex")
});

// ---------------------------------------------------------------------------
// Value conversion
// ---------------------------------------------------------------------------

/// Try JSON parse; convert the string `"null"` to `Value::Null`;
/// fall back to raw string on any other failure.
fn convert_param_value(s: &str) -> Value {
    let trimmed = s.trim();
    if trimmed == "null" {
        return Value::Null;
    }
    serde_json::from_str(trimmed).unwrap_or_else(|_| Value::String(trimmed.to_owned()))
}

// ---------------------------------------------------------------------------
// Qwen3CoderParser
// ---------------------------------------------------------------------------

/// Parses the Qwen3-Coder nested XML tool-call format.
#[derive(Debug, Clone)]
pub struct Qwen3CoderParser;

impl ToolCallParser for Qwen3CoderParser {
    fn names(&self) -> Vec<String> {
        vec!["qwen3_coder".to_owned()]
    }

    fn format_tool_calls(&self, calls: &[ToolCall]) -> String {
        calls
            .iter()
            .map(|c| {
                let params: String = if let Value::Object(map) = &c.arguments {
                    map.iter()
                        .map(|(k, v)| {
                            let val_str = match v {
                                Value::String(s) => s.clone(),
                                Value::Null => "null".to_owned(),
                                other => other.to_string(),
                            };
                            format!("<parameter={}>{}</parameter>", k, val_str)
                        })
                        .collect::<Vec<_>>()
                        .join("")
                } else {
                    String::new()
                };
                format!(
                    "<tool_call><function={}>{}</function></tool_call>",
                    c.name, params
                )
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
        let mut calls = Vec::new();

        for outer_cap in OUTER_RE.captures_iter(text) {
            let body = &outer_cap[1];
            if let Some(func_cap) = FUNC_RE.captures(body) {
                let name = func_cap[1].trim().to_owned();
                let params_body = &func_cap[2];

                let mut args = serde_json::Map::new();
                for param_cap in PARAM_RE.captures_iter(params_body) {
                    let key = param_cap[1].trim().to_owned();
                    let val = convert_param_value(&param_cap[2]);
                    args.insert(key, val);
                }

                calls.push(ToolCall {
                    id: gen_call_id(),
                    name,
                    arguments: Value::Object(args),
                });
            }
        }

        let cleaned = OUTER_RE.replace_all(text, "").trim().to_owned();
        let text_part = if cleaned.is_empty() { None } else { Some(cleaned) };
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

    fn parser() -> Qwen3CoderParser {
        Qwen3CoderParser
    }

    #[test]
    fn test_qwen3_parse_basic() {
        let input = "<tool_call><function=get_weather><parameter=city>Paris</parameter></function></tool_call>";
        let (text, calls) = parser().parse(input);
        assert!(text.is_none(), "no text expected");
        let calls = calls.expect("expected a call");
        assert_eq!(calls[0].name, "get_weather");
        assert_eq!(calls[0].arguments["city"], "Paris");
    }

    #[test]
    fn test_qwen3_parse_numeric_parameter() {
        let input =
            "<tool_call><function=search><parameter=limit>10</parameter></function></tool_call>";
        let (_text, calls) = parser().parse(input);
        let calls = calls.expect("expected a call");
        assert_eq!(calls[0].arguments["limit"], json!(10));
    }

    #[test]
    fn test_qwen3_parse_null_parameter() {
        let input =
            "<tool_call><function=clear><parameter=value>null</parameter></function></tool_call>";
        let (_text, calls) = parser().parse(input);
        let calls = calls.expect("expected a call");
        assert_eq!(calls[0].arguments["value"], Value::Null);
    }

    #[test]
    fn test_qwen3_parse_with_content_before() {
        let input = "I'll look that up.\n<tool_call><function=lookup><parameter=id>7</parameter></function></tool_call>";
        let (text, calls) = parser().parse(input);
        assert!(calls.is_some());
        assert!(text.as_deref().unwrap_or("").contains("I'll look that up"));
    }

    #[test]
    fn test_qwen3_no_tool_call() {
        let input = "Just a plain reply.";
        let (text, calls) = parser().parse(input);
        assert_eq!(text.as_deref(), Some("Just a plain reply."));
        assert!(calls.is_none());
    }

    #[test]
    fn test_qwen3_format_tool_calls() {
        let calls = vec![ToolCall {
            id: "id1".to_owned(),
            name: "ping".to_owned(),
            arguments: json!({"host": "localhost"}),
        }];
        let output = parser().format_tool_calls(&calls);
        assert!(output.contains("<tool_call>"));
        assert!(output.contains("<function=ping>"));
        assert!(output.contains("<parameter=host>localhost</parameter>"));
        assert!(output.contains("</function>"));
        assert!(output.contains("</tool_call>"));
    }

    #[test]
    fn test_qwen3_format_round_trip() {
        let calls = vec![ToolCall {
            id: "id2".to_owned(),
            name: "echo".to_owned(),
            arguments: json!({"msg": "hello"}),
        }];
        let formatted = parser().format_tool_calls(&calls);
        let (_text, parsed) = parser().parse(&formatted);
        let parsed = parsed.expect("should parse back");
        assert_eq!(parsed[0].name, "echo");
        assert_eq!(parsed[0].arguments["msg"], "hello");
    }

    #[test]
    fn test_qwen3_format_null_value() {
        let calls = vec![ToolCall {
            id: "id3".to_owned(),
            name: "clear".to_owned(),
            arguments: json!({"value": null}),
        }];
        let formatted = parser().format_tool_calls(&calls);
        let (_text, parsed) = parser().parse(&formatted);
        let parsed = parsed.expect("should parse back");
        assert_eq!(parsed[0].arguments["value"], Value::Null);
    }

    #[test]
    fn test_qwen3_names() {
        assert_eq!(parser().names(), vec!["qwen3_coder"]);
    }

    #[test]
    fn test_qwen3_format_tool_response() {
        let resp = ToolResponse {
            tool_call_id: "r1".to_owned(),
            name: "ping".to_owned(),
            content: json!("pong"),
        };
        let output = parser().format_tool_response(&resp);
        assert!(output.contains("r1"));
        assert!(output.contains("ping"));
    }
}
