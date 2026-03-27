use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::hermes::gen_call_id;
use super::{ParseResult, ToolCall, ToolCallParser, ToolResponse};

// ---------------------------------------------------------------------------
// Kimi K2 token constants
// ---------------------------------------------------------------------------

/// Section begin: plural form (primary).
const SECTION_BEGIN_PLURAL: &str = "<|tool_calls_section_begin|>";
/// Section begin: singular form (alternate).
const SECTION_BEGIN_SINGULAR: &str = "<|tool_call_section_begin|>";
const SECTION_END: &str = "<|tool_calls_section_end|>";
const CALL_BEGIN: &str = "<|tool_call_begin|>";
const ARG_BEGIN: &str = "<|tool_call_argument_begin|>";
const CALL_END: &str = "<|tool_call_end|>";

// ---------------------------------------------------------------------------
// Compiled patterns
// ---------------------------------------------------------------------------

/// Matches a single Kimi K2 tool call:
/// `<|tool_call_begin|>functions.name:index<|tool_call_argument_begin|>{args}<|tool_call_end|>`
static KIMI_CALL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"{}\s*([\w.]+:\d+)\s*{}\s*([\s\S]*?)\s*{}",
        regex::escape(CALL_BEGIN),
        regex::escape(ARG_BEGIN),
        regex::escape(CALL_END),
    ))
    .expect("kimi_k2 call regex")
});

/// Matches the full section (either plural or singular open token).
static KIMI_SECTION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"({}|{})[\s\S]*?{}",
        regex::escape(SECTION_BEGIN_PLURAL),
        regex::escape(SECTION_BEGIN_SINGULAR),
        regex::escape(SECTION_END),
    ))
    .expect("kimi_k2 section regex")
});

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract function name from a dotted ID like `"functions.get_weather:0"` → `"get_weather"`.
fn extract_function_name(dotted_id: &str) -> String {
    // Strip optional trailing `:index`.
    let without_index = if let Some(pos) = dotted_id.rfind(':') {
        &dotted_id[..pos]
    } else {
        dotted_id
    };
    // Return the part after the last `.`.
    if let Some(pos) = without_index.rfind('.') {
        without_index[pos + 1..].to_owned()
    } else {
        without_index.to_owned()
    }
}

// ---------------------------------------------------------------------------
// KimiK2Parser
// ---------------------------------------------------------------------------

/// Parses Kimi K2 model tool-call format.
#[derive(Debug, Clone)]
pub struct KimiK2Parser;

impl ToolCallParser for KimiK2Parser {
    fn names(&self) -> Vec<String> {
        vec!["kimi_k2".to_owned()]
    }

    fn format_tool_calls(&self, calls: &[ToolCall]) -> String {
        let inner: String = calls
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let args = serde_json::to_string(&c.arguments).unwrap_or_default();
                format!(
                    "{}functions.{}:{}{}{}{}\n",
                    CALL_BEGIN, c.name, i, ARG_BEGIN, args, CALL_END
                )
            })
            .collect();
        format!("{}{}{}", SECTION_BEGIN_PLURAL, inner, SECTION_END)
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

        for cap in KIMI_CALL_RE.captures_iter(text) {
            let dotted_id = cap[1].trim();
            let name = extract_function_name(dotted_id);
            let json_raw = cap[2].trim();
            let arguments: Value = serde_json::from_str(json_raw)
                .unwrap_or_else(|_| Value::Object(serde_json::Map::new()));
            calls.push(ToolCall {
                id: gen_call_id(),
                name,
                arguments,
            });
        }

        // Remove the tool-call section from the visible text.
        let cleaned = KIMI_SECTION_RE.replace_all(text, "").trim().to_owned();
        // Also remove any stray tokens not caught by the section regex.
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

    fn parser() -> KimiK2Parser {
        KimiK2Parser
    }

    #[test]
    fn test_kimi_parse_basic() {
        let input = format!(
            "{}{}functions.get_weather:0{}{}{}{}\n{}",
            SECTION_BEGIN_PLURAL,
            CALL_BEGIN,
            ARG_BEGIN,
            r#"{"city":"Paris"}"#,
            CALL_END,
            SECTION_END,
            "",
        );
        let (text, calls) = parser().parse(&input);
        assert!(text.is_none(), "no text expected");
        let calls = calls.expect("expected a call");
        assert_eq!(calls[0].name, "get_weather");
        assert_eq!(calls[0].arguments, json!({"city": "Paris"}));
    }

    #[test]
    fn test_kimi_singular_section_begin() {
        // Alternate singular token.
        let input = format!(
            "{}{}functions.search:0{}{}{}{}\n{}",
            SECTION_BEGIN_SINGULAR,
            CALL_BEGIN,
            ARG_BEGIN,
            r#"{"q":"rust"}"#,
            CALL_END,
            SECTION_END,
            "",
        );
        let (_text, calls) = parser().parse(&input);
        let calls = calls.expect("expected a call");
        assert_eq!(calls[0].name, "search");
    }

    #[test]
    fn test_kimi_parse_with_content_before() {
        let call_section = format!(
            "{}{}functions.ping:0{}{}{}{}",
            SECTION_BEGIN_PLURAL, CALL_BEGIN, ARG_BEGIN, r#"{}"#, CALL_END, SECTION_END,
        );
        let input = format!("OK, calling now.\n{}", call_section);
        let (text, calls) = parser().parse(&input);
        assert!(calls.is_some());
        assert!(text.as_deref().unwrap_or("").contains("OK, calling now"));
    }

    #[test]
    fn test_kimi_no_tool_call() {
        let input = "Hello!";
        let (text, calls) = parser().parse(input);
        assert_eq!(text.as_deref(), Some("Hello!"));
        assert!(calls.is_none());
    }

    #[test]
    fn test_kimi_format_round_trip() {
        let calls = vec![ToolCall {
            id: "id1".to_owned(),
            name: "lookup".to_owned(),
            arguments: json!({"id": 5}),
        }];
        let formatted = parser().format_tool_calls(&calls);
        let (_text, parsed) = parser().parse(&formatted);
        let parsed = parsed.expect("should parse back");
        assert_eq!(parsed[0].name, "lookup");
        assert_eq!(parsed[0].arguments["id"], 5);
    }

    #[test]
    fn test_kimi_extract_function_name() {
        assert_eq!(extract_function_name("functions.get_weather:0"), "get_weather");
        assert_eq!(extract_function_name("functions.search:1"), "search");
        assert_eq!(extract_function_name("get_weather"), "get_weather");
    }

    #[test]
    fn test_kimi_names() {
        assert_eq!(parser().names(), vec!["kimi_k2"]);
    }
}
