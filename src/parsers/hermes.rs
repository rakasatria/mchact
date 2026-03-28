use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;
use uuid::Uuid;

use super::{ParseResult, ToolCall, ToolCallParser, ToolResponse};

// ---------------------------------------------------------------------------
// Compiled patterns (initialised once)
// ---------------------------------------------------------------------------

static HERMES_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<tool_call>([\s\S]*?)(?:</tool_call>|$)").expect("hermes regex")
});

static LONGCAT_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<longcat_tool_call>([\s\S]*?)(?:</longcat_tool_call>|$)").expect("longcat regex")
});

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Generate a short UUID-based call id.
pub fn gen_call_id() -> String {
    format!("call_{}", &Uuid::new_v4().to_string().replace('-', "")[..12])
}

/// Parse model output using the given compiled `pattern`.
///
/// Each capture is expected to be a JSON object with at minimum a `name` key.
/// An optional `arguments` key holds the tool arguments.
pub fn parse_with_pattern(text: &str, pattern: &Regex) -> ParseResult {
    let mut calls: Vec<ToolCall> = Vec::new();

    for cap in pattern.captures_iter(text) {
        let raw = cap[1].trim();
        if raw.is_empty() {
            continue;
        }
        let json: Value = match serde_json::from_str(raw) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let name = match json.get("name").and_then(Value::as_str) {
            Some(n) => n.to_owned(),
            None => continue,
        };
        let arguments = json
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| Value::Object(serde_json::Map::new()));

        calls.push(ToolCall {
            id: gen_call_id(),
            name,
            arguments,
        });
    }

    // Strip tag regions from the text to produce the plain-text portion.
    let cleaned = pattern.replace_all(text, "").trim().to_owned();
    let text_part = if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    };
    let calls_part = if calls.is_empty() { None } else { Some(calls) };

    (text_part, calls_part)
}

