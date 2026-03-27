use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::hermes::gen_call_id;
use super::{ParseResult, ToolCall, ToolCallParser, ToolResponse};

// ---------------------------------------------------------------------------
// DeepSeek token constants
// ---------------------------------------------------------------------------

const TOOL_CALLS_BEGIN: &str = "<\u{ff5c}tool\u{2581}calls\u{2581}begin\u{ff5c}>";
const TOOL_CALL_BEGIN: &str = "<\u{ff5c}tool\u{2581}call\u{2581}begin\u{ff5c}>";
const TOOL_SEP: &str = "<\u{ff5c}tool\u{2581}sep\u{ff5c}>";
const TOOL_CALL_END: &str = "<\u{ff5c}tool\u{2581}call\u{2581}end\u{ff5c}>";
const TOOL_CALLS_END: &str = "<\u{ff5c}tool\u{2581}calls\u{2581}end\u{ff5c}>";

// ---------------------------------------------------------------------------
// DeepSeek V3 — type<｜tool▁sep｜>name\n```json\n{args}\n```
// ---------------------------------------------------------------------------

/// Pattern: `<｜tool▁call▁begin｜>type<｜tool▁sep｜>name\n```json\n{args}\n```\n<｜tool▁call▁end｜>`
static DS_V3_CALL_RE: LazyLock<Regex> = LazyLock::new(|| {
    // type field before sep, then name, then ```json ... ```
    Regex::new(&format!(
        r"{}[^<]*{}(\S+)\s*```json\s*([\s\S]*?)```\s*{}",
        regex::escape(TOOL_CALL_BEGIN),
        regex::escape(TOOL_SEP),
        regex::escape(TOOL_CALL_END),
    ))
    .expect("deepseek_v3 call regex")
});

// ---------------------------------------------------------------------------
// DeepSeek V3.1 — name<｜tool▁sep｜>{args}
// ---------------------------------------------------------------------------

/// Pattern: `<｜tool▁call▁begin｜>name<｜tool▁sep｜>{args}<｜tool▁call▁end｜>`
static DS_V31_CALL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"{}(\S+?)\s*{}\s*([\s\S]*?)\s*{}",
        regex::escape(TOOL_CALL_BEGIN),
        regex::escape(TOOL_SEP),
        regex::escape(TOOL_CALL_END),
    ))
    .expect("deepseek_v31 call regex")
});

// ---------------------------------------------------------------------------
// Shared strip helper
// ---------------------------------------------------------------------------

/// Remove all DeepSeek tool-call section tokens and their contents from text.
fn strip_ds_tokens(text: &str) -> String {
    // Remove everything between tool_calls_begin and tool_calls_end.
    let begin = regex::escape(TOOL_CALLS_BEGIN);
    let end = regex::escape(TOOL_CALLS_END);
    let section_re =
        Regex::new(&format!(r"{}[\s\S]*?{}", begin, end)).expect("ds strip section re");

    let without_section = section_re.replace_all(text, "").to_string();

    // Also strip any stray per-call tokens.
    let token_re = Regex::new(&format!(
        r"({}|{}|{}|{}|{})",
        regex::escape(TOOL_CALLS_BEGIN),
        regex::escape(TOOL_CALL_BEGIN),
        regex::escape(TOOL_SEP),
        regex::escape(TOOL_CALL_END),
        regex::escape(TOOL_CALLS_END),
    ))
    .expect("ds strip token re");

    token_re.replace_all(&without_section, "").trim().to_owned()
}

// ---------------------------------------------------------------------------
// DeepSeekV3Parser
// ---------------------------------------------------------------------------

/// Parses the DeepSeek V3 tool-call format with `type` field and code fences.
#[derive(Debug, Clone)]
pub struct DeepSeekV3Parser;

impl ToolCallParser for DeepSeekV3Parser {
    fn names(&self) -> Vec<String> {
        vec!["deepseek_v3".to_owned()]
    }

