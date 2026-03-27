use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::hermes::gen_call_id;
use super::{ParseResult, ToolCall, ToolCallParser, ToolResponse};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Try to deserialise `s` as JSON; fall back to a JSON string on failure.
fn parse_value_or_string(s: &str) -> Value {
    serde_json::from_str(s).unwrap_or_else(|_| Value::String(s.to_owned()))
}

// ---------------------------------------------------------------------------
// GLM 4.5 patterns
// ---------------------------------------------------------------------------

/// `<tool_call>func_name\n<arg_key>k</arg_key><arg_value>v</arg_value>\n</tool_call>`
static GLM45_CALL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<tool_call>(\w+)\n([\s\S]*?)</tool_call>").expect("glm45 call regex")
});

static GLM45_PAIR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<arg_key>([^<]*)</arg_key><arg_value>([\s\S]*?)</arg_value>")
        .expect("glm45 pair regex")
});

// ---------------------------------------------------------------------------
// GLM 4.7 patterns (flexible newline between </arg_key> and <arg_value>)
// ---------------------------------------------------------------------------

static GLM47_CALL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<tool_call>(\w+)\n([\s\S]*?)</tool_call>").expect("glm47 call regex")
});

static GLM47_PAIR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<arg_key>([^<]*)</arg_key>\s*<arg_value>([\s\S]*?)</arg_value>")
        .expect("glm47 pair regex")
});

// ---------------------------------------------------------------------------
// Shared parse core
// ---------------------------------------------------------------------------

fn parse_glm_calls(text: &str, call_re: &Regex, pair_re: &Regex) -> Vec<ToolCall> {
    let mut calls = Vec::new();

    for cap in call_re.captures_iter(text) {
        let name = cap[1].trim().to_owned();
        let body = &cap[2];

        let mut args = serde_json::Map::new();
        for pair in pair_re.captures_iter(body) {
            let key = pair[1].trim().to_owned();
            let val = parse_value_or_string(pair[2].trim());
            args.insert(key, val);
        }

        calls.push(ToolCall {
            id: gen_call_id(),
            name,
            arguments: Value::Object(args),
        });
    }

    calls
}

