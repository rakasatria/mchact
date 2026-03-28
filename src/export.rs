use std::io::{BufRead, BufWriter, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::parsers::{ParserRegistry, ToolCall, ToolCallParser, ToolResponse};

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareGptTurn {
    pub from: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryEntry {
    pub prompt_index: Option<u64>,
    pub messages: Vec<serde_json::Value>,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(default)]
    pub completed: bool,
    #[serde(default)]
    pub partial: bool,
    #[serde(default)]
    pub api_calls: u64,
    #[serde(default)]
    pub toolsets_used: Vec<String>,
    #[serde(default)]
    pub tool_stats: serde_json::Value,
    #[serde(default)]
    pub tool_error_counts: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareGptEntry {
    pub prompt_index: Option<u64>,
    pub conversations: Vec<ShareGptTurn>,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(default)]
    pub completed: bool,
    #[serde(default)]
    pub partial: bool,
    #[serde(default)]
    pub api_calls: u64,
    #[serde(default)]
    pub toolsets_used: Vec<String>,
    #[serde(default)]
    pub tool_stats: serde_json::Value,
    #[serde(default)]
    pub tool_error_counts: serde_json::Value,
}

#[derive(Debug, Default)]
pub struct ExportStats {
    pub total: u64,
    pub exported: u64,
    pub filtered: u64,
}

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

/// Extract text content from an OpenAI message's `content` field.
/// Content may be a string or an array of content blocks.
fn extract_text_content(content: &serde_json::Value) -> String {
    match content {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(blocks) => {
            let parts: Vec<String> = blocks
                .iter()
                .filter_map(|block| {
                    if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                        block.get("text").and_then(|t| t.as_str()).map(String::from)
                    } else {
                        None
                    }
                })
                .collect();
            parts.join("")
        }
        _ => String::new(),
    }
}