    fn format_tool_calls(&self, calls: &[ToolCall]) -> String {
        let inner: String = calls
            .iter()
            .map(|c| {
                let args = serde_json::to_string_pretty(&c.arguments).unwrap_or_default();
                format!(
                    "{}function{}{}  \n```json\n{}\n```\n{}",
                    TOOL_CALL_BEGIN, TOOL_SEP, c.name, args, TOOL_CALL_END
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!("{}{}{}", TOOL_CALLS_BEGIN, inner, TOOL_CALLS_END)
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

        for cap in DS_V3_CALL_RE.captures_iter(text) {
            let name = cap[1].trim().to_owned();
            let json_raw = cap[2].trim();
            let arguments: Value = serde_json::from_str(json_raw)
                .unwrap_or_else(|_| Value::Object(serde_json::Map::new()));
            calls.push(ToolCall {
                id: gen_call_id(),
                name,
                arguments,
            });
        }

        let cleaned = strip_ds_tokens(text);
        // Also strip the whole-section pattern from what's left.
        let text_part = if cleaned.is_empty() {
            None
        } else {
            Some(cleaned)
        };
        let calls_part = if calls.is_empty() { None } else { Some(calls) };
        (text_part, calls_part)
    }

    fn clone_box(&self) -> Box<dyn ToolCallParser> {
        Box::new(self.clone())
    }
}

// ---------------------------------------------------------------------------
// DeepSeekV31Parser
// ---------------------------------------------------------------------------

/// Parses the DeepSeek V3.1 format — name before sep, raw JSON after.
#[derive(Debug, Clone)]
pub struct DeepSeekV31Parser;

impl ToolCallParser for DeepSeekV31Parser {
    fn names(&self) -> Vec<String> {
        vec!["deepseek_v3_1".to_owned(), "deepseek_v31".to_owned()]
    }

    fn format_tool_calls(&self, calls: &[ToolCall]) -> String {
        let inner: String = calls
            .iter()
            .map(|c| {
                let args = serde_json::to_string(&c.arguments).unwrap_or_default();
                format!(
                    "{}{}{}{}\n{}",
                    TOOL_CALL_BEGIN, c.name, TOOL_SEP, args, TOOL_CALL_END
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!("{}{}{}", TOOL_CALLS_BEGIN, inner, TOOL_CALLS_END)
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

        for cap in DS_V31_CALL_RE.captures_iter(text) {
            let name = cap[1].trim().to_owned();
            let json_raw = cap[2].trim();
            let arguments: Value = serde_json::from_str(json_raw)
                .unwrap_or_else(|_| Value::Object(serde_json::Map::new()));
            calls.push(ToolCall {
                id: gen_call_id(),
                name,
                arguments,
            });
        }

        let cleaned = strip_ds_tokens(text);
        let text_part = if cleaned.is_empty() {
            None
        } else {
            Some(cleaned)
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

    fn v3() -> DeepSeekV3Parser {
        DeepSeekV3Parser
    }

    fn v31() -> DeepSeekV31Parser {
        DeepSeekV31Parser
    }

    // Helper to build a V3 tool call string.
    fn make_v3_call(name: &str, args: &str) -> String {
        format!(
            "{}function{}{}  \n```json\n{}\n```\n{}",
            TOOL_CALL_BEGIN, TOOL_SEP, name, args, TOOL_CALL_END
        )
    }

    fn make_v31_call(name: &str, args: &str) -> String {
        format!("{}{}{}{}\n{}", TOOL_CALL_BEGIN, name, TOOL_SEP, args, TOOL_CALL_END)
    }

    // -- DeepSeek V3 ---------------------------------------------------------

    #[test]
    fn test_v3_parse_basic() {
        let call = make_v3_call("get_weather", r#"{"city":"Tokyo"}"#);
        let input = format!("{}{}{}", TOOL_CALLS_BEGIN, call, TOOL_CALLS_END);
        let (text, calls) = v3().parse(&input);
        assert!(text.is_none(), "no text expected");
        let calls = calls.expect("expected a call");
        assert_eq!(calls[0].name, "get_weather");
        assert_eq!(calls[0].arguments, json!({"city": "Tokyo"}));
    }

    #[test]
    fn test_v3_parse_with_content_before() {
        let call = make_v3_call("search", r#"{"q":"rust"}"#);
        let input = format!("Searching now...\n{}{}{}", TOOL_CALLS_BEGIN, call, TOOL_CALLS_END);
        let (text, calls) = v3().parse(&input);
        assert!(calls.is_some());
        let text = text.expect("expected text");
        assert!(text.contains("Searching now"));
    }

    #[test]
    fn test_v3_no_tool_call() {
        let input = "Just a reply.";
        let (text, calls) = v3().parse(input);
        assert_eq!(text.as_deref(), Some("Just a reply."));
        assert!(calls.is_none());
    }

    #[test]
    fn test_v3_format_round_trip() {
        let calls = vec![ToolCall {
            id: "id1".to_owned(),
            name: "ping".to_owned(),
            arguments: json!({}),
        }];
        let formatted = v3().format_tool_calls(&calls);
        let (_, parsed) = v3().parse(&formatted);
        let parsed = parsed.expect("should parse back");
        assert_eq!(parsed[0].name, "ping");
    }

    #[test]
    fn test_v3_names() {
        assert_eq!(v3().names(), vec!["deepseek_v3"]);
    }

    // -- DeepSeek V3.1 -------------------------------------------------------

    #[test]
    fn test_v31_parse_basic() {
        let call = make_v31_call("get_weather", r#"{"city":"Berlin"}"#);
        let input = format!("{}{}{}", TOOL_CALLS_BEGIN, call, TOOL_CALLS_END);
        let (text, calls) = v31().parse(&input);
        assert!(text.is_none());
        let calls = calls.expect("expected a call");
        assert_eq!(calls[0].name, "get_weather");
        assert_eq!(calls[0].arguments["city"], "Berlin");
    }

    #[test]
    fn test_v31_parse_with_content_before() {
        let call = make_v31_call("lookup", r#"{"id":7}"#);
        let input = format!("OK!\n{}{}{}", TOOL_CALLS_BEGIN, call, TOOL_CALLS_END);
        let (text, calls) = v31().parse(&input);
        assert!(calls.is_some());
        assert!(text.as_deref().unwrap_or("").contains("OK!"));
    }

    #[test]
    fn test_v31_no_tool_call() {
        let input = "Hello there.";
        let (text, calls) = v31().parse(input);
        assert_eq!(text.as_deref(), Some("Hello there."));
        assert!(calls.is_none());
    }

    #[test]
    fn test_v31_format_round_trip() {
        let calls = vec![ToolCall {
            id: "id2".to_owned(),
            name: "echo".to_owned(),
            arguments: json!({"msg": "hi"}),
        }];
        let formatted = v31().format_tool_calls(&calls);
        let (_, parsed) = v31().parse(&formatted);
        let parsed = parsed.expect("should parse back");
        assert_eq!(parsed[0].name, "echo");
        assert_eq!(parsed[0].arguments["msg"], "hi");
    }

    #[test]
    fn test_v31_names() {
        let names = v31().names();
        assert!(names.contains(&"deepseek_v3_1".to_owned()));
        assert!(names.contains(&"deepseek_v31".to_owned()));
    }
}