/// Emit a single tool call in GLM key-value XML format.
fn format_glm_call(call: &ToolCall) -> String {
    let pairs: String = if let Value::Object(map) = &call.arguments {
        map.iter()
            .map(|(k, v)| {
                let val_str = match v {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                format!("<arg_key>{}</arg_key><arg_value>{}</arg_value>", k, val_str)
            })
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        String::new()
    };
    format!("<tool_call>{}\n{}\n</tool_call>", call.name, pairs)
}

fn format_glm_response(response: &ToolResponse) -> String {
    let obj = serde_json::json!({
        "tool_call_id": response.tool_call_id,
        "name": response.name,
        "content": response.content,
    });
    format!("<tool_response>{}</tool_response>", obj)
}

// ---------------------------------------------------------------------------
// Glm45Parser
// ---------------------------------------------------------------------------

/// Parses GLM 4.5 `<tool_call>` format with strict `</arg_key><arg_value>` adjacency.
#[derive(Debug, Clone)]
pub struct Glm45Parser;

impl ToolCallParser for Glm45Parser {
    fn names(&self) -> Vec<String> {
        vec!["glm45".to_owned()]
    }

    fn format_tool_calls(&self, calls: &[ToolCall]) -> String {
        calls
            .iter()
            .map(format_glm_call)
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn format_tool_response(&self, response: &ToolResponse) -> String {
        format_glm_response(response)
    }

    fn parse(&self, text: &str) -> ParseResult {
        let calls = parse_glm_calls(text, &GLM45_CALL_RE, &GLM45_PAIR_RE);
        let cleaned = GLM45_CALL_RE.replace_all(text, "").trim().to_owned();
        let text_part = if cleaned.is_empty() { None } else { Some(cleaned) };
        let calls_part = if calls.is_empty() { None } else { Some(calls) };
        (text_part, calls_part)
    }

    fn clone_box(&self) -> Box<dyn ToolCallParser> {
        Box::new(self.clone())
    }
}

// ---------------------------------------------------------------------------
// Glm47Parser
// ---------------------------------------------------------------------------

/// Parses GLM 4.7 `<tool_call>` format with flexible whitespace between arg tags.
#[derive(Debug, Clone)]
pub struct Glm47Parser;

impl ToolCallParser for Glm47Parser {
    fn names(&self) -> Vec<String> {
        vec!["glm47".to_owned()]
    }

    fn format_tool_calls(&self, calls: &[ToolCall]) -> String {
        calls
            .iter()
            .map(format_glm_call)
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn format_tool_response(&self, response: &ToolResponse) -> String {
        format_glm_response(response)
    }

    fn parse(&self, text: &str) -> ParseResult {
        let calls = parse_glm_calls(text, &GLM47_CALL_RE, &GLM47_PAIR_RE);
        let cleaned = GLM47_CALL_RE.replace_all(text, "").trim().to_owned();
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

    fn glm45() -> Glm45Parser {
        Glm45Parser
    }

    fn glm47() -> Glm47Parser {
        Glm47Parser
    }

    // -- GLM 4.5 -------------------------------------------------------------

    #[test]
    fn test_glm45_parse_basic() {
        let input = "<tool_call>get_weather\n<arg_key>city</arg_key><arg_value>London</arg_value>\n</tool_call>";
        let (text, calls) = glm45().parse(input);
        assert!(text.is_none(), "no text expected");
        let calls = calls.expect("expected a call");
        assert_eq!(calls[0].name, "get_weather");
        assert_eq!(calls[0].arguments["city"], "London");
    }

    #[test]
    fn test_glm45_parse_json_value() {
        let input = "<tool_call>search\n<arg_key>limit</arg_key><arg_value>10</arg_value>\n</tool_call>";
        let (_text, calls) = glm45().parse(input);
        let calls = calls.expect("expected a call");
        // Numeric value should be parsed as JSON number.
        assert_eq!(calls[0].arguments["limit"], json!(10));
    }

    #[test]
    fn test_glm45_parse_with_content_before() {
        let input =
            "Let me check that.\n<tool_call>lookup\n<arg_key>id</arg_key><arg_value>42</arg_value>\n</tool_call>";
        let (text, calls) = glm45().parse(input);
        assert!(calls.is_some());
        assert!(text.as_deref().unwrap_or("").contains("Let me check"));
    }

    #[test]
    fn test_glm45_no_tool_call() {
        let input = "Hello world.";
        let (text, calls) = glm45().parse(input);
        assert_eq!(text.as_deref(), Some("Hello world."));
        assert!(calls.is_none());
    }

    #[test]
    fn test_glm45_format_tool_calls() {
        let calls = vec![ToolCall {
            id: "x".to_owned(),
            name: "ping".to_owned(),
            arguments: json!({"host": "localhost"}),
        }];
        let output = glm45().format_tool_calls(&calls);
        assert!(output.contains("<tool_call>ping"));
        assert!(output.contains("<arg_key>host</arg_key>"));
        assert!(output.contains("<arg_value>localhost</arg_value>"));
    }

    // -- GLM 4.7 -------------------------------------------------------------

    #[test]
    fn test_glm47_parse_with_newline_between_tags() {
        // GLM 4.7 allows a newline between </arg_key> and <arg_value>
        let input =
            "<tool_call>get_weather\n<arg_key>city</arg_key>\n<arg_value>Tokyo</arg_value>\n</tool_call>";
        let (_text, calls) = glm47().parse(input);
        let calls = calls.expect("expected a call");
        assert_eq!(calls[0].name, "get_weather");
        assert_eq!(calls[0].arguments["city"], "Tokyo");
    }

    #[test]
    fn test_glm47_no_tool_call() {
        let input = "Just a reply.";
        let (text, calls) = glm47().parse(input);
        assert_eq!(text.as_deref(), Some("Just a reply."));
        assert!(calls.is_none());
    }

    #[test]
    fn test_glm47_format_tool_response() {
        let resp = ToolResponse {
            tool_call_id: "r1".to_owned(),
            name: "ping".to_owned(),
            content: json!("pong"),
        };
        let output = glm47().format_tool_response(&resp);
        assert!(output.contains("r1"));
        assert!(output.contains("ping"));
    }

    #[test]
    fn test_glm47_names() {
        assert_eq!(glm47().names(), vec!["glm47"]);
    }

    #[test]
    fn test_glm45_names() {
        assert_eq!(glm45().names(), vec!["glm45"]);
    }
}
