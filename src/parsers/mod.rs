pub mod hermes;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Represents a single tool call extracted from model output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

/// Represents the result returned by a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolResponse {
    pub tool_call_id: String,
    pub name: String,
    pub content: Value,
}

/// Result of parsing a model message: optional text content and optional tool calls.
pub type ParseResult = (Option<String>, Option<Vec<ToolCall>>);

/// Trait for model-specific tool-call parsing and formatting.
pub trait ToolCallParser: Send + Sync {
    /// Human-readable names this parser is registered under (e.g. `["hermes", "qwen"]`).
    fn names(&self) -> Vec<String>;

    /// Serialize a list of tool calls into the model's native text format.
    fn format_tool_calls(&self, calls: &[ToolCall]) -> String;

    /// Serialize a tool response into the model's native text format.
    fn format_tool_response(&self, response: &ToolResponse) -> String;

    /// Parse raw model output into (text, tool_calls).
    fn parse(&self, text: &str) -> ParseResult;

    /// Return a heap-allocated clone. Required for storing parsers in collections.
    fn clone_box(&self) -> Box<dyn ToolCallParser>;
}

/// Blanket helper so `Box<dyn ToolCallParser>` can be used directly as a parser.
impl ToolCallParser for Box<dyn ToolCallParser> {
    fn names(&self) -> Vec<String> {
        (**self).names()
    }

    fn format_tool_calls(&self, calls: &[ToolCall]) -> String {
        (**self).format_tool_calls(calls)
    }

    fn format_tool_response(&self, response: &ToolResponse) -> String {
        (**self).format_tool_response(response)
    }

    fn parse(&self, text: &str) -> ParseResult {
        (**self).parse(text)
    }

    fn clone_box(&self) -> Box<dyn ToolCallParser> {
        (**self).clone_box()
    }
}

/// Registry that maps parser names to their implementations.
pub struct ParserRegistry {
    parsers: HashMap<String, Box<dyn ToolCallParser>>,
}

impl ParserRegistry {
    pub fn new() -> Self {
        Self {
            parsers: HashMap::new(),
        }
    }

    /// Register a parser under all of its declared names.
    pub fn register(&mut self, parser: Box<dyn ToolCallParser>) {
        for name in parser.names() {
            let clone = parser.clone_box();
            self.parsers.insert(name, clone);
        }
    }

    /// Look up a parser by name.
    pub fn get(&self, name: &str) -> Option<&dyn ToolCallParser> {
        self.parsers.get(name).map(|p| p.as_ref())
    }

    /// Return all registered parser names, sorted.
    pub fn available_names(&self) -> Vec<String> {
        let mut names: Vec<_> = self.parsers.keys().cloned().collect();
        names.sort();
        names
    }
}

impl Default for ParserRegistry {
    fn default() -> Self {
        Self::new()
    }
}