/// Extract the native `reasoning` field if present (DeepSeek-style).
fn extract_reasoning(msg: &serde_json::Value) -> Option<String> {
    msg.get("reasoning")
        .or_else(|| msg.get("reasoning_content"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from)
}

/// Replace `<REASONING_SCRATCHPAD>` XML tags with `<think>` tags in a string.
fn replace_scratchpad_tags(text: &str) -> String {
    text.replace("<REASONING_SCRATCHPAD>", "<think>")
        .replace("</REASONING_SCRATCHPAD>", "</think>")
}

/// Parse a single OpenAI tool_call object into a `ToolCall`.
fn parse_openai_tool_call(tc: &serde_json::Value) -> Option<ToolCall> {
    let id = tc.get("id")?.as_str()?.to_string();
    let function = tc.get("function")?;
    let name = function.get("name")?.as_str()?.to_string();
    let arguments_str = function
        .get("arguments")
        .and_then(|v| v.as_str())
        .unwrap_or("{}");
    let arguments: serde_json::Value =
        serde_json::from_str(arguments_str).unwrap_or(serde_json::Value::Object(Default::default()));
    Some(ToolCall {
        id,
        name,
        arguments,
    })
}

// ---------------------------------------------------------------------------
// Core conversion: OpenAI messages → ShareGPT turns
// ---------------------------------------------------------------------------

/// Convert a slice of OpenAI-format messages to ShareGPT conversation turns.
pub fn openai_to_sharegpt(
    messages: &[serde_json::Value],
    parser: &dyn ToolCallParser,
) -> Vec<ShareGptTurn> {
    let mut turns: Vec<ShareGptTurn> = Vec::new();

    for msg in messages {
        let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");

        match role {
            "system" => {
                let value = extract_text_content(msg.get("content").unwrap_or(&serde_json::Value::Null));
                turns.push(ShareGptTurn {
                    from: "system".to_string(),
                    value,
                });
            }
            "user" => {
                let value = extract_text_content(msg.get("content").unwrap_or(&serde_json::Value::Null));
                turns.push(ShareGptTurn {
                    from: "human".to_string(),
                    value,
                });
            }
            "assistant" => {
                // --- build think block ---
                let reasoning = extract_reasoning(msg);
                let content_text = extract_text_content(
                    msg.get("content").unwrap_or(&serde_json::Value::Null),
                );
                let content_with_replaced = replace_scratchpad_tags(&content_text);

                // If the content itself already contains <think> blocks, use it verbatim.
                // Otherwise, prepend an explicit <think> block (possibly empty).
                let think_block = if content_with_replaced.contains("<think>") {
                    // The <think> block is embedded in the content already; no extra wrapper needed.
                    String::new()
                } else {
                    let inner = reasoning.unwrap_or_default();
                    format!("<think>\n{inner}\n</think>")
                };

                // Remaining content (after stripping any embedded think blocks for assembly).
                let body_text = content_with_replaced;

                // --- tool calls ---
                let tool_calls_raw = msg
                    .get("tool_calls")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();

                let parsed_calls: Vec<ToolCall> = tool_calls_raw
                    .iter()
                    .filter_map(parse_openai_tool_call)
                    .collect();

                let formatted_calls = if parsed_calls.is_empty() {
                    String::new()
                } else {
                    parser.format_tool_calls(&parsed_calls)
                };

                // Assemble the final value.
                let mut parts: Vec<&str> = Vec::new();
                if !think_block.is_empty() {
                    parts.push(think_block.as_str());
                }
                if !body_text.is_empty() {
                    parts.push(body_text.as_str());
                }
                if !formatted_calls.is_empty() {
                    parts.push(formatted_calls.as_str());
                }
                let value = parts.join("\n");

                turns.push(ShareGptTurn {
                    from: "gpt".to_string(),
                    value,
                });
            }
            "tool" => {
                let tool_call_id = msg
                    .get("tool_call_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let name = msg
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let content = msg
                    .get("content")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                let response = ToolResponse {
                    tool_call_id,
                    name,
                    content,
                };
                let value = parser.format_tool_response(&response);
                turns.push(ShareGptTurn {
                    from: "tool".to_string(),
                    value,
                });
            }
            _ => {
                // Unknown roles are skipped.
            }
        }
    }

    turns
}

// ---------------------------------------------------------------------------
// Export function
// ---------------------------------------------------------------------------

/// Derive an output path from the input path and format when none is specified.
pub fn default_output_path(input: &Path, format: &str) -> PathBuf {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("trajectories");
    let parent = input.parent().unwrap_or_else(|| Path::new("."));
    parent.join(format!("{}_{}.jsonl", stem, format))
}

/// Stream-convert a JSONL file of trajectories to the requested format.
///
/// - `format`: `"openai"` (pass-through) or `"sharegpt"` (converted)
/// - `parser_name`: name registered in `ParserRegistry` (used only for `"sharegpt"`)
/// - `filter_completed`: skip entries where `completed == false`
/// - `filter_min_tools`: skip entries with `api_calls < filter_min_tools`
pub fn export_file(
    input: &Path,
    output: &Path,
    format: &str,
    parser_name: &str,
    filter_completed: bool,
    filter_min_tools: Option<u64>,
) -> Result<ExportStats, Box<dyn std::error::Error>> {
    // Resolve parser once (needed for sharegpt).
    let registry = ParserRegistry::new();
    let parser_opt: Option<Box<dyn ToolCallParser>> = if format == "sharegpt" {
        match registry.get(parser_name) {
            Some(p) => Some(p.clone_box()),
            None => {
                return Err(format!(
                    "unknown parser '{}'. Available: {}",
                    parser_name,
                    registry.available_names().join(", ")
                )
                .into())
            }
        }
    } else {
        None
    };

    let in_file = std::fs::File::open(input)
        .map_err(|e| format!("cannot open input '{}': {e}", input.display()))?;
    let reader = std::io::BufReader::new(in_file);

    let out_file = std::fs::File::create(output)
        .map_err(|e| format!("cannot create output '{}': {e}", output.display()))?;
    let mut writer = BufWriter::new(out_file);

    let mut stats = ExportStats::default();

    for line_result in reader.lines() {
        let line = line_result.map_err(|e| format!("read error: {e}"))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        stats.total += 1;

        let entry: TrajectoryEntry = match serde_json::from_str(trimmed) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("Warning: skipping malformed line {}: {e}", stats.total);
                stats.filtered += 1;
                continue;
            }
        };

        // Apply filters.
        if filter_completed && !entry.completed {
            stats.filtered += 1;
            continue;
        }
        if let Some(min) = filter_min_tools {
            if entry.api_calls < min {
                stats.filtered += 1;
                continue;
            }
        }

        // Serialize based on format.
        let serialized = match format {
            "sharegpt" => {
                let parser = parser_opt.as_deref().expect("parser resolved above");
                let conversations = openai_to_sharegpt(&entry.messages, parser);
                let sharegpt_entry = ShareGptEntry {
                    prompt_index: entry.prompt_index,
                    conversations,
                    metadata: entry.metadata,
                    completed: entry.completed,
                    partial: entry.partial,
                    api_calls: entry.api_calls,
                    toolsets_used: entry.toolsets_used,
                    tool_stats: entry.tool_stats,
                    tool_error_counts: entry.tool_error_counts,
                };
                serde_json::to_string(&sharegpt_entry)
                    .map_err(|e| format!("serialize error: {e}"))?
            }
            _ => {
                // "openai" or any other value: pass through as-is (re-serialize from parsed entry).
                serde_json::to_string(&entry).map_err(|e| format!("serialize error: {e}"))?
            }
        };

        writeln!(writer, "{serialized}").map_err(|e| format!("write error: {e}"))?;
        stats.exported += 1;
    }

    writer.flush().map_err(|e| format!("flush error: {e}"))?;
    Ok(stats)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::hermes::HermesParser;
    use serde_json::json;

    fn hermes_parser() -> Box<dyn ToolCallParser> {
        Box::new(HermesParser)
    }

    #[test]
    fn test_openai_to_sharegpt_basic() {
        let messages = vec![
            json!({"role": "system", "content": "You are helpful."}),
            json!({"role": "user", "content": "Hello!"}),
            json!({"role": "assistant", "content": "Hi there!"}),
        ];
        let parser = hermes_parser();
        let turns = openai_to_sharegpt(&messages, parser.as_ref());

        assert_eq!(turns.len(), 3);

        assert_eq!(turns[0].from, "system");
        assert_eq!(turns[0].value, "You are helpful.");

        assert_eq!(turns[1].from, "human");
        assert_eq!(turns[1].value, "Hello!");

        assert_eq!(turns[2].from, "gpt");
        // The gpt turn must contain a <think> block.
        assert!(
            turns[2].value.contains("<think>"),
            "gpt turn must have <think> block; got: {}",
            turns[2].value
        );
        assert!(
            turns[2].value.contains("Hi there!"),
            "gpt turn must contain the assistant content; got: {}",
            turns[2].value
        );
    }

    #[test]
    fn test_openai_to_sharegpt_with_tool_calls() {
        let messages = vec![
            json!({"role": "user", "content": "List files"}),
            json!({
                "role": "assistant",
                "content": "",
                "tool_calls": [
                    {
                        "id": "call_abc",
                        "type": "function",
                        "function": {
                            "name": "bash",
                            "arguments": "{\"command\": \"ls\"}"
                        }
                    }
                ]
            }),
            json!({
                "role": "tool",
                "tool_call_id": "call_abc",
                "name": "bash",
                "content": "file1.txt\nfile2.txt"
            }),
        ];

        let parser = hermes_parser();
        let turns = openai_to_sharegpt(&messages, parser.as_ref());

        assert_eq!(turns.len(), 3);

        assert_eq!(turns[0].from, "human");

        // gpt turn should contain tool call formatting
        assert_eq!(turns[1].from, "gpt");
        assert!(
            !turns[1].value.is_empty(),
            "gpt turn with tool calls must not be empty"
        );

        // tool turn
        assert_eq!(turns[2].from, "tool");
        assert!(
            !turns[2].value.is_empty(),
            "tool response must not be empty"
        );
    }

    #[test]
    fn test_openai_to_sharegpt_reasoning_field() {
        let messages = vec![json!({
            "role": "assistant",
            "content": "The answer is 42.",
            "reasoning": "Let me think step by step..."
        })];

        let parser = hermes_parser();
        let turns = openai_to_sharegpt(&messages, parser.as_ref());

        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].from, "gpt");
        let value = &turns[0].value;
        assert!(value.contains("<think>"), "must have think block");
        assert!(
            value.contains("Let me think step by step..."),
            "must contain reasoning content"
        );
        assert!(value.contains("The answer is 42."), "must contain text");
    }

    #[test]
    fn test_default_output_path() {
        let input = Path::new("/data/trajectories.jsonl");
        let out = default_output_path(input, "sharegpt");
        assert_eq!(out, PathBuf::from("/data/trajectories_sharegpt.jsonl"));
    }
}