/// Serialise a slice of `ToolCall`s into `<TAG>{json}</TAG>` blocks.
pub fn format_calls_with_tag(calls: &[ToolCall], tag: &str) -> String {
    calls
        .iter()
        .map(|c| {
            let obj = serde_json::json!({
                "name": c.name,
                "arguments": c.arguments,
            });
            format!("<{tag}>{}</{tag}>", obj)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Serialise a `ToolResponse` into a `<TAG_result>{json}</TAG_result>` block.
pub fn format_tool_response_with_tag(response: &ToolResponse, tag: &str) -> String {
    let obj = serde_json::json!({
        "tool_call_id": response.tool_call_id,
        "name": response.name,
        "content": response.content,
    });
    format!("<{tag}_result>{}</{tag}_result>", obj)
}

// ---------------------------------------------------------------------------
// HermesParser
// ---------------------------------------------------------------------------

/// Parses the `<tool_call>…</tool_call>` format used by Hermes-family models.
#[derive(Debug, Clone)]
pub struct HermesParser;

impl ToolCallParser for HermesParser {
    fn names(&self) -> Vec<String> {
        vec!["hermes".to_owned()]
    }

    fn format_tool_calls(&self, calls: &[ToolCall]) -> String {
        format_calls_with_tag(calls, "tool_call")
    }

    fn format_tool_response(&self, response: &ToolResponse) -> String {
        format_tool_response_with_tag(response, "tool_call")
    }

    fn parse(&self, text: &str) -> ParseResult {
        parse_with_pattern(text, &HERMES_PATTERN)
    }

    fn clone_box(&self) -> Box<dyn ToolCallParser> {
        Box::new(self.clone())
    }
}

// ---------------------------------------------------------------------------
// LongcatParser
// ---------------------------------------------------------------------------

/// Parses the `<longcat_tool_call>…</longcat_tool_call>` format.
#[derive(Debug, Clone)]
pub struct LongcatParser;

impl ToolCallParser for LongcatParser {
    fn names(&self) -> Vec<String> {
        vec!["longcat".to_owned()]
    }

    fn format_tool_calls(&self, calls: &[ToolCall]) -> String {
        format_calls_with_tag(calls, "longcat_tool_call")
    }

    fn format_tool_response(&self, response: &ToolResponse) -> String {
        format_tool_response_with_tag(response, "longcat_tool_call")
    }

    fn parse(&self, text: &str) -> ParseResult {
        parse_with_pattern(text, &LONGCAT_PATTERN)
    }

    fn clone_box(&self) -> Box<dyn ToolCallParser> {
        Box::new(self.clone())
    }
}

// ---------------------------------------------------------------------------
// QwenParser  (delegates to HermesParser)
// ---------------------------------------------------------------------------

/// Thin wrapper around `HermesParser` for Qwen-family models, which use the
/// same `<tool_call>` format.
#[derive(Debug, Clone)]
pub struct QwenParser(HermesParser);

impl QwenParser {
    pub fn new() -> Self {
        Self(HermesParser)
    }
}

impl Default for QwenParser {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolCallParser for QwenParser {
    fn names(&self) -> Vec<String> {
        vec!["qwen".to_owned()]
    }

    fn format_tool_calls(&self, calls: &[ToolCall]) -> String {
        self.0.format_tool_calls(calls)
    }

    fn format_tool_response(&self, response: &ToolResponse) -> String {
        self.0.format_tool_response(response)
    }

    fn parse(&self, text: &str) -> ParseResult {
        self.0.parse(text)
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
    use crate::parsers::{ParserRegistry, ToolCall, ToolResponse};

    fn hermes() -> HermesParser {
        HermesParser
    }

    // -- parse tests --------------------------------------------------------

    #[test]
    fn test_hermes_parse_single_tool_call() {
        let input = r#"<tool_call>{"name":"get_weather","arguments":{"city":"London"}}</tool_call>"#;
        let (text, calls) = hermes().parse(input);
        assert!(text.is_none(), "no text expected, got: {text:?}");
        let calls = calls.expect("expected one call");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "get_weather");
        assert_eq!(calls[0].arguments, json!({"city": "London"}));
    }

    #[test]
    fn test_hermes_parse_multiple_tool_calls() {
        let input = concat!(
            r#"<tool_call>{"name":"tool_a","arguments":{"x":1}}</tool_call>"#,
            "\n",
            r#"<tool_call>{"name":"tool_b","arguments":{"y":2}}</tool_call>"#,
        );
        let (text, calls) = hermes().parse(input);
        assert!(text.is_none());
        let calls = calls.expect("expected two calls");
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "tool_a");
        assert_eq!(calls[1].name, "tool_b");
    }

    #[test]
    fn test_hermes_parse_unclosed_tag() {
        // Unclosed tag — the `$` anchor in the regex still captures the content.
        let input = r#"<tool_call>{"name":"incomplete","arguments":{}}"#;
        let (_, calls) = hermes().parse(input);
        let calls = calls.expect("expected a call from unclosed tag");
        assert_eq!(calls[0].name, "incomplete");
    }

    #[test]
    fn test_hermes_parse_no_tool_calls() {
        let input = "Just a plain message with no tool calls.";
        let (text, calls) = hermes().parse(input);
        assert_eq!(text.as_deref(), Some("Just a plain message with no tool calls."));
        assert!(calls.is_none());
    }

    // -- format tests -------------------------------------------------------

    #[test]
    fn test_hermes_format_tool_calls() {
        let calls = vec![ToolCall {
            id: "call_abc".to_owned(),
            name: "search".to_owned(),
            arguments: json!({"query": "rust async"}),
        }];
        let output = hermes().format_tool_calls(&calls);
        assert!(output.contains("<tool_call>"), "missing opening tag");
        assert!(output.contains("</tool_call>"), "missing closing tag");
        assert!(output.contains("\"search\""), "missing tool name");
        assert!(output.contains("rust async"), "missing argument value");
    }

    #[test]
    fn test_hermes_format_tool_response() {
        let response = ToolResponse {
            tool_call_id: "call_abc".to_owned(),
            name: "search".to_owned(),
            content: json!({"results": []}),
        };
        let output = hermes().format_tool_response(&response);
        assert!(output.contains("<tool_call_result>"), "missing result tag");
        assert!(output.contains("call_abc"), "missing call id");
    }

    // -- longcat ------------------------------------------------------------

    #[test]
    fn test_longcat_parse() {
        let input =
            r#"<longcat_tool_call>{"name":"run_code","arguments":{"lang":"python"}}</longcat_tool_call>"#;
        let (text, calls) = LongcatParser.parse(input);
        assert!(text.is_none());
        let calls = calls.expect("expected one call");
        assert_eq!(calls[0].name, "run_code");
        assert_eq!(calls[0].arguments["lang"], "python");
    }

    // -- qwen delegation ----------------------------------------------------

    #[test]
    fn test_qwen_delegates_to_hermes() {
        let input = r#"<tool_call>{"name":"add","arguments":{"a":1,"b":2}}</tool_call>"#;
        let qwen = QwenParser::new();
        let hermes_result = hermes().parse(input);
        let qwen_result = qwen.parse(input);
        // IDs are random; compare name + arguments only.
        let h_calls = hermes_result.1.unwrap();
        let q_calls = qwen_result.1.unwrap();
        assert_eq!(h_calls[0].name, q_calls[0].name);
        assert_eq!(h_calls[0].arguments, q_calls[0].arguments);
    }

    // -- registry -----------------------------------------------------------

    #[test]
    fn test_parser_registry() {
        let mut registry = ParserRegistry::new();
        registry.register(Box::new(HermesParser));
        registry.register(Box::new(LongcatParser));
        registry.register(Box::new(QwenParser::new()));

        let names = registry.available_names();
        assert!(names.contains(&"hermes".to_owned()));
        assert!(names.contains(&"longcat".to_owned()));
        assert!(names.contains(&"qwen".to_owned()));

        let parser = registry.get("hermes").expect("hermes not found");
        let input =
            r#"<tool_call>{"name":"ping","arguments":{}}</tool_call>"#;
        let (_, calls) = parser.parse(input);
        assert_eq!(calls.unwrap()[0].name, "ping");
    }
}
