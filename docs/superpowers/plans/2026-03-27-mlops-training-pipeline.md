# MLOps Training Pipeline Implementation Plan (Plan A)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add batch trajectory generation, format export with 11 tool-call parsers, toolset distributions, trajectory compression, pipeline shortcut, and 4 training agent tools to mchact.

**Architecture:** Multi-process batch runner (coordinator spawns `mchact worker` processes), JSONL-based IPC with checkpointing, Rust parsers for 11 model formats, Python subprocess for compression. Three interaction modes: CLI commands, `mchact train` pipeline shortcut, and agent tools.

**Tech Stack:** Rust (clap CLI, serde_json, tokio::process, rand), Python 3.10+ (HuggingFace tokenizers, OpenRouter API for summarization)

**Spec:** `docs/superpowers/specs/2026-03-27-mlops-training-design.md`

**Scope:** This is Plan A of 3. Plan B covers RL training, Plan C covers agent learning (skill auto-creation + browser vision).

---

### Task 1: Toolset Distribution Loading & Sampling

**Files:**
- Create: `src/distributions.rs`
- Create: `training/distributions.yaml`
- Modify: `src/lib.rs`
- Modify: `Cargo.toml`

- [ ] **Step 1: Add `rand` dependency**

In `Cargo.toml`, add to `[dependencies]`:
```toml
rand = "0.8"
```

- [ ] **Step 2: Create `training/distributions.yaml`**

```yaml
default:
  description: "All tools enabled"
  tools:
    web_search: 100
    web_fetch: 100
    browser: 100
    bash: 100
    read_file: 100
    write_file: 100
    edit_file: 100
    glob: 100
    grep: 100
    image_generate: 100
    browser_vision: 100
    mixture_of_agents: 100

research:
  description: "Web-heavy research tasks"
  tools:
    web_search: 90
    browser: 70
    web_fetch: 60
    browser_vision: 50
    mixture_of_agents: 40
    bash: 10

development:
  description: "Code-heavy development tasks"
  tools:
    bash: 95
    read_file: 95
    write_file: 95
    edit_file: 95
    grep: 90
    glob: 90
    mixture_of_agents: 60
    web_search: 30

science:
  description: "Research + code mix"
  tools:
    web_search: 94
    bash: 94
    read_file: 94
    write_file: 94
    browser_vision: 65
    browser: 50
    image_generate: 15
    mixture_of_agents: 10

safe:
  description: "No shell access"
  tools:
    web_search: 80
    browser: 70
    web_fetch: 60
    browser_vision: 60
    image_generate: 60
    mixture_of_agents: 50

minimal:
  description: "Web search only"
  tools:
    web_search: 100

terminal_only:
  description: "Shell and files only"
  tools:
    bash: 100
    read_file: 100
    write_file: 100
    edit_file: 100
    glob: 100
    grep: 100

terminal_web:
  description: "Shell, files, and web"
  tools:
    bash: 100
    read_file: 100
    write_file: 100
    edit_file: 100
    glob: 100
    grep: 100
    web_search: 100
    web_fetch: 100

browser_tasks:
  description: "Browser-heavy automation"
  tools:
    browser: 97
    bash: 15
    browser_vision: 12

terminal_tasks:
  description: "Terminal-heavy with browser support"
  tools:
    bash: 97
    read_file: 97
    write_file: 97
    web_search: 97
    browser: 75
    browser_vision: 50
    image_generate: 10

mixed_tasks:
  description: "Balanced browser and terminal"
  tools:
    browser: 92
    bash: 92
    read_file: 92
    write_file: 92
    web_search: 35
    browser_vision: 15
    image_generate: 15

balanced:
  description: "Equal probability for all tools"
  tools:
    web_search: 50
    web_fetch: 50
    browser: 50
    bash: 50
    read_file: 50
    write_file: 50
    image_generate: 50
    browser_vision: 50
    mixture_of_agents: 50

creative:
  description: "Image and vision focused"
  tools:
    image_generate: 90
    browser_vision: 90
    web_search: 30

reasoning:
  description: "Multi-model reasoning"
  tools:
    mixture_of_agents: 90
    web_search: 30
    bash: 20

multimodal:
  description: "Media-heavy tasks"
  tools:
    image_generate: 90
    text_to_speech: 70
    browser_vision: 60
    web_search: 50
    bash: 30

browser_use:
  description: "Full browser automation"
  tools:
    browser: 100
    web_search: 80
    browser_vision: 70

browser_only:
  description: "Browser only"
  tools:
    browser: 100

image_gen:
  description: "Image generation focused"
  tools:
    image_generate: 90
    browser_vision: 90
    web_search: 55
    bash: 45
    mixture_of_agents: 10
```

- [ ] **Step 3: Write failing test for distribution loading**

Create `src/distributions.rs`:
```rust
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct Distribution {
    pub description: String,
    pub tools: HashMap<String, f64>,
}

pub type DistributionMap = HashMap<String, Distribution>;

pub fn load_distributions(path: &Path) -> Result<DistributionMap, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read distributions file: {e}"))?;
    let map: DistributionMap =
        serde_yaml::from_str(&content).map_err(|e| format!("Invalid distributions YAML: {e}"))?;
    Ok(map)
}

pub fn sample_tools(distribution: &Distribution) -> Vec<String> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut selected: Vec<String> = distribution
        .tools
        .iter()
        .filter(|(_, &prob)| rng.gen_range(0.0..100.0) < prob)
        .map(|(name, _)| name.clone())
        .collect();

    if selected.is_empty() {
        if let Some((best_name, _)) = distribution
            .tools
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        {
            selected.push(best_name.clone());
        }
    }

    selected.sort();
    selected
}

pub fn list_distribution_names(distributions: &DistributionMap) -> Vec<(String, String)> {
    let mut names: Vec<_> = distributions
        .iter()
        .map(|(name, dist)| (name.clone(), dist.description.clone()))
        .collect();
    names.sort_by(|a, b| a.0.cmp(&b.0));
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_distributions() {
        let path = Path::new("training/distributions.yaml");
        let dists = load_distributions(path).expect("Should load distributions");
        assert!(dists.contains_key("default"));
        assert!(dists.contains_key("research"));
        assert!(dists.contains_key("development"));
        let default = &dists["default"];
        assert_eq!(*default.tools.get("bash").unwrap(), 100.0);
    }

    #[test]
    fn test_sample_always_returns_at_least_one() {
        let dist = Distribution {
            description: "test".into(),
            tools: HashMap::from([("bash".into(), 0.001)]), // very low probability
        };
        // Even with near-zero probability, fallback guarantees at least one
        for _ in 0..100 {
            let selected = sample_tools(&dist);
            assert!(!selected.is_empty());
        }
    }

    #[test]
    fn test_sample_default_returns_all() {
        let dist = Distribution {
            description: "test".into(),
            tools: HashMap::from([
                ("bash".into(), 100.0),
                ("web_search".into(), 100.0),
            ]),
        };
        let selected = sample_tools(&dist);
        assert!(selected.contains(&"bash".to_string()));
        assert!(selected.contains(&"web_search".to_string()));
    }

    #[test]
    fn test_list_distribution_names() {
        let mut dists = DistributionMap::new();
        dists.insert("beta".into(), Distribution {
            description: "B desc".into(),
            tools: HashMap::new(),
        });
        dists.insert("alpha".into(), Distribution {
            description: "A desc".into(),
            tools: HashMap::new(),
        });
        let names = list_distribution_names(&dists);
        assert_eq!(names[0].0, "alpha");
        assert_eq!(names[1].0, "beta");
    }
}
```

- [ ] **Step 4: Register module in `src/lib.rs`**

Add after `pub mod compressor;`:
```rust
pub mod distributions;
```

- [ ] **Step 5: Run tests**

Run: `cargo test distributions -- --nocapture`
Expected: All 4 tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/distributions.rs src/lib.rs training/distributions.yaml Cargo.toml
git commit -m "feat: add toolset distribution loading and sampling"
```

---

### Task 2: Tool-Call Parser Trait & Hermes Parser

**Files:**
- Create: `src/parsers/mod.rs`
- Create: `src/parsers/hermes.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Create parser trait and registry**

Create `src/parsers/mod.rs`:
```rust
pub mod hermes;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResponse {
    pub tool_call_id: String,
    pub name: String,
    pub content: serde_json::Value,
}

/// Result of parsing model output: (content_without_tools, parsed_tool_calls)
pub type ParseResult = (Option<String>, Option<Vec<ToolCall>>);

pub trait ToolCallParser: Send + Sync {
    fn names(&self) -> &[&str];
    fn format_tool_calls(&self, content: Option<&str>, tool_calls: &[ToolCall]) -> String;
    fn format_tool_response(&self, response: &ToolResponse) -> String;
    fn parse(&self, text: &str) -> ParseResult;
}

pub struct ParserRegistry {
    parsers: HashMap<String, Box<dyn ToolCallParser>>,
}

impl ParserRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            parsers: HashMap::new(),
        };
        registry.register(Box::new(hermes::HermesParser));
        registry.register(Box::new(hermes::LongcatParser));
        registry.register(Box::new(hermes::QwenParser));
        registry
    }

    fn register(&mut self, parser: Box<dyn ToolCallParser>) {
        for name in parser.names() {
            self.parsers.insert(name.to_string(), parser.clone_box());
        }
    }

    pub fn get(&self, name: &str) -> Option<&dyn ToolCallParser> {
        self.parsers.get(name).map(|p| p.as_ref())
    }

    pub fn available_names(&self) -> Vec<String> {
        let mut names: Vec<_> = self.parsers.keys().cloned().collect();
        names.sort();
        names.dedup();
        names
    }
}

/// Blanket clone support for parsers
pub trait CloneParser: ToolCallParser {
    fn clone_box(&self) -> Box<dyn ToolCallParser>;
}

impl<T: ToolCallParser + Clone + 'static> CloneParser for T {
    fn clone_box(&self) -> Box<dyn ToolCallParser> {
        Box::new(self.clone())
    }
}

// Redirect ToolCallParser to require CloneParser
impl ToolCallParser for Box<dyn ToolCallParser> {
    fn names(&self) -> &[&str] { self.as_ref().names() }
    fn format_tool_calls(&self, content: Option<&str>, tool_calls: &[ToolCall]) -> String {
        self.as_ref().format_tool_calls(content, tool_calls)
    }
    fn format_tool_response(&self, response: &ToolResponse) -> String {
        self.as_ref().format_tool_response(response)
    }
    fn parse(&self, text: &str) -> ParseResult {
        self.as_ref().parse(text)
    }
}
```

- [ ] **Step 2: Write Hermes parser with tests**

Create `src/parsers/hermes.rs`:
```rust
use super::{CloneParser, ParseResult, ToolCall, ToolCallParser, ToolResponse};
use regex::Regex;
use std::sync::LazyLock;

static HERMES_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)<tool_call>\s*(.*?)\s*</tool_call>|<tool_call>\s*(.*)").unwrap()
});

static LONGCAT_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)<longcat_tool_call>\s*(.*?)\s*</longcat_tool_call>|<longcat_tool_call>\s*(.*)")
        .unwrap()
});

fn gen_call_id() -> String {
    format!("call_{}", &uuid::Uuid::new_v4().to_string()[..8])
}

fn parse_with_pattern(text: &str, pattern: &Regex, open_tag: &str) -> ParseResult {
    let mut tool_calls = Vec::new();
    let content_before = text.split(open_tag).next().map(|s| s.trim().to_string());
    let content = if content_before.as_deref() == Some("") {
        None
    } else {
        content_before
    };

    for cap in pattern.captures_iter(text) {
        let json_str = cap.get(1).or_else(|| cap.get(2)).map(|m| m.as_str().trim());
        if let Some(json_str) = json_str {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
                if let (Some(name), Some(args)) = (
                    parsed.get("name").and_then(|v| v.as_str()),
                    parsed.get("arguments"),
                ) {
                    tool_calls.push(ToolCall {
                        id: gen_call_id(),
                        name: name.to_string(),
                        arguments: args.clone(),
                    });
                }
            }
        }
    }

    if tool_calls.is_empty() {
        (Some(text.to_string()), None)
    } else {
        (content, Some(tool_calls))
    }
}

fn format_calls_with_tag(
    content: Option<&str>,
    tool_calls: &[ToolCall],
    open_tag: &str,
    close_tag: &str,
) -> String {
    let mut output = String::new();
    if let Some(c) = content {
        if !c.is_empty() {
            output.push_str(c);
            output.push('\n');
        }
    }
    for call in tool_calls {
        let json = serde_json::json!({
            "name": call.name,
            "arguments": call.arguments,
        });
        output.push_str(&format!(
            "{}\n{}\n{}\n",
            open_tag,
            serde_json::to_string(&json).unwrap_or_default(),
            close_tag
        ));
    }
    output.trim_end().to_string()
}

fn format_response_with_tag(response: &ToolResponse, open_tag: &str, close_tag: &str) -> String {
    let json = serde_json::json!({
        "tool_call_id": response.tool_call_id,
        "name": response.name,
        "content": response.content,
    });
    format!(
        "{}\n{}\n{}",
        open_tag,
        serde_json::to_string(&json).unwrap_or_default(),
        close_tag
    )
}

#[derive(Clone)]
pub struct HermesParser;

impl ToolCallParser for HermesParser {
    fn names(&self) -> &[&str] {
        &["hermes"]
    }

    fn format_tool_calls(&self, content: Option<&str>, tool_calls: &[ToolCall]) -> String {
        format_calls_with_tag(content, tool_calls, "<tool_call>", "</tool_call>")
    }

    fn format_tool_response(&self, response: &ToolResponse) -> String {
        format_response_with_tag(response, "<tool_response>", "</tool_response>")
    }

    fn parse(&self, text: &str) -> ParseResult {
        parse_with_pattern(text, &HERMES_PATTERN, "<tool_call>")
    }
}

#[derive(Clone)]
pub struct LongcatParser;

impl ToolCallParser for LongcatParser {
    fn names(&self) -> &[&str] {
        &["longcat"]
    }

    fn format_tool_calls(&self, content: Option<&str>, tool_calls: &[ToolCall]) -> String {
        format_calls_with_tag(content, tool_calls, "<longcat_tool_call>", "</longcat_tool_call>")
    }

    fn format_tool_response(&self, response: &ToolResponse) -> String {
        format_response_with_tag(response, "<tool_response>", "</tool_response>")
    }

    fn parse(&self, text: &str) -> ParseResult {
        parse_with_pattern(text, &LONGCAT_PATTERN, "<longcat_tool_call>")
    }
}

#[derive(Clone)]
pub struct QwenParser;

impl ToolCallParser for QwenParser {
    fn names(&self) -> &[&str] {
        &["qwen"]
    }

    fn format_tool_calls(&self, content: Option<&str>, tool_calls: &[ToolCall]) -> String {
        HermesParser.format_tool_calls(content, tool_calls)
    }

    fn format_tool_response(&self, response: &ToolResponse) -> String {
        HermesParser.format_tool_response(response)
    }

    fn parse(&self, text: &str) -> ParseResult {
        HermesParser.parse(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hermes_parse_single_tool_call() {
        let text = r#"Let me search for that.
<tool_call>
{"name": "web_search", "arguments": {"query": "rust async"}}
</tool_call>"#;

        let (content, calls) = HermesParser.parse(text);
        assert_eq!(content.as_deref(), Some("Let me search for that."));
        let calls = calls.unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "web_search");
        assert_eq!(calls[0].arguments["query"], "rust async");
    }

    #[test]
    fn test_hermes_parse_multiple_tool_calls() {
        let text = r#"<tool_call>
{"name": "bash", "arguments": {"command": "ls"}}
</tool_call>
<tool_call>
{"name": "read_file", "arguments": {"path": "foo.txt"}}
</tool_call>"#;

        let (content, calls) = HermesParser.parse(text);
        assert!(content.is_none());
        let calls = calls.unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "bash");
        assert_eq!(calls[1].name, "read_file");
    }

    #[test]
    fn test_hermes_parse_unclosed_tag() {
        let text = r#"<tool_call>
{"name": "bash", "arguments": {"command": "ls"}}"#;

        let (_, calls) = HermesParser.parse(text);
        let calls = calls.unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "bash");
    }

    #[test]
    fn test_hermes_parse_no_tool_calls() {
        let text = "Just a regular response with no tools.";
        let (content, calls) = HermesParser.parse(text);
        assert_eq!(content.as_deref(), Some(text));
        assert!(calls.is_none());
    }

    #[test]
    fn test_hermes_format_tool_calls() {
        let calls = vec![ToolCall {
            id: "call_abc".into(),
            name: "bash".into(),
            arguments: serde_json::json!({"command": "ls"}),
        }];
        let formatted = HermesParser.format_tool_calls(Some("Hello"), &calls);
        assert!(formatted.contains("<tool_call>"));
        assert!(formatted.contains("</tool_call>"));
        assert!(formatted.contains("Hello"));
        assert!(formatted.contains("\"name\":\"bash\""));
    }

    #[test]
    fn test_hermes_format_tool_response() {
        let resp = ToolResponse {
            tool_call_id: "call_abc".into(),
            name: "bash".into(),
            content: serde_json::json!("file1.txt\nfile2.txt"),
        };
        let formatted = HermesParser.format_tool_response(&resp);
        assert!(formatted.contains("<tool_response>"));
        assert!(formatted.contains("</tool_response>"));
        assert!(formatted.contains("call_abc"));
    }

    #[test]
    fn test_longcat_parse() {
        let text = r#"<longcat_tool_call>
{"name": "bash", "arguments": {"command": "ls"}}
</longcat_tool_call>"#;
        let (_, calls) = LongcatParser.parse(text);
        assert_eq!(calls.unwrap().len(), 1);
    }

    #[test]
    fn test_qwen_delegates_to_hermes() {
        let text = r#"<tool_call>
{"name": "bash", "arguments": {"command": "ls"}}
</tool_call>"#;
        let (_, calls) = QwenParser.parse(text);
        assert_eq!(calls.unwrap().len(), 1);
    }
}
```

- [ ] **Step 3: Register parsers module in `src/lib.rs`**

Add after `pub mod distributions;`:
```rust
pub mod parsers;
```

- [ ] **Step 4: Run tests**

Run: `cargo test parsers::hermes -- --nocapture`
Expected: All 7 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/parsers/ src/lib.rs
git commit -m "feat: add tool-call parser trait with Hermes/Longcat/Qwen parsers"
```

---

### Task 3: Remaining Tool-Call Parsers (Llama, Mistral, DeepSeek, GLM, Kimi, Qwen3-Coder)

**Files:**
- Create: `src/parsers/llama.rs`
- Create: `src/parsers/mistral.rs`
- Create: `src/parsers/deepseek.rs`
- Create: `src/parsers/glm.rs`
- Create: `src/parsers/kimi.rs`
- Create: `src/parsers/qwen_coder.rs`
- Modify: `src/parsers/mod.rs`

- [ ] **Step 1: Write Llama parser**

Create `src/parsers/llama.rs`:
```rust
use super::{CloneParser, ParseResult, ToolCall, ToolCallParser, ToolResponse};

fn gen_call_id() -> String {
    format!("call_{}", &uuid::Uuid::new_v4().to_string()[..8])
}

/// Llama 3/4 parser. Tool calls are raw JSON objects with "name" + "arguments"|"parameters".
/// Optional <|python_tag|> prefix. Uses incremental JSON scanning.
#[derive(Clone)]
pub struct LlamaParser;

impl LlamaParser {
    fn try_parse_json_at(text: &str, start: usize) -> Option<(serde_json::Value, usize)> {
        let slice = &text[start..];
        let mut depth = 0i32;
        let mut in_string = false;
        let mut escape = false;
        for (i, ch) in slice.char_indices() {
            if escape {
                escape = false;
                continue;
            }
            match ch {
                '\\' if in_string => escape = true,
                '"' => in_string = !in_string,
                '{' if !in_string => depth += 1,
                '}' if !in_string => {
                    depth -= 1;
                    if depth == 0 {
                        let json_str = &slice[..=i];
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
                            return Some((val, start + i + 1));
                        }
                        return None;
                    }
                }
                _ => {}
            }
        }
        None
    }
}

impl ToolCallParser for LlamaParser {
    fn names(&self) -> &[&str] {
        &["llama3_json", "llama4_json"]
    }

    fn format_tool_calls(&self, content: Option<&str>, tool_calls: &[ToolCall]) -> String {
        let mut output = String::new();
        if let Some(c) = content {
            if !c.is_empty() {
                output.push_str(c);
                output.push('\n');
            }
        }
        output.push_str("<|python_tag|>");
        for call in tool_calls {
            let json = serde_json::json!({
                "name": call.name,
                "arguments": call.arguments,
            });
            output.push_str(&serde_json::to_string(&json).unwrap_or_default());
        }
        output
    }

    fn format_tool_response(&self, response: &ToolResponse) -> String {
        serde_json::json!({"output": response.content}).to_string()
    }

    fn parse(&self, text: &str) -> ParseResult {
        let content_text = text.split("<|python_tag|>").next().unwrap_or(text);
        let content = if content_text.trim().is_empty() {
            None
        } else {
            Some(content_text.trim().to_string())
        };

        let mut tool_calls = Vec::new();
        let mut pos = 0;
        while pos < text.len() {
            if let Some(brace_pos) = text[pos..].find('{') {
                let abs_pos = pos + brace_pos;
                if let Some((val, end_pos)) = Self::try_parse_json_at(text, abs_pos) {
                    if val.get("name").and_then(|v| v.as_str()).is_some()
                        && (val.get("arguments").is_some() || val.get("parameters").is_some())
                    {
                        let name = val["name"].as_str().unwrap().to_string();
                        let args = val
                            .get("arguments")
                            .or_else(|| val.get("parameters"))
                            .cloned()
                            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
                        tool_calls.push(ToolCall {
                            id: gen_call_id(),
                            name,
                            arguments: args,
                        });
                    }
                    pos = end_pos;
                } else {
                    pos = abs_pos + 1;
                }
            } else {
                break;
            }
        }

        if tool_calls.is_empty() {
            (Some(text.to_string()), None)
        } else {
            (content, Some(tool_calls))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llama_parse_basic() {
        let text = r#"{"name": "bash", "arguments": {"command": "ls"}}"#;
        let (_, calls) = LlamaParser.parse(text);
        let calls = calls.unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "bash");
    }

    #[test]
    fn test_llama_parse_with_python_tag() {
        let text = r#"Some text<|python_tag|>{"name": "bash", "arguments": {"command": "ls"}}"#;
        let (content, calls) = LlamaParser.parse(text);
        assert_eq!(content.as_deref(), Some("Some text"));
        assert_eq!(calls.unwrap().len(), 1);
    }

    #[test]
    fn test_llama_parse_parameters_alias() {
        let text = r#"{"name": "bash", "parameters": {"command": "ls"}}"#;
        let (_, calls) = LlamaParser.parse(text);
        let calls = calls.unwrap();
        assert_eq!(calls[0].arguments["command"], "ls");
    }

    #[test]
    fn test_llama_no_tool_calls() {
        let text = "Just regular text with some {braces}";
        let (content, calls) = LlamaParser.parse(text);
        assert!(content.is_some());
        assert!(calls.is_none());
    }
}
```

- [ ] **Step 2: Write Mistral parser**

Create `src/parsers/mistral.rs`:
```rust
use super::{CloneParser, ParseResult, ToolCall, ToolCallParser, ToolResponse};
use rand::Rng;

const TOOL_CALLS_TOKEN: &str = "[TOOL_CALLS]";

fn gen_mistral_id() -> String {
    let mut rng = rand::thread_rng();
    (0..9)
        .map(|_| {
            let idx = rng.gen_range(0..62);
            if idx < 10 {
                (b'0' + idx) as char
            } else if idx < 36 {
                (b'a' + idx - 10) as char
            } else {
                (b'A' + idx - 36) as char
            }
        })
        .collect()
}

#[derive(Clone)]
pub struct MistralParser;

impl ToolCallParser for MistralParser {
    fn names(&self) -> &[&str] {
        &["mistral"]
    }

    fn format_tool_calls(&self, content: Option<&str>, tool_calls: &[ToolCall]) -> String {
        let mut output = String::new();
        if let Some(c) = content {
            if !c.is_empty() {
                output.push_str(c);
                output.push('\n');
            }
        }
        // Use v11+ format: [TOOL_CALLS]func_name{args}
        for call in tool_calls {
            output.push_str(TOOL_CALLS_TOKEN);
            output.push_str(&call.name);
            output.push_str(&serde_json::to_string(&call.arguments).unwrap_or_default());
        }
        output
    }

    fn format_tool_response(&self, response: &ToolResponse) -> String {
        format!(
            "[TOOL_RESULTS]{}[/TOOL_RESULTS]",
            serde_json::json!({"content": response.content})
        )
    }

    fn parse(&self, text: &str) -> ParseResult {
        let content_before = text.split(TOOL_CALLS_TOKEN).next().map(|s| s.trim().to_string());
        let content = match &content_before {
            Some(s) if s.is_empty() => None,
            other => other.clone(),
        };

        let parts: Vec<&str> = text.split(TOOL_CALLS_TOKEN).collect();
        if parts.len() < 2 {
            return (Some(text.to_string()), None);
        }

        let mut tool_calls = Vec::new();
        let after_first = parts[1..].join(TOOL_CALLS_TOKEN);
        let trimmed = after_first.trim();

        // Detect format: if starts with [ or { → pre-v11 (JSON)
        if trimmed.starts_with('[') || trimmed.starts_with('{') {
            // Pre-v11: JSON array or object
            if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(trimmed) {
                for val in arr {
                    if let Some(name) = val.get("name").and_then(|v| v.as_str()) {
                        let args = val.get("arguments").cloned().unwrap_or_default();
                        tool_calls.push(ToolCall {
                            id: gen_mistral_id(),
                            name: name.to_string(),
                            arguments: args,
                        });
                    }
                }
            } else if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                if let Some(name) = val.get("name").and_then(|v| v.as_str()) {
                    let args = val.get("arguments").cloned().unwrap_or_default();
                    tool_calls.push(ToolCall {
                        id: gen_mistral_id(),
                        name: name.to_string(),
                        arguments: args,
                    });
                }
            }
        } else {
            // v11+: func_name{args} per [TOOL_CALLS] section
            for part in &parts[1..] {
                let part = part.trim();
                if part.is_empty() {
                    continue;
                }
                if let Some(brace_pos) = part.find('{') {
                    let func_name = part[..brace_pos].trim();
                    let json_str = &part[brace_pos..];
                    if let Ok(args) = serde_json::from_str::<serde_json::Value>(json_str) {
                        tool_calls.push(ToolCall {
                            id: gen_mistral_id(),
                            name: func_name.to_string(),
                            arguments: args,
                        });
                    }
                }
            }
        }

        if tool_calls.is_empty() {
            (Some(text.to_string()), None)
        } else {
            (content, Some(tool_calls))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mistral_v11_format() {
        let text = r#"[TOOL_CALLS]bash{"command": "ls"}"#;
        let (content, calls) = MistralParser.parse(text);
        assert!(content.is_none());
        let calls = calls.unwrap();
        assert_eq!(calls[0].name, "bash");
    }

    #[test]
    fn test_mistral_pre_v11_array() {
        let text = r#"[TOOL_CALLS][{"name": "bash", "arguments": {"command": "ls"}}]"#;
        let (_, calls) = MistralParser.parse(text);
        assert_eq!(calls.unwrap().len(), 1);
    }

    #[test]
    fn test_mistral_pre_v11_object() {
        let text = r#"[TOOL_CALLS]{"name": "bash", "arguments": {"command": "ls"}}"#;
        let (_, calls) = MistralParser.parse(text);
        assert_eq!(calls.unwrap().len(), 1);
    }

    #[test]
    fn test_mistral_id_format() {
        let id = gen_mistral_id();
        assert_eq!(id.len(), 9);
        assert!(id.chars().all(|c| c.is_ascii_alphanumeric()));
    }
}
```

- [ ] **Step 3: Write DeepSeek V3 + V3.1 parsers**

Create `src/parsers/deepseek.rs`:
```rust
use super::{CloneParser, ParseResult, ToolCall, ToolCallParser, ToolResponse};
use regex::Regex;
use std::sync::LazyLock;

fn gen_call_id() -> String {
    format!("call_{}", &uuid::Uuid::new_v4().to_string()[..8])
}

// DeepSeek V3: <｜tool▁call▁begin｜>type<｜tool▁sep｜>name ```json\n{...}\n``` <｜tool▁call▁end｜>
static V3_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)<｜tool▁call▁begin｜>(?P<type>.*?)<｜tool▁sep｜>(?P<name>.*?)\s*```json\s*(?P<args>.*?)\s*```\s*<｜tool▁call▁end｜>").unwrap()
});

// DeepSeek V3.1: <｜tool▁call▁begin｜>name<｜tool▁sep｜>{args}<｜tool▁call▁end｜>
static V31_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)<｜tool▁call▁begin｜>(?P<name>.*?)<｜tool▁sep｜>(?P<args>.*?)<｜tool▁call▁end｜>").unwrap()
});

const CALLS_BEGIN: &str = "<｜tool▁calls▁begin｜>";
const CALL_BEGIN: &str = "<｜tool▁call▁begin｜>";
const TOOL_SEP: &str = "<｜tool▁sep｜>";
const CALL_END: &str = "<｜tool▁call▁end｜>";
const CALLS_END: &str = "<｜tool▁calls▁end｜>";

#[derive(Clone)]
pub struct DeepSeekV3Parser;

impl ToolCallParser for DeepSeekV3Parser {
    fn names(&self) -> &[&str] {
        &["deepseek_v3"]
    }

    fn format_tool_calls(&self, content: Option<&str>, tool_calls: &[ToolCall]) -> String {
        let mut output = String::new();
        if let Some(c) = content {
            if !c.is_empty() {
                output.push_str(c);
                output.push('\n');
            }
        }
        output.push_str(CALLS_BEGIN);
        output.push('\n');
        for call in tool_calls {
            output.push_str(&format!(
                "{}function{}{}\n```json\n{}\n```\n{}\n",
                CALL_BEGIN,
                TOOL_SEP,
                call.name,
                serde_json::to_string_pretty(&call.arguments).unwrap_or_default(),
                CALL_END
            ));
        }
        output.push_str(CALLS_END);
        output
    }

    fn format_tool_response(&self, response: &ToolResponse) -> String {
        format!(
            "<｜tool▁output▁begin｜>{}<｜tool▁output▁end｜>",
            serde_json::to_string(&response.content).unwrap_or_default()
        )
    }

    fn parse(&self, text: &str) -> ParseResult {
        let content = text.split(CALLS_BEGIN).next().map(|s| s.trim().to_string());
        let content = match &content {
            Some(s) if s.is_empty() => None,
            other => other.clone(),
        };

        let mut tool_calls = Vec::new();
        for cap in V3_PATTERN.captures_iter(text) {
            let name = cap.name("name").map(|m| m.as_str().trim()).unwrap_or("");
            let args_str = cap.name("args").map(|m| m.as_str().trim()).unwrap_or("{}");
            if let Ok(args) = serde_json::from_str::<serde_json::Value>(args_str) {
                tool_calls.push(ToolCall {
                    id: gen_call_id(),
                    name: name.to_string(),
                    arguments: args,
                });
            }
        }

        if tool_calls.is_empty() {
            (Some(text.to_string()), None)
        } else {
            (content, Some(tool_calls))
        }
    }
}

#[derive(Clone)]
pub struct DeepSeekV31Parser;

impl ToolCallParser for DeepSeekV31Parser {
    fn names(&self) -> &[&str] {
        &["deepseek_v3_1", "deepseek_v31"]
    }

    fn format_tool_calls(&self, content: Option<&str>, tool_calls: &[ToolCall]) -> String {
        let mut output = String::new();
        if let Some(c) = content {
            if !c.is_empty() {
                output.push_str(c);
                output.push('\n');
            }
        }
        output.push_str(CALLS_BEGIN);
        output.push('\n');
        for call in tool_calls {
            output.push_str(&format!(
                "{}{}{}{}{}\n",
                CALL_BEGIN,
                call.name,
                TOOL_SEP,
                serde_json::to_string(&call.arguments).unwrap_or_default(),
                CALL_END
            ));
        }
        output.push_str(CALLS_END);
        output
    }

    fn format_tool_response(&self, response: &ToolResponse) -> String {
        DeepSeekV3Parser.format_tool_response(response)
    }

    fn parse(&self, text: &str) -> ParseResult {
        let content = text.split(CALLS_BEGIN).next().map(|s| s.trim().to_string());
        let content = match &content {
            Some(s) if s.is_empty() => None,
            other => other.clone(),
        };

        let mut tool_calls = Vec::new();
        for cap in V31_PATTERN.captures_iter(text) {
            let name = cap.name("name").map(|m| m.as_str().trim()).unwrap_or("");
            let args_str = cap.name("args").map(|m| m.as_str().trim()).unwrap_or("{}");
            if let Ok(args) = serde_json::from_str::<serde_json::Value>(args_str) {
                tool_calls.push(ToolCall {
                    id: gen_call_id(),
                    name: name.to_string(),
                    arguments: args,
                });
            }
        }

        if tool_calls.is_empty() {
            (Some(text.to_string()), None)
        } else {
            (content, Some(tool_calls))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deepseek_v3_parse() {
        let text = format!(
            "Hello\n{}\n{}function{}bash\n```json\n{{\"command\": \"ls\"}}\n```\n{}\n{}",
            CALLS_BEGIN, CALL_BEGIN, TOOL_SEP, CALL_END, CALLS_END
        );
        let (content, calls) = DeepSeekV3Parser.parse(&text);
        assert_eq!(content.as_deref(), Some("Hello"));
        let calls = calls.unwrap();
        assert_eq!(calls[0].name, "bash");
    }

    #[test]
    fn test_deepseek_v31_parse() {
        let text = format!(
            "{}{}bash{}{{\"command\": \"ls\"}}{}",
            CALLS_BEGIN, CALL_BEGIN, TOOL_SEP, CALL_END
        );
        let (_, calls) = DeepSeekV31Parser.parse(&text);
        let calls = calls.unwrap();
        assert_eq!(calls[0].name, "bash");
    }
}
```

- [ ] **Step 4: Write GLM 4.5 + 4.7 parsers**

Create `src/parsers/glm.rs`:
```rust
use super::{CloneParser, ParseResult, ToolCall, ToolCallParser, ToolResponse};
use regex::Regex;
use std::sync::LazyLock;

fn gen_call_id() -> String {
    format!("call_{}", &uuid::Uuid::new_v4().to_string()[..8])
}

static FUNC_CALL_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<tool_call>.*?</tool_call>").unwrap());

static FUNC_DETAIL_45: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<tool_call>([^\n]*)\n(.*)</tool_call>").unwrap());

static FUNC_DETAIL_47: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<tool_call>(.*?)(<arg_key>.*?)?</tool_call>").unwrap());

static ARG_REGEX_45: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<arg_key>(.*?)</arg_key>\s*<arg_value>(.*?)</arg_value>").unwrap());

static ARG_REGEX_47: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)<arg_key>(.*?)</arg_key>(?:\\n|\s)*<arg_value>(.*?)</arg_value>").unwrap()
});

fn deserialize_value(value: &str) -> serde_json::Value {
    // Try JSON first
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(value) {
        return v;
    }
    // Fallback: raw string
    serde_json::Value::String(value.to_string())
}

fn parse_glm(
    text: &str,
    detail_regex: &Regex,
    arg_regex: &Regex,
) -> ParseResult {
    let content = {
        let first_tag = text.find("<tool_call>");
        match first_tag {
            Some(pos) if pos > 0 => Some(text[..pos].trim().to_string()),
            Some(_) => None,
            None => Some(text.to_string()),
        }
    };

    let mut tool_calls = Vec::new();

    for func_match in FUNC_CALL_REGEX.find_iter(text) {
        let block = func_match.as_str();
        if let Some(cap) = detail_regex.captures(block) {
            let func_name = cap.get(1).map(|m| m.as_str().trim()).unwrap_or("");
            // Strip any <arg_key> content from func_name for GLM 4.7
            let func_name = func_name.split('<').next().unwrap_or(func_name).trim();

            let args_section = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            let mut args = serde_json::Map::new();

            for arg_cap in arg_regex.captures_iter(args_section) {
                let key = arg_cap.get(1).map(|m| m.as_str().trim()).unwrap_or("");
                let val = arg_cap.get(2).map(|m| m.as_str().trim()).unwrap_or("");
                if !key.is_empty() {
                    args.insert(key.to_string(), deserialize_value(val));
                }
            }

            if !func_name.is_empty() {
                tool_calls.push(ToolCall {
                    id: gen_call_id(),
                    name: func_name.to_string(),
                    arguments: serde_json::Value::Object(args),
                });
            }
        }
    }

    if tool_calls.is_empty() {
        (Some(text.to_string()), None)
    } else {
        (content, Some(tool_calls))
    }
}

#[derive(Clone)]
pub struct Glm45Parser;

impl ToolCallParser for Glm45Parser {
    fn names(&self) -> &[&str] {
        &["glm45"]
    }

    fn format_tool_calls(&self, content: Option<&str>, tool_calls: &[ToolCall]) -> String {
        let mut output = String::new();
        if let Some(c) = content {
            if !c.is_empty() {
                output.push_str(c);
                output.push('\n');
            }
        }
        for call in tool_calls {
            output.push_str(&format!("<tool_call>{}\n", call.name));
            if let Some(obj) = call.arguments.as_object() {
                for (k, v) in obj {
                    output.push_str(&format!(
                        "<arg_key>{}</arg_key><arg_value>{}</arg_value>\n",
                        k,
                        serde_json::to_string(v).unwrap_or_default()
                    ));
                }
            }
            output.push_str("</tool_call>\n");
        }
        output.trim_end().to_string()
    }

    fn format_tool_response(&self, response: &ToolResponse) -> String {
        format!(
            "<|observation|>\n{}",
            serde_json::json!({"output": response.content})
        )
    }

    fn parse(&self, text: &str) -> ParseResult {
        parse_glm(text, &FUNC_DETAIL_45, &ARG_REGEX_45)
    }
}

#[derive(Clone)]
pub struct Glm47Parser;

impl ToolCallParser for Glm47Parser {
    fn names(&self) -> &[&str] {
        &["glm47"]
    }

    fn format_tool_calls(&self, content: Option<&str>, tool_calls: &[ToolCall]) -> String {
        Glm45Parser.format_tool_calls(content, tool_calls)
    }

    fn format_tool_response(&self, response: &ToolResponse) -> String {
        Glm45Parser.format_tool_response(response)
    }

    fn parse(&self, text: &str) -> ParseResult {
        parse_glm(text, &FUNC_DETAIL_47, &ARG_REGEX_47)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glm45_parse() {
        let text = "<tool_call>bash\n<arg_key>command</arg_key><arg_value>ls</arg_value>\n</tool_call>";
        let (_, calls) = Glm45Parser.parse(text);
        let calls = calls.unwrap();
        assert_eq!(calls[0].name, "bash");
        assert_eq!(calls[0].arguments["command"], "ls");
    }

    #[test]
    fn test_glm45_multiple_args() {
        let text = "<tool_call>web_search\n<arg_key>query</arg_key><arg_value>rust</arg_value>\n<arg_key>limit</arg_key><arg_value>10</arg_value>\n</tool_call>";
        let (_, calls) = Glm45Parser.parse(text);
        let calls = calls.unwrap();
        assert_eq!(calls[0].arguments["query"], "rust");
        assert_eq!(calls[0].arguments["limit"], 10);
    }

    #[test]
    fn test_glm47_flexible_newlines() {
        let text = "<tool_call>bash\n<arg_key>command</arg_key>\n<arg_value>ls</arg_value>\n</tool_call>";
        let (_, calls) = Glm47Parser.parse(text);
        let calls = calls.unwrap();
        assert_eq!(calls[0].name, "bash");
    }
}
```

- [ ] **Step 5: Write Kimi K2 parser**

Create `src/parsers/kimi.rs`:
```rust
use super::{CloneParser, ParseResult, ToolCall, ToolCallParser, ToolResponse};
use regex::Regex;
use std::sync::LazyLock;

static KIMI_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)<\|tool_call_begin\|>\s*(?P<id>[^<]+:\d+)\s*<\|tool_call_argument_begin\|>\s*(?P<args>(?:(?!<\|tool_call_begin\|>).)*?)\s*<\|tool_call_end\|>").unwrap()
});

const START_TOKENS: &[&str] = &[
    "<|tool_calls_section_begin|>",
    "<|tool_call_section_begin|>",
];

fn extract_func_name(id: &str) -> String {
    // "functions.get_weather:0" → "get_weather"
    let without_index = id.split(':').next().unwrap_or(id);
    without_index.split('.').last().unwrap_or(without_index).trim().to_string()
}

#[derive(Clone)]
pub struct KimiK2Parser;

impl ToolCallParser for KimiK2Parser {
    fn names(&self) -> &[&str] {
        &["kimi_k2"]
    }

    fn format_tool_calls(&self, content: Option<&str>, tool_calls: &[ToolCall]) -> String {
        let mut output = String::new();
        if let Some(c) = content {
            if !c.is_empty() {
                output.push_str(c);
                output.push('\n');
            }
        }
        output.push_str("<|tool_calls_section_begin|>\n");
        for (i, call) in tool_calls.iter().enumerate() {
            output.push_str(&format!(
                "<|tool_call_begin|>functions.{}:{}<|tool_call_argument_begin|>{}<|tool_call_end|>\n",
                call.name,
                i,
                serde_json::to_string(&call.arguments).unwrap_or_default()
            ));
        }
        output.push_str("<|tool_calls_section_end|>");
        output
    }

    fn format_tool_response(&self, response: &ToolResponse) -> String {
        format!(
            "<tool_response>{}</tool_response>",
            serde_json::json!({"result": response.content})
        )
    }

    fn parse(&self, text: &str) -> ParseResult {
        let has_start = START_TOKENS.iter().any(|t| text.contains(t));
        if !has_start {
            return (Some(text.to_string()), None);
        }

        let content = {
            let mut earliest = text.len();
            for token in START_TOKENS {
                if let Some(pos) = text.find(token) {
                    earliest = earliest.min(pos);
                }
            }
            let before = text[..earliest].trim();
            if before.is_empty() { None } else { Some(before.to_string()) }
        };

        let mut tool_calls = Vec::new();
        for cap in KIMI_PATTERN.captures_iter(text) {
            let id = cap.name("id").map(|m| m.as_str().trim()).unwrap_or("");
            let args_str = cap.name("args").map(|m| m.as_str().trim()).unwrap_or("{}");
            let func_name = extract_func_name(id);
            if let Ok(args) = serde_json::from_str::<serde_json::Value>(args_str) {
                tool_calls.push(ToolCall {
                    id: id.to_string(),
                    name: func_name,
                    arguments: args,
                });
            }
        }

        if tool_calls.is_empty() {
            (Some(text.to_string()), None)
        } else {
            (content, Some(tool_calls))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kimi_parse() {
        let text = r#"<|tool_calls_section_begin|>
<|tool_call_begin|>functions.bash:0<|tool_call_argument_begin|>{"command": "ls"}<|tool_call_end|>
<|tool_calls_section_end|>"#;
        let (_, calls) = KimiK2Parser.parse(text);
        let calls = calls.unwrap();
        assert_eq!(calls[0].name, "bash");
        assert_eq!(calls[0].id, "functions.bash:0");
    }

    #[test]
    fn test_kimi_extract_func_name() {
        assert_eq!(extract_func_name("functions.get_weather:0"), "get_weather");
        assert_eq!(extract_func_name("bash:1"), "bash");
    }

    #[test]
    fn test_kimi_singular_start_token() {
        let text = r#"<|tool_call_section_begin|>
<|tool_call_begin|>functions.bash:0<|tool_call_argument_begin|>{"command": "ls"}<|tool_call_end|>"#;
        let (_, calls) = KimiK2Parser.parse(text);
        assert_eq!(calls.unwrap().len(), 1);
    }
}
```

- [ ] **Step 6: Write Qwen3 Coder parser**

Create `src/parsers/qwen_coder.rs`:
```rust
use super::{CloneParser, ParseResult, ToolCall, ToolCallParser, ToolResponse};
use regex::Regex;
use std::sync::LazyLock;

fn gen_call_id() -> String {
    format!("call_{}", &uuid::Uuid::new_v4().to_string().replace('-', "")[..24])
}

static TOOL_CALL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)<tool_call>(.*?)</tool_call>|<tool_call>(.*?)$").unwrap()
});

static FUNCTION_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)<function=(.*?)>(.*?)</function>|<function=(.*?)>(.*?)$").unwrap()
});

static PARAMETER_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)<parameter=(.*?)>(.*?)(?:</parameter>|(?=<parameter=)|(?=</function>)|$)")
        .unwrap()
});

fn try_convert_value(value: &str) -> serde_json::Value {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("null") {
        return serde_json::Value::Null;
    }
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return v;
    }
    serde_json::Value::String(trimmed.to_string())
}

#[derive(Clone)]
pub struct Qwen3CoderParser;

impl ToolCallParser for Qwen3CoderParser {
    fn names(&self) -> &[&str] {
        &["qwen3_coder"]
    }

    fn format_tool_calls(&self, content: Option<&str>, tool_calls: &[ToolCall]) -> String {
        let mut output = String::new();
        if let Some(c) = content {
            if !c.is_empty() {
                output.push_str(c);
                output.push('\n');
            }
        }
        for call in tool_calls {
            output.push_str("<tool_call>\n");
            output.push_str(&format!("<function={}>\n", call.name));
            if let Some(obj) = call.arguments.as_object() {
                for (k, v) in obj {
                    let val_str = match v {
                        serde_json::Value::String(s) => s.clone(),
                        other => serde_json::to_string(other).unwrap_or_default(),
                    };
                    output.push_str(&format!("<parameter={}>{}</parameter>\n", k, val_str));
                }
            }
            output.push_str("</function>\n</tool_call>\n");
        }
        output.trim_end().to_string()
    }

    fn format_tool_response(&self, response: &ToolResponse) -> String {
        format!(
            "<tool_response>\n{}\n</tool_response>",
            serde_json::to_string(&response.content).unwrap_or_default()
        )
    }

    fn parse(&self, text: &str) -> ParseResult {
        let content = {
            let first = text
                .find("<tool_call>")
                .or_else(|| text.find("<function="));
            match first {
                Some(pos) if pos > 0 => Some(text[..pos].trim().to_string()),
                Some(_) => None,
                None => Some(text.to_string()),
            }
        };

        let mut tool_calls = Vec::new();

        for tc_cap in TOOL_CALL_REGEX.captures_iter(text) {
            let block = tc_cap.get(1).or_else(|| tc_cap.get(2)).map(|m| m.as_str()).unwrap_or("");

            for func_cap in FUNCTION_REGEX.captures_iter(block) {
                let func_name = func_cap
                    .get(1)
                    .or_else(|| func_cap.get(3))
                    .map(|m| m.as_str().trim())
                    .unwrap_or("");
                let params_block = func_cap
                    .get(2)
                    .or_else(|| func_cap.get(4))
                    .map(|m| m.as_str())
                    .unwrap_or("");

                let mut args = serde_json::Map::new();
                for param_cap in PARAMETER_REGEX.captures_iter(params_block) {
                    let key = param_cap.get(1).map(|m| m.as_str().trim()).unwrap_or("");
                    let val = param_cap.get(2).map(|m| m.as_str().trim()).unwrap_or("");
                    if !key.is_empty() {
                        args.insert(key.to_string(), try_convert_value(val));
                    }
                }

                if !func_name.is_empty() {
                    tool_calls.push(ToolCall {
                        id: gen_call_id(),
                        name: func_name.to_string(),
                        arguments: serde_json::Value::Object(args),
                    });
                }
            }
        }

        if tool_calls.is_empty() {
            (Some(text.to_string()), None)
        } else {
            (content, Some(tool_calls))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qwen3_coder_parse() {
        let text = r#"<tool_call>
<function=bash>
<parameter=command>ls -la</parameter>
</function>
</tool_call>"#;
        let (_, calls) = Qwen3CoderParser.parse(text);
        let calls = calls.unwrap();
        assert_eq!(calls[0].name, "bash");
        assert_eq!(calls[0].arguments["command"], "ls -la");
    }

    #[test]
    fn test_qwen3_coder_type_conversion() {
        let text = r#"<tool_call>
<function=search>
<parameter=query>rust</parameter>
<parameter=limit>10</parameter>
<parameter=exact>true</parameter>
<parameter=data>null</parameter>
</function>
</tool_call>"#;
        let (_, calls) = Qwen3CoderParser.parse(text);
        let calls = calls.unwrap();
        assert_eq!(calls[0].arguments["query"], "rust");
        assert_eq!(calls[0].arguments["limit"], 10);
        assert_eq!(calls[0].arguments["exact"], true);
        assert!(calls[0].arguments["data"].is_null());
    }

    #[test]
    fn test_qwen3_coder_unclosed() {
        let text = r#"<tool_call>
<function=bash>
<parameter=command>ls</parameter>"#;
        let (_, calls) = Qwen3CoderParser.parse(text);
        let calls = calls.unwrap();
        assert_eq!(calls[0].name, "bash");
    }
}
```

- [ ] **Step 7: Register all parsers in `src/parsers/mod.rs`**

Add module declarations and register in `ParserRegistry::new()`:
```rust
pub mod hermes;
pub mod llama;
pub mod mistral;
pub mod deepseek;
pub mod glm;
pub mod kimi;
pub mod qwen_coder;
```

Update `ParserRegistry::new()`:
```rust
pub fn new() -> Self {
    let mut registry = Self {
        parsers: HashMap::new(),
    };
    registry.register(Box::new(hermes::HermesParser));
    registry.register(Box::new(hermes::LongcatParser));
    registry.register(Box::new(hermes::QwenParser));
    registry.register(Box::new(llama::LlamaParser));
    registry.register(Box::new(mistral::MistralParser));
    registry.register(Box::new(deepseek::DeepSeekV3Parser));
    registry.register(Box::new(deepseek::DeepSeekV31Parser));
    registry.register(Box::new(glm::Glm45Parser));
    registry.register(Box::new(glm::Glm47Parser));
    registry.register(Box::new(kimi::KimiK2Parser));
    registry.register(Box::new(qwen_coder::Qwen3CoderParser));
    registry
}
```

- [ ] **Step 8: Run all parser tests**

Run: `cargo test parsers -- --nocapture`
Expected: All tests pass across all parser modules.

- [ ] **Step 9: Commit**

```bash
git add src/parsers/
git commit -m "feat: add 11 tool-call parsers (Llama, Mistral, DeepSeek, GLM, Kimi, Qwen3-Coder)"
```

---

### Task 4: ShareGPT Export Converter (`mchact export`)

**Files:**
- Create: `src/export.rs`
- Modify: `src/lib.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write export module with ShareGPT conversion**

Create `src/export.rs`:
```rust
use crate::parsers::{ParserRegistry, ToolCall, ToolResponse};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_index: Option<u64>,
    pub conversations: Vec<ShareGptTurn>,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub metadata: serde_json::Value,
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

fn convert_reasoning(content: &str) -> String {
    content
        .replace("<REASONING_SCRATCHPAD>", "<think>")
        .replace("</REASONING_SCRATCHPAD>", "</think>")
}

pub fn openai_to_sharegpt(
    entry: &TrajectoryEntry,
    parser: &dyn crate::parsers::ToolCallParser,
) -> ShareGptEntry {
    let mut conversations = Vec::new();

    for msg in &entry.messages {
        let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");
        let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");
        let reasoning = msg.get("reasoning").and_then(|v| v.as_str()).unwrap_or("");

        match role {
            "system" => {
                conversations.push(ShareGptTurn {
                    from: "system".into(),
                    value: content.to_string(),
                });
            }
            "user" => {
                conversations.push(ShareGptTurn {
                    from: "human".into(),
                    value: content.to_string(),
                });
            }
            "assistant" => {
                let tool_calls_raw = msg.get("tool_calls").and_then(|v| v.as_array());

                // Build think block
                let think_content = if !reasoning.is_empty() {
                    reasoning.to_string()
                } else {
                    let converted = convert_reasoning(content);
                    if converted.contains("<think>") {
                        // Already has think tags from scratchpad conversion
                        String::new()
                    } else {
                        String::new()
                    }
                };

                let base_content = convert_reasoning(content);

                let mut value = if !think_content.is_empty() {
                    format!("<think>\n{}\n</think>\n{}", think_content, base_content)
                } else if !base_content.contains("<think>") {
                    format!("<think>\n\n</think>\n{}", base_content)
                } else {
                    base_content
                };

                // Format tool calls via parser
                if let Some(tc_array) = tool_calls_raw {
                    let calls: Vec<ToolCall> = tc_array
                        .iter()
                        .filter_map(|tc| {
                            let func = tc.get("function")?;
                            Some(ToolCall {
                                id: tc.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                name: func.get("name").and_then(|v| v.as_str())?.to_string(),
                                arguments: func
                                    .get("arguments")
                                    .and_then(|v| v.as_str())
                                    .and_then(|s| serde_json::from_str(s).ok())
                                    .unwrap_or_default(),
                            })
                        })
                        .collect();

                    if !calls.is_empty() {
                        let formatted = parser.format_tool_calls(None, &calls);
                        value = format!("{}\n{}", value.trim_end(), formatted);
                    }
                }

                conversations.push(ShareGptTurn {
                    from: "gpt".into(),
                    value: value.trim().to_string(),
                });
            }
            "tool" => {
                let tool_call_id = msg
                    .get("tool_call_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let name = msg.get("name").and_then(|v| v.as_str()).unwrap_or("");

                let content_val = if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(content)
                {
                    parsed
                } else {
                    serde_json::Value::String(content.to_string())
                };

                let response = ToolResponse {
                    tool_call_id: tool_call_id.to_string(),
                    name: name.to_string(),
                    content: content_val,
                };

                conversations.push(ShareGptTurn {
                    from: "tool".into(),
                    value: parser.format_tool_response(&response),
                });
            }
            _ => {}
        }
    }

    ShareGptEntry {
        prompt_index: entry.prompt_index,
        conversations,
        metadata: entry.metadata.clone(),
        completed: entry.completed,
        partial: entry.partial,
        api_calls: entry.api_calls,
        toolsets_used: entry.toolsets_used.clone(),
        tool_stats: entry.tool_stats.clone(),
        tool_error_counts: entry.tool_error_counts.clone(),
    }
}

pub fn export_file(
    input: &Path,
    output: &Path,
    format: &str,
    parser_name: &str,
    filter_completed: bool,
    filter_min_tools: Option<u64>,
) -> Result<ExportStats, String> {
    let registry = ParserRegistry::new();

    let reader =
        BufReader::new(std::fs::File::open(input).map_err(|e| format!("Cannot open input: {e}"))?);
    let mut writer = std::io::BufWriter::new(
        std::fs::File::create(output).map_err(|e| format!("Cannot create output: {e}"))?,
    );

    let mut stats = ExportStats::default();

    for line in reader.lines() {
        let line = line.map_err(|e| format!("Read error: {e}"))?;
        if line.trim().is_empty() {
            continue;
        }

        let entry: TrajectoryEntry =
            serde_json::from_str(&line).map_err(|e| format!("Invalid JSON: {e}"))?;

        stats.total += 1;

        if filter_completed && !entry.completed {
            stats.filtered += 1;
            continue;
        }

        if let Some(min) = filter_min_tools {
            let tool_count: u64 = entry
                .messages
                .iter()
                .filter(|m| m.get("role").and_then(|v| v.as_str()) == Some("tool"))
                .count() as u64;
            if tool_count < min {
                stats.filtered += 1;
                continue;
            }
        }

        match format {
            "openai" => {
                writer
                    .write_all(line.as_bytes())
                    .map_err(|e| format!("Write error: {e}"))?;
                writer
                    .write_all(b"\n")
                    .map_err(|e| format!("Write error: {e}"))?;
            }
            "sharegpt" => {
                let parser = registry
                    .get(parser_name)
                    .ok_or_else(|| format!("Unknown parser: {parser_name}"))?;
                let sharegpt = openai_to_sharegpt(&entry, parser);
                let json = serde_json::to_string(&sharegpt)
                    .map_err(|e| format!("Serialize error: {e}"))?;
                writer
                    .write_all(json.as_bytes())
                    .map_err(|e| format!("Write error: {e}"))?;
                writer
                    .write_all(b"\n")
                    .map_err(|e| format!("Write error: {e}"))?;
            }
            other => return Err(format!("Unknown format: {other}")),
        }

        stats.exported += 1;
    }

    Ok(stats)
}

#[derive(Debug, Default)]
pub struct ExportStats {
    pub total: u64,
    pub exported: u64,
    pub filtered: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::hermes::HermesParser;

    #[test]
    fn test_openai_to_sharegpt_basic() {
        let entry = TrajectoryEntry {
            prompt_index: Some(0),
            messages: vec![
                serde_json::json!({"role": "system", "content": "You are helpful."}),
                serde_json::json!({"role": "user", "content": "Hello"}),
                serde_json::json!({"role": "assistant", "content": "Hi there!"}),
            ],
            metadata: serde_json::json!({}),
            completed: true,
            partial: false,
            api_calls: 1,
            toolsets_used: vec![],
            tool_stats: serde_json::Value::Null,
            tool_error_counts: serde_json::Value::Null,
        };

        let result = openai_to_sharegpt(&entry, &HermesParser);
        assert_eq!(result.conversations.len(), 3);
        assert_eq!(result.conversations[0].from, "system");
        assert_eq!(result.conversations[1].from, "human");
        assert_eq!(result.conversations[2].from, "gpt");
        assert!(result.conversations[2].value.contains("<think>"));
    }

    #[test]
    fn test_openai_to_sharegpt_with_tool_calls() {
        let entry = TrajectoryEntry {
            prompt_index: Some(0),
            messages: vec![
                serde_json::json!({"role": "system", "content": "System"}),
                serde_json::json!({"role": "user", "content": "Search for rust"}),
                serde_json::json!({
                    "role": "assistant",
                    "content": "Let me search.",
                    "tool_calls": [{
                        "id": "call_abc",
                        "type": "function",
                        "function": {
                            "name": "web_search",
                            "arguments": "{\"query\": \"rust\"}"
                        }
                    }]
                }),
                serde_json::json!({
                    "role": "tool",
                    "tool_call_id": "call_abc",
                    "name": "web_search",
                    "content": "{\"results\": []}"
                }),
            ],
            metadata: serde_json::Value::Null,
            completed: true,
            partial: false,
            api_calls: 2,
            toolsets_used: vec!["web".into()],
            tool_stats: serde_json::Value::Null,
            tool_error_counts: serde_json::Value::Null,
        };

        let result = openai_to_sharegpt(&entry, &HermesParser);
        assert_eq!(result.conversations.len(), 4);
        assert!(result.conversations[2].value.contains("<tool_call>"));
        assert!(result.conversations[3].value.contains("<tool_response>"));
    }
}
```

- [ ] **Step 2: Register module and add CLI subcommand**

Add to `src/lib.rs` after `pub mod distributions;`:
```rust
pub mod export;
```

Add to `src/main.rs` in the `MainCommand` enum:
```rust
/// Export trajectories to different formats
Export {
    /// Input trajectories JSONL file
    input: PathBuf,
    /// Output format: openai or sharegpt
    #[arg(long, default_value = "openai")]
    format: String,
    /// Tool-call parser for ShareGPT format
    #[arg(long, default_value = "hermes")]
    parser: String,
    /// Output file (default: derives from input)
    #[arg(long)]
    output: Option<PathBuf>,
    /// Only export completed trajectories
    #[arg(long)]
    filter_completed: bool,
    /// Minimum tool calls to include
    #[arg(long)]
    filter_min_tools: Option<u64>,
},
```

Add the match arm in `main()`:
```rust
Some(MainCommand::Export {
    input,
    format,
    parser,
    output,
    filter_completed,
    filter_min_tools,
}) => {
    let output = output.unwrap_or_else(|| {
        let stem = input.file_stem().unwrap_or_default().to_string_lossy();
        input.with_file_name(format!("{}_{}.jsonl", stem, format))
    });
    println!("Exporting {} → {} (format: {}, parser: {})", input.display(), output.display(), format, parser);
    match mchact::export::export_file(&input, &output, &format, &parser, filter_completed, filter_min_tools) {
        Ok(stats) => {
            println!("Done. {} exported, {} filtered, {} total.", stats.exported, stats.filtered, stats.total);
        }
        Err(e) => {
            eprintln!("Export failed: {e}");
            std::process::exit(1);
        }
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test export -- --nocapture`
Expected: All tests pass.

- [ ] **Step 4: Verify CLI compiles**

Run: `cargo build`
Expected: Clean build.

- [ ] **Step 5: Commit**

```bash
git add src/export.rs src/lib.rs src/main.rs
git commit -m "feat: add trajectory export with ShareGPT conversion and CLI command"
```

---

### Task 5: Batch Worker Process (`mchact worker`)

**Files:**
- Create: `src/batch_worker.rs`
- Modify: `src/lib.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write batch worker module**

Create `src/batch_worker.rs`:
```rust
//! Worker process for batch trajectory generation.
//! Invoked by the coordinator as: `mchact worker --batch-file <path> --config <path> [options]`
//!
//! Reads prompts from batch file, runs each through process_with_agent,
//! extracts tool stats, writes trajectory entries to output JSONL.

use crate::config::Config;
use crate::distributions::{self, Distribution};
use crate::export::TrajectoryEntry;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

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
        Self { count: 0, success: 0, failure: 0 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningStats {
    pub total_assistant_turns: u64,
    pub turns_with_reasoning: u64,
    pub turns_without_reasoning: u64,
    pub has_any_reasoning: bool,
}

/// Determine if a tool response indicates success or failure.
pub fn is_tool_success(content: &str) -> bool {
    if content.is_empty() {
        return false;
    }
    if content.trim().to_lowercase().starts_with("error:") {
        return false;
    }
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(content) {
        if let Some(obj) = parsed.as_object() {
            // Check top-level error field
            if let Some(err) = obj.get("error") {
                if !err.is_null() {
                    return false;
                }
            }
            // Check nested content.error (terminal tool pattern)
            if let Some(inner) = obj.get("content").and_then(|v| v.as_object()) {
                if let Some(err) = inner.get("error") {
                    if !err.is_null() {
                        return false;
                    }
                }
            }
            // Check "success": false
            if obj.get("success") == Some(&serde_json::Value::Bool(false)) {
                return false;
            }
        }
    }
    true
}

/// Extract tool usage statistics from OpenAI-format messages.
pub fn extract_tool_stats(messages: &[serde_json::Value]) -> HashMap<String, ToolStat> {
    let mut stats: HashMap<String, ToolStat> = HashMap::new();
    let mut call_id_to_name: HashMap<String, String> = HashMap::new();

    for msg in messages {
        let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");

        if role == "assistant" {
            if let Some(tool_calls) = msg.get("tool_calls").and_then(|v| v.as_array()) {
                for tc in tool_calls {
                    let name = tc
                        .get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let id = tc.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    if !name.is_empty() {
                        stats.entry(name.to_string()).or_insert_with(ToolStat::zero).count += 1;
                        if !id.is_empty() {
                            call_id_to_name.insert(id.to_string(), name.to_string());
                        }
                    }
                }
            }
        } else if role == "tool" {
            let call_id = msg.get("tool_call_id").and_then(|v| v.as_str()).unwrap_or("");
            let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");

            if let Some(tool_name) = call_id_to_name.get(call_id) {
                let stat = stats.entry(tool_name.clone()).or_insert_with(ToolStat::zero);
                if is_tool_success(content) {
                    stat.success += 1;
                } else {
                    stat.failure += 1;
                }
            }
        }
    }

    stats
}

/// Extract reasoning statistics from messages.
pub fn extract_reasoning_stats(messages: &[serde_json::Value]) -> ReasoningStats {
    let mut total = 0u64;
    let mut with_reasoning = 0u64;

    for msg in messages {
        if msg.get("role").and_then(|v| v.as_str()) != Some("assistant") {
            continue;
        }
        total += 1;
        let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");
        let reasoning = msg.get("reasoning").and_then(|v| v.as_str()).unwrap_or("");
        if content.contains("<REASONING_SCRATCHPAD>") || !reasoning.is_empty() {
            with_reasoning += 1;
        }
    }

    ReasoningStats {
        total_assistant_turns: total,
        turns_with_reasoning: with_reasoning,
        turns_without_reasoning: total.saturating_sub(with_reasoning),
        has_any_reasoning: with_reasoning > 0,
    }
}

/// Normalize tool stats to include all registered tools.
pub fn normalize_tool_stats(
    raw: &HashMap<String, ToolStat>,
    all_tools: &[String],
) -> HashMap<String, ToolStat> {
    let mut normalized = HashMap::new();
    for tool in all_tools {
        normalized.insert(
            tool.clone(),
            raw.get(tool).cloned().unwrap_or_else(ToolStat::zero),
        );
    }
    // Include unexpected tools (new tools added at runtime)
    for (tool, stat) in raw {
        normalized.entry(tool.clone()).or_insert_with(|| stat.clone());
    }
    normalized
}

/// Normalize tool error counts.
pub fn normalize_error_counts(
    raw: &HashMap<String, ToolStat>,
    all_tools: &[String],
) -> HashMap<String, u64> {
    let mut counts = HashMap::new();
    for tool in all_tools {
        counts.insert(
            tool.clone(),
            raw.get(tool).map(|s| s.failure).unwrap_or(0),
        );
    }
    for (tool, stat) in raw {
        counts.entry(tool.clone()).or_insert(stat.failure);
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_tool_success_empty() {
        assert!(!is_tool_success(""));
    }

    #[test]
    fn test_is_tool_success_error_prefix() {
        assert!(!is_tool_success("Error: file not found"));
        assert!(!is_tool_success("ERROR: timeout"));
    }

    #[test]
    fn test_is_tool_success_json_error() {
        assert!(!is_tool_success(r#"{"error": "not found"}"#));
        assert!(is_tool_success(r#"{"error": null, "data": "ok"}"#));
    }

    #[test]
    fn test_is_tool_success_nested_error() {
        assert!(!is_tool_success(r#"{"content": {"error": "timeout"}}"#));
    }

    #[test]
    fn test_is_tool_success_false() {
        assert!(!is_tool_success(r#"{"success": false}"#));
    }

    #[test]
    fn test_is_tool_success_normal() {
        assert!(is_tool_success("file1.txt\nfile2.txt"));
        assert!(is_tool_success(r#"{"data": [1,2,3]}"#));
    }

    #[test]
    fn test_extract_tool_stats() {
        let messages = vec![
            serde_json::json!({
                "role": "assistant",
                "content": "",
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {"name": "bash", "arguments": "{\"command\":\"ls\"}"}
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call_1",
                "content": "file1.txt"
            }),
        ];
        let stats = extract_tool_stats(&messages);
        assert_eq!(stats["bash"].count, 1);
        assert_eq!(stats["bash"].success, 1);
        assert_eq!(stats["bash"].failure, 0);
    }

    #[test]
    fn test_extract_reasoning_stats() {
        let messages = vec![
            serde_json::json!({"role": "assistant", "content": "No reasoning"}),
            serde_json::json!({"role": "assistant", "content": "<REASONING_SCRATCHPAD>thinking</REASONING_SCRATCHPAD>answer"}),
            serde_json::json!({"role": "assistant", "content": "plain", "reasoning": "deep thought"}),
        ];
        let stats = extract_reasoning_stats(&messages);
        assert_eq!(stats.total_assistant_turns, 3);
        assert_eq!(stats.turns_with_reasoning, 2);
        assert_eq!(stats.turns_without_reasoning, 1);
    }

    #[test]
    fn test_normalize_tool_stats() {
        let mut raw = HashMap::new();
        raw.insert("bash".to_string(), ToolStat { count: 5, success: 4, failure: 1 });
        let all = vec!["bash".to_string(), "web_search".to_string(), "read_file".to_string()];
        let normalized = normalize_tool_stats(&raw, &all);
        assert_eq!(normalized["bash"].count, 5);
        assert_eq!(normalized["web_search"].count, 0);
        assert_eq!(normalized["read_file"].count, 0);
    }
}
```

- [ ] **Step 2: Register module in `src/lib.rs`**

Add after `pub mod export;`:
```rust
pub mod batch_worker;
```

- [ ] **Step 3: Run tests**

Run: `cargo test batch_worker -- --nocapture`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/batch_worker.rs src/lib.rs
git commit -m "feat: add batch worker with tool stats extraction and normalization"
```

---

### Task 6: Batch Coordinator (`mchact batch`)

**Files:**
- Create: `src/batch.rs`
- Modify: `src/lib.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write batch coordinator**

Create `src/batch.rs`:
```rust
//! Batch trajectory generation coordinator.
//! Spawns N worker processes, manages checkpoints, combines results.

use crate::batch_worker::{
    normalize_error_counts, normalize_tool_stats, BatchPrompt, ReasoningStats, ToolStat,
};
use crate::distributions::{self, DistributionMap};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct BatchConfig {
    pub dataset: PathBuf,
    pub workers: usize,
    pub batch_size: usize,
    pub distribution: String,
    pub max_iterations: usize,
    pub model: Option<String>,
    pub run_name: String,
    pub output_dir: PathBuf,
    pub resume: bool,
    pub max_samples: Option<usize>,
    pub config_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub run_name: String,
    pub completed_prompts: Vec<u64>,
    pub batch_stats: HashMap<String, BatchStat>,
    pub last_updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchStat {
    pub processed: u64,
    pub skipped: u64,
    pub failed: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunStatistics {
    pub run_name: String,
    pub distribution: String,
    pub total_prompts: u64,
    pub total_batches: u64,
    pub batch_size: u64,
    pub model: String,
    pub completed_at: String,
    pub duration_seconds: f64,
    pub tool_statistics: HashMap<String, ToolStatWithRates>,
    pub reasoning_statistics: ReasoningStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolStatWithRates {
    pub count: u64,
    pub success: u64,
    pub failure: u64,
    pub success_rate: f64,
    pub failure_rate: f64,
}

/// Load prompts from JSONL dataset.
pub fn load_dataset(path: &Path, max_samples: Option<usize>) -> Result<Vec<BatchPrompt>, String> {
    let reader = BufReader::new(
        std::fs::File::open(path).map_err(|e| format!("Cannot open dataset: {e}"))?,
    );
    let mut prompts = Vec::new();
    for (idx, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| format!("Read error at line {}: {e}", idx + 1))?;
        if line.trim().is_empty() {
            continue;
        }
        let mut entry: serde_json::Value =
            serde_json::from_str(&line).map_err(|e| format!("Invalid JSON at line {}: {e}", idx + 1))?;

        let prompt = entry
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("Missing 'prompt' field at line {}", idx + 1))?
            .to_string();

        let toolsets = entry
            .get("toolsets")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect());

        let image = entry
            .get("image")
            .and_then(|v| v.as_str())
            .map(String::from);

        prompts.push(BatchPrompt {
            prompt_index: idx as u64,
            prompt,
            toolsets,
            image,
        });

        if let Some(max) = max_samples {
            if prompts.len() >= max {
                break;
            }
        }
    }
    Ok(prompts)
}

/// Split prompts into batches of given size.
pub fn split_batches(prompts: Vec<BatchPrompt>, batch_size: usize) -> Vec<Vec<BatchPrompt>> {
    prompts.chunks(batch_size).map(|c| c.to_vec()).collect()
}

/// Load checkpoint for resume.
pub fn load_checkpoint(output_dir: &Path) -> Option<Checkpoint> {
    let path = output_dir.join("checkpoint.json");
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Save checkpoint.
pub fn save_checkpoint(output_dir: &Path, checkpoint: &Checkpoint) -> Result<(), String> {
    let path = output_dir.join("checkpoint.json");
    let json = serde_json::to_string_pretty(checkpoint)
        .map_err(|e| format!("Serialize checkpoint: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("Write checkpoint: {e}"))?;
    Ok(())
}

/// Content-based resume: scan batch output files for completed prompt text.
pub fn find_completed_prompts(output_dir: &Path) -> HashSet<String> {
    let mut completed = HashSet::new();
    let pattern = output_dir.join("batch_*.jsonl");
    if let Ok(entries) = glob::glob(&pattern.to_string_lossy()) {
        for entry in entries.flatten() {
            if let Ok(reader) = std::fs::File::open(&entry).map(BufReader::new) {
                for line in reader.lines().flatten() {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                        if let Some(msgs) = val.get("messages").and_then(|v| v.as_array()) {
                            // Find user prompt (second message typically)
                            for msg in msgs {
                                if msg.get("role").and_then(|v| v.as_str()) == Some("user") {
                                    if let Some(content) = msg.get("content").and_then(|v| v.as_str()) {
                                        completed.insert(content.to_string());
                                    }
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    completed
}

/// Filter dataset by removing already-completed prompts.
pub fn filter_completed(
    prompts: Vec<BatchPrompt>,
    completed: &HashSet<String>,
) -> Vec<BatchPrompt> {
    prompts
        .into_iter()
        .filter(|p| !completed.contains(&p.prompt))
        .collect()
}

/// Combine all batch_*.jsonl into final trajectories.jsonl with validation.
pub fn combine_batches(
    output_dir: &Path,
    all_tool_names: &[String],
) -> Result<CombineResult, String> {
    let pattern = output_dir.join("batch_*.jsonl");
    let final_path = output_dir.join("trajectories.jsonl");
    let mut writer = std::io::BufWriter::new(
        std::fs::File::create(&final_path).map_err(|e| format!("Create output: {e}"))?,
    );

    let tool_set: HashSet<&str> = all_tool_names.iter().map(|s| s.as_str()).collect();
    let mut result = CombineResult::default();

    let entries = glob::glob(&pattern.to_string_lossy())
        .map_err(|e| format!("Glob error: {e}"))?;

    for entry in entries.flatten() {
        let reader = BufReader::new(
            std::fs::File::open(&entry).map_err(|e| format!("Open batch file: {e}"))?,
        );
        for line in reader.lines() {
            let line = line.map_err(|e| format!("Read: {e}"))?;
            result.total += 1;

            match serde_json::from_str::<serde_json::Value>(&line) {
                Ok(val) => {
                    // Validate tool names
                    if let Some(stats) = val.get("tool_stats").and_then(|v| v.as_object()) {
                        let invalid: Vec<_> = stats
                            .keys()
                            .filter(|k| !tool_set.contains(k.as_str()))
                            .collect();
                        if !invalid.is_empty() {
                            result.filtered += 1;
                            continue;
                        }
                    }
                    writer.write_all(line.as_bytes()).map_err(|e| format!("Write: {e}"))?;
                    writer.write_all(b"\n").map_err(|e| format!("Write: {e}"))?;
                    result.valid += 1;
                }
                Err(_) => {
                    result.filtered += 1;
                }
            }
        }
    }

    Ok(result)
}

#[derive(Debug, Default)]
pub struct CombineResult {
    pub total: u64,
    pub valid: u64,
    pub filtered: u64,
}

/// Calculate aggregate tool statistics with success/failure rates.
pub fn aggregate_tool_stats(
    entries: &[HashMap<String, ToolStat>],
) -> HashMap<String, ToolStatWithRates> {
    let mut totals: HashMap<String, ToolStat> = HashMap::new();

    for entry_stats in entries {
        for (tool, stat) in entry_stats {
            let total = totals.entry(tool.clone()).or_insert_with(ToolStat::zero);
            total.count += stat.count;
            total.success += stat.success;
            total.failure += stat.failure;
        }
    }

    totals
        .into_iter()
        .map(|(name, stat)| {
            let total_calls = stat.success + stat.failure;
            let (success_rate, failure_rate) = if total_calls > 0 {
                (
                    (stat.success as f64 / total_calls as f64 * 100.0 * 100.0).round() / 100.0,
                    (stat.failure as f64 / total_calls as f64 * 100.0 * 100.0).round() / 100.0,
                )
            } else {
                (0.0, 0.0)
            };
            (
                name,
                ToolStatWithRates {
                    count: stat.count,
                    success: stat.success,
                    failure: stat.failure,
                    success_rate,
                    failure_rate,
                },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_batches() {
        let prompts: Vec<BatchPrompt> = (0..25)
            .map(|i| BatchPrompt {
                prompt_index: i,
                prompt: format!("prompt {i}"),
                toolsets: None,
                image: None,
            })
            .collect();
        let batches = split_batches(prompts, 10);
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0].len(), 10);
        assert_eq!(batches[1].len(), 10);
        assert_eq!(batches[2].len(), 5);
    }

    #[test]
    fn test_filter_completed() {
        let prompts = vec![
            BatchPrompt { prompt_index: 0, prompt: "done".into(), toolsets: None, image: None },
            BatchPrompt { prompt_index: 1, prompt: "todo".into(), toolsets: None, image: None },
        ];
        let completed: HashSet<String> = ["done".to_string()].into();
        let filtered = filter_completed(prompts, &completed);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].prompt, "todo");
    }

    #[test]
    fn test_aggregate_tool_stats() {
        let entries = vec![
            HashMap::from([
                ("bash".into(), ToolStat { count: 5, success: 4, failure: 1 }),
            ]),
            HashMap::from([
                ("bash".into(), ToolStat { count: 3, success: 3, failure: 0 }),
            ]),
        ];
        let agg = aggregate_tool_stats(&entries);
        assert_eq!(agg["bash"].count, 8);
        assert_eq!(agg["bash"].success, 7);
        assert_eq!(agg["bash"].failure, 1);
        assert!((agg["bash"].success_rate - 87.5).abs() < 0.01);
    }
}
```

- [ ] **Step 2: Register module and add CLI subcommand**

Add to `src/lib.rs`:
```rust
pub mod batch;
```

Add `Batch` variant to `MainCommand` enum in `src/main.rs`:
```rust
/// Generate training trajectories from a prompt dataset
Batch {
    /// JSONL dataset file with 'prompt' field per line
    dataset: PathBuf,
    /// Number of parallel worker processes
    #[arg(long, default_value = "4")]
    workers: usize,
    /// Prompts per batch
    #[arg(long, default_value = "10")]
    batch_size: usize,
    /// Toolset distribution name
    #[arg(long, default_value = "default")]
    distribution: String,
    /// Max tool-call iterations per prompt
    #[arg(long, default_value = "10")]
    max_iterations: usize,
    /// Override model
    #[arg(long)]
    model: Option<String>,
    /// Output directory name
    #[arg(long)]
    run_name: Option<String>,
    /// Resume from checkpoint
    #[arg(long)]
    resume: bool,
    /// Truncate dataset to N prompts
    #[arg(long)]
    max_samples: Option<usize>,
    /// Output directory
    #[arg(long)]
    output: Option<PathBuf>,
},
```

Add match arm (placeholder — full worker spawning in next task):
```rust
Some(MainCommand::Batch {
    dataset,
    workers,
    batch_size,
    distribution,
    max_iterations,
    model,
    run_name,
    resume,
    max_samples,
    output,
}) => {
    let run_name = run_name.unwrap_or_else(|| {
        format!("run-{}", chrono::Utc::now().format("%Y%m%d-%H%M"))
    });
    let output_dir = output.unwrap_or_else(|| PathBuf::from("training-runs").join(&run_name));
    std::fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    println!("Batch run: {} (workers: {}, distribution: {})", run_name, workers, distribution);

    let prompts = mchact::batch::load_dataset(&dataset, max_samples)
        .expect("Failed to load dataset");
    println!("Loaded {} prompts", prompts.len());

    let prompts = if resume {
        let completed = mchact::batch::find_completed_prompts(&output_dir);
        let filtered = mchact::batch::filter_completed(prompts, &completed);
        println!("Resuming: {} prompts remaining ({} already done)", filtered.len(), completed.len());
        filtered
    } else {
        prompts
    };

    if prompts.is_empty() {
        println!("No prompts to process.");
        return Ok(());
    }

    let batches = mchact::batch::split_batches(prompts, batch_size);
    println!("Split into {} batches of up to {} prompts", batches.len(), batch_size);

    // TODO: Task 7 will implement worker process spawning here
    println!("Worker spawning not yet implemented. Use Task 7.");
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test batch -- --nocapture`
Expected: All tests pass.

- [ ] **Step 4: Verify build**

Run: `cargo build`
Expected: Clean build.

- [ ] **Step 5: Commit**

```bash
git add src/batch.rs src/lib.rs src/main.rs
git commit -m "feat: add batch coordinator with dataset loading, checkpointing, and validation"
```

---

### Task 7: Worker Process Spawning & IPC

**Files:**
- Modify: `src/main.rs` (add `Worker` subcommand + batch coordinator spawning)
- Modify: `src/batch.rs` (add spawn_workers function)

- [ ] **Step 1: Add `Worker` subcommand to main.rs**

Add to `MainCommand` enum:
```rust
/// Internal: worker process for batch trajectory generation
#[command(hide = true)]
Worker {
    /// Batch file to process
    #[arg(long)]
    batch_file: PathBuf,
    /// Distribution name
    #[arg(long, default_value = "default")]
    distribution: String,
    /// Max tool iterations
    #[arg(long, default_value = "10")]
    max_iterations: usize,
    /// Override model
    #[arg(long)]
    model: Option<String>,
    /// Distributions YAML file
    #[arg(long)]
    distributions_file: Option<PathBuf>,
},
```

Add match arm for Worker (runs one batch, writes results to batch file path with `.out` suffix):
```rust
Some(MainCommand::Worker {
    batch_file,
    distribution,
    max_iterations,
    model,
    distributions_file,
}) => {
    let output_file = batch_file.with_extension("out.jsonl");

    // Load prompts from batch file
    let content = std::fs::read_to_string(&batch_file)
        .expect("Cannot read batch file");
    let prompts: Vec<mchact::batch_worker::BatchPrompt> = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();

    // Load distributions
    let dist_path = distributions_file.unwrap_or_else(|| PathBuf::from("training/distributions.yaml"));
    let dists = mchact::distributions::load_distributions(&dist_path)
        .unwrap_or_default();
    let dist = dists.get(&distribution);

    let config = Config::load()?;

    // Process each prompt
    let mut writer = std::io::BufWriter::new(
        std::fs::File::create(&output_file).expect("Cannot create output file"),
    );

    let rt = tokio::runtime::Runtime::new().unwrap();
    for prompt in &prompts {
        // Sample tools for this prompt
        let _sampled_tools = prompt.toolsets.clone().unwrap_or_else(|| {
            dist.map(|d| mchact::distributions::sample_tools(d))
                .unwrap_or_default()
        });

        // TODO: Actually run process_with_agent with sampled tools
        // For now, write a placeholder entry
        let entry = serde_json::json!({
            "prompt_index": prompt.prompt_index,
            "messages": [
                {"role": "system", "content": "You are a helpful assistant."},
                {"role": "user", "content": prompt.prompt},
            ],
            "metadata": {"batch_num": 0, "timestamp": chrono::Utc::now().to_rfc3339(), "model": model.as_deref().unwrap_or("default")},
            "completed": false,
            "partial": false,
            "api_calls": 0,
            "toolsets_used": [],
            "tool_stats": {},
            "tool_error_counts": {}
        });
        let _ = writeln!(writer, "{}", serde_json::to_string(&entry).unwrap());
    }

    println!("Worker completed: {} prompts → {}", prompts.len(), output_file.display());
}
```

- [ ] **Step 2: Add worker spawning to batch coordinator**

Add to `src/batch.rs`:
```rust
/// Spawn worker processes and wait for completion.
pub fn spawn_workers(
    batches: &[Vec<BatchPrompt>],
    output_dir: &Path,
    max_concurrent: usize,
    distribution: &str,
    max_iterations: usize,
    model: Option<&str>,
    config_path: Option<&Path>,
    distributions_file: Option<&Path>,
) -> Result<Vec<PathBuf>, String> {
    let exe = std::env::current_exe().map_err(|e| format!("Cannot find mchact binary: {e}"))?;
    let mut output_files = Vec::new();

    // Write batch files
    for (i, batch) in batches.iter().enumerate() {
        let batch_path = output_dir.join(format!("batch_{i}.input.jsonl"));
        let mut writer = std::io::BufWriter::new(
            std::fs::File::create(&batch_path)
                .map_err(|e| format!("Create batch file: {e}"))?,
        );
        for prompt in batch {
            let json = serde_json::to_string(prompt)
                .map_err(|e| format!("Serialize prompt: {e}"))?;
            writeln!(writer, "{json}").map_err(|e| format!("Write: {e}"))?;
        }
        output_files.push(output_dir.join(format!("batch_{i}.input.out.jsonl")));
    }

    // Spawn workers in chunks of max_concurrent
    for chunk_start in (0..batches.len()).step_by(max_concurrent) {
        let chunk_end = (chunk_start + max_concurrent).min(batches.len());
        let mut children = Vec::new();

        for i in chunk_start..chunk_end {
            let batch_path = output_dir.join(format!("batch_{i}.input.jsonl"));
            let mut cmd = Command::new(&exe);
            cmd.arg("worker")
                .arg("--batch-file")
                .arg(&batch_path)
                .arg("--distribution")
                .arg(distribution)
                .arg("--max-iterations")
                .arg(max_iterations.to_string());

            if let Some(m) = model {
                cmd.arg("--model").arg(m);
            }
            if let Some(p) = config_path {
                cmd.arg("--config").arg(p);
            }
            if let Some(d) = distributions_file {
                cmd.arg("--distributions-file").arg(d);
            }

            let child = cmd
                .spawn()
                .map_err(|e| format!("Spawn worker {i}: {e}"))?;
            children.push((i, child));
        }

        // Wait for all in chunk
        for (i, mut child) in children {
            let status = child.wait().map_err(|e| format!("Wait worker {i}: {e}"))?;
            if !status.success() {
                eprintln!("Warning: worker {i} exited with {status}");
            }
        }

        println!(
            "  Batch {}-{} complete ({}/{})",
            chunk_start,
            chunk_end - 1,
            chunk_end,
            batches.len()
        );
    }

    // Rename output files to final names
    let mut final_files = Vec::new();
    for i in 0..batches.len() {
        let src = output_dir.join(format!("batch_{i}.input.out.jsonl"));
        let dst = output_dir.join(format!("batch_{i}.jsonl"));
        if src.exists() {
            std::fs::rename(&src, &dst).map_err(|e| format!("Rename: {e}"))?;
            final_files.push(dst);
        }
    }

    Ok(final_files)
}
```

- [ ] **Step 3: Update Batch match arm to use spawn_workers**

Replace the TODO in the Batch match arm:
```rust
let start = std::time::Instant::now();

mchact::batch::spawn_workers(
    &batches,
    &output_dir,
    workers,
    &distribution,
    max_iterations,
    model.as_deref(),
    cli.config.as_deref(),
    None, // uses default training/distributions.yaml
).expect("Worker spawning failed");

// Combine results
let all_tool_names: Vec<String> = {
    // Collect all tool names from the registry
    // For now use a static list; later derive from ToolRegistry
    vec![
        "bash", "browser", "read_file", "write_file", "edit_file",
        "glob", "grep", "web_fetch", "web_search", "send_message",
        "image_generate", "text_to_speech", "video_generate",
        "read_document", "browser_vision", "mixture_of_agents",
    ].into_iter().map(String::from).collect()
};

let combine = mchact::batch::combine_batches(&output_dir, &all_tool_names)
    .expect("Failed to combine batches");

let duration = start.elapsed();
println!("Done. {} valid, {} filtered, {} total ({:.1}s)",
    combine.valid, combine.filtered, combine.total, duration.as_secs_f64());
```

- [ ] **Step 4: Verify build**

Run: `cargo build`
Expected: Clean build.

- [ ] **Step 5: Commit**

```bash
git add src/batch.rs src/main.rs
git commit -m "feat: add multi-process worker spawning with IPC via JSONL files"
```

---

### Task 8: Pipeline Shortcut (`mchact train`)

**Files:**
- Create: `src/train_pipeline.rs`
- Modify: `src/lib.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write pipeline module**

Create `src/train_pipeline.rs`:
```rust
//! Pipeline shortcut: batch → export → compress in one command.

use std::path::{Path, PathBuf};

pub struct PipelineConfig {
    pub dataset: PathBuf,
    pub workers: usize,
    pub batch_size: usize,
    pub distribution: String,
    pub max_iterations: usize,
    pub model: Option<String>,
    pub format: String,
    pub parser: String,
    pub compress: bool,
    pub target_tokens: usize,
    pub run_name: String,
    pub output_dir: PathBuf,
    pub resume: bool,
    pub config_path: Option<PathBuf>,
}

pub struct PipelineResult {
    pub trajectories: PathBuf,
    pub exported: Option<PathBuf>,
    pub compressed: Option<PathBuf>,
    pub statistics: PathBuf,
}

pub fn run_pipeline(config: &PipelineConfig) -> Result<PipelineResult, String> {
    std::fs::create_dir_all(&config.output_dir)
        .map_err(|e| format!("Create output dir: {e}"))?;

    // Step 1: Batch generation
    println!("Step 1/3: Generating trajectories...");
    let prompts = crate::batch::load_dataset(&config.dataset, None)?;
    let prompts = if config.resume {
        let completed = crate::batch::find_completed_prompts(&config.output_dir);
        crate::batch::filter_completed(prompts, &completed)
    } else {
        prompts
    };

    if prompts.is_empty() {
        return Err("No prompts to process.".into());
    }

    let batches = crate::batch::split_batches(prompts, config.batch_size);

    crate::batch::spawn_workers(
        &batches,
        &config.output_dir,
        config.workers,
        &config.distribution,
        config.max_iterations,
        config.model.as_deref(),
        config.config_path.as_deref(),
        None,
    )?;

    let all_tools: Vec<String> = vec![
        "bash", "browser", "read_file", "write_file", "edit_file",
        "glob", "grep", "web_fetch", "web_search", "send_message",
        "image_generate", "text_to_speech", "video_generate",
        "read_document", "browser_vision", "mixture_of_agents",
    ].into_iter().map(String::from).collect();

    crate::batch::combine_batches(&config.output_dir, &all_tools)?;
    let trajectories = config.output_dir.join("trajectories.jsonl");

    // Step 2: Export (if format != openai)
    let exported = if config.format != "openai" {
        println!("Step 2/3: Exporting to {} (parser: {})...", config.format, config.parser);
        let out_name = format!("trajectories_{}.jsonl", config.format);
        let output = config.output_dir.join(&out_name);
        crate::export::export_file(
            &trajectories,
            &output,
            &config.format,
            &config.parser,
            false,
            None,
        )?;
        Some(output)
    } else {
        println!("Step 2/3: Skipped (already OpenAI format).");
        None
    };

    // Step 3: Compress (if requested)
    let compress_input = exported.as_ref().unwrap_or(&trajectories);
    let compressed = if config.compress {
        println!("Step 3/3: Compressing to {} tokens...", config.target_tokens);
        let output = compress_input.with_extension("compressed.jsonl");

        // Invoke Python compression script
        let status = std::process::Command::new("python3")
            .arg("training/compress.py")
            .arg(compress_input)
            .arg("--target-tokens")
            .arg(config.target_tokens.to_string())
            .arg("--output")
            .arg(&output)
            .status()
            .map_err(|e| format!("Failed to run compression: {e}. Is Python 3.10+ installed?"))?;

        if !status.success() {
            return Err(format!("Compression failed with exit code: {status}"));
        }

        Some(output)
    } else {
        println!("Step 3/3: Skipped (compression not requested).");
        None
    };

    let statistics = config.output_dir.join("statistics.json");

    Ok(PipelineResult {
        trajectories,
        exported,
        compressed,
        statistics,
    })
}
```

- [ ] **Step 2: Register module and add CLI subcommand**

Add to `src/lib.rs`:
```rust
pub mod train_pipeline;
```

Add `Train` variant to `MainCommand`:
```rust
/// Run full training pipeline: batch → export → compress
Train {
    /// JSONL dataset file
    dataset: PathBuf,
    #[arg(long, default_value = "4")]
    workers: usize,
    #[arg(long, default_value = "10")]
    batch_size: usize,
    #[arg(long, default_value = "default")]
    distribution: String,
    #[arg(long, default_value = "10")]
    max_iterations: usize,
    #[arg(long)]
    model: Option<String>,
    #[arg(long, default_value = "openai")]
    format: String,
    #[arg(long, default_value = "hermes")]
    parser: String,
    /// Enable compression after export
    #[arg(long)]
    compress: bool,
    /// Compression target tokens
    #[arg(long, default_value = "15250")]
    target_tokens: usize,
    #[arg(long)]
    run_name: Option<String>,
    #[arg(long)]
    resume: bool,
    #[arg(long)]
    output: Option<PathBuf>,
},
```

Add match arm:
```rust
Some(MainCommand::Train {
    dataset, workers, batch_size, distribution, max_iterations,
    model, format, parser, compress, target_tokens, run_name, resume, output,
}) => {
    let run_name = run_name.unwrap_or_else(|| {
        format!("run-{}", chrono::Utc::now().format("%Y%m%d-%H%M"))
    });
    let output_dir = output.unwrap_or_else(|| PathBuf::from("training-runs").join(&run_name));

    let config = mchact::train_pipeline::PipelineConfig {
        dataset, workers, batch_size, distribution, max_iterations,
        model, format, parser, compress, target_tokens,
        run_name: run_name.clone(), output_dir: output_dir.clone(),
        resume, config_path: cli.config.clone(),
    };

    match mchact::train_pipeline::run_pipeline(&config) {
        Ok(result) => {
            println!("\nPipeline complete. Output: {}", output_dir.display());
            println!("  trajectories: {}", result.trajectories.display());
            if let Some(e) = &result.exported {
                println!("  exported: {}", e.display());
            }
            if let Some(c) = &result.compressed {
                println!("  compressed: {}", c.display());
            }
        }
        Err(e) => {
            eprintln!("Pipeline failed: {e}");
            std::process::exit(1);
        }
    }
}
```

- [ ] **Step 3: Verify build**

Run: `cargo build`
Expected: Clean build.

- [ ] **Step 4: Commit**

```bash
git add src/train_pipeline.rs src/lib.rs src/main.rs
git commit -m "feat: add mchact train pipeline shortcut (batch → export → compress)"
```

---

### Task 9: Training Agent Tools

**Files:**
- Create: `src/tools/training.rs`
- Modify: `src/tools/mod.rs`

- [ ] **Step 1: Write training agent tools**

Create `src/tools/training.rs`:
```rust
use super::{schema_object, Tool, ToolResult};
use async_trait::async_trait;
use serde_json::json;

pub struct BatchGenerateTool;

#[async_trait]
impl Tool for BatchGenerateTool {
    fn name(&self) -> &str {
        "batch_generate"
    }

    fn definition(&self) -> serde_json::Value {
        schema_object(
            "batch_generate",
            "Generate tool-calling trajectories from a prompt dataset. Spawns parallel workers.",
            json!({
                "type": "object",
                "required": ["dataset"],
                "properties": {
                    "dataset": {"type": "string", "description": "Path to JSONL file with 'prompt' field per line"},
                    "workers": {"type": "integer", "description": "Parallel worker processes (default: 4)"},
                    "distribution": {"type": "string", "description": "Toolset distribution name (default: 'default')"},
                    "max_iterations": {"type": "integer", "description": "Max tool-call iterations per prompt (default: 10)"},
                    "model": {"type": "string", "description": "Override model for generation"},
                    "run_name": {"type": "string", "description": "Output directory name"}
                }
            }),
        )
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let dataset = input.get("dataset").and_then(|v| v.as_str()).unwrap_or("");
        let workers = input.get("workers").and_then(|v| v.as_u64()).unwrap_or(4) as usize;
        let distribution = input.get("distribution").and_then(|v| v.as_str()).unwrap_or("default");
        let max_iterations = input.get("max_iterations").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
        let model = input.get("model").and_then(|v| v.as_str()).map(String::from);
        let run_name = input.get("run_name").and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| format!("run-{}", chrono::Utc::now().format("%Y%m%d-%H%M")));

        let dataset_path = std::path::Path::new(dataset);
        if !dataset_path.exists() {
            return ToolResult {
                content: json!({"error": format!("Dataset not found: {dataset}")}).to_string(),
                is_error: true,
            };
        }

        let output_dir = std::path::PathBuf::from("training-runs").join(&run_name);
        if let Err(e) = std::fs::create_dir_all(&output_dir) {
            return ToolResult {
                content: json!({"error": format!("Cannot create output dir: {e}")}).to_string(),
                is_error: true,
            };
        }

        let start = std::time::Instant::now();

        match crate::batch::load_dataset(dataset_path, None) {
            Ok(prompts) => {
                let total = prompts.len();
                let batches = crate::batch::split_batches(prompts, 10);

                match crate::batch::spawn_workers(
                    &batches, &output_dir, workers, distribution,
                    max_iterations, model.as_deref(), None, None,
                ) {
                    Ok(_) => {
                        let duration = start.elapsed();
                        ToolResult {
                            content: json!({
                                "run_name": run_name,
                                "output_dir": output_dir.to_string_lossy(),
                                "total_prompts": total,
                                "duration_seconds": duration.as_secs_f64(),
                            }).to_string(),
                            is_error: false,
                        }
                    }
                    Err(e) => ToolResult {
                        content: json!({"error": e}).to_string(),
                        is_error: true,
                    },
                }
            }
            Err(e) => ToolResult {
                content: json!({"error": e}).to_string(),
                is_error: true,
            },
        }
    }
}

pub struct ExportTrajectoriesTool;

#[async_trait]
impl Tool for ExportTrajectoriesTool {
    fn name(&self) -> &str {
        "export_trajectories"
    }

    fn definition(&self) -> serde_json::Value {
        schema_object(
            "export_trajectories",
            "Convert trajectories to different format with model-specific tool-call parsers.",
            json!({
                "type": "object",
                "required": ["input"],
                "properties": {
                    "input": {"type": "string", "description": "Path to trajectories.jsonl"},
                    "format": {"type": "string", "enum": ["openai", "sharegpt"], "description": "Output format (default: openai)"},
                    "parser": {"type": "string", "description": "Tool-call parser for ShareGPT (default: hermes)"},
                    "filter_completed": {"type": "boolean", "description": "Only completed trajectories"}
                }
            }),
        )
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let input_path = input.get("input").and_then(|v| v.as_str()).unwrap_or("");
        let format = input.get("format").and_then(|v| v.as_str()).unwrap_or("openai");
        let parser = input.get("parser").and_then(|v| v.as_str()).unwrap_or("hermes");
        let filter_completed = input.get("filter_completed").and_then(|v| v.as_bool()).unwrap_or(false);

        let input_path = std::path::Path::new(input_path);
        let output_path = input_path.with_extension(format!("{format}.jsonl"));

        match crate::export::export_file(input_path, &output_path, format, parser, filter_completed, None) {
            Ok(stats) => ToolResult {
                content: json!({
                    "output": output_path.to_string_lossy(),
                    "entries": stats.exported,
                    "filtered": stats.filtered,
                }).to_string(),
                is_error: false,
            },
            Err(e) => ToolResult {
                content: json!({"error": e}).to_string(),
                is_error: true,
            },
        }
    }
}

pub struct CompressTrajectoriesTool;

#[async_trait]
impl Tool for CompressTrajectoriesTool {
    fn name(&self) -> &str {
        "compress_trajectories"
    }

    fn definition(&self) -> serde_json::Value {
        schema_object(
            "compress_trajectories",
            "Compress trajectories to fit a token budget using LLM summarization. Requires Python 3.10+.",
            json!({
                "type": "object",
                "required": ["input"],
                "properties": {
                    "input": {"type": "string", "description": "Path to trajectories JSONL"},
                    "target_tokens": {"type": "integer", "description": "Max tokens per trajectory (default: 15250)"},
                    "model": {"type": "string", "description": "Summarization model"}
                }
            }),
        )
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let input_path = input.get("input").and_then(|v| v.as_str()).unwrap_or("");
        let target_tokens = input.get("target_tokens").and_then(|v| v.as_u64()).unwrap_or(15250);
        let model = input.get("model").and_then(|v| v.as_str());

        let output_path = std::path::Path::new(input_path).with_extension("compressed.jsonl");

        let mut cmd = tokio::process::Command::new("python3");
        cmd.arg("training/compress.py")
            .arg(input_path)
            .arg("--target-tokens")
            .arg(target_tokens.to_string())
            .arg("--output")
            .arg(&output_path);

        if let Some(m) = model {
            cmd.arg("--model").arg(m);
        }

        match cmd.status().await {
            Ok(status) if status.success() => ToolResult {
                content: json!({
                    "output": output_path.to_string_lossy(),
                    "target_tokens": target_tokens,
                }).to_string(),
                is_error: false,
            },
            Ok(status) => ToolResult {
                content: json!({"error": format!("Compression failed with exit code: {status}")}).to_string(),
                is_error: true,
            },
            Err(e) => ToolResult {
                content: json!({"error": format!("Failed to run compression: {e}. Is Python 3.10+ installed?")}).to_string(),
                is_error: true,
            },
        }
    }
}

pub struct TrainPipelineTool;

#[async_trait]
impl Tool for TrainPipelineTool {
    fn name(&self) -> &str {
        "train_pipeline"
    }

    fn definition(&self) -> serde_json::Value {
        schema_object(
            "train_pipeline",
            "Run full training pipeline: generate trajectories, export, and optionally compress.",
            json!({
                "type": "object",
                "required": ["dataset"],
                "properties": {
                    "dataset": {"type": "string", "description": "Path to JSONL dataset"},
                    "distribution": {"type": "string", "description": "Toolset distribution (default: 'default')"},
                    "format": {"type": "string", "enum": ["openai", "sharegpt"], "description": "Export format"},
                    "parser": {"type": "string", "description": "Parser for ShareGPT (default: hermes)"},
                    "compress": {"type": "boolean", "description": "Run compression (default: false)"},
                    "target_tokens": {"type": "integer", "description": "Compression target (default: 15250)"},
                    "workers": {"type": "integer", "description": "Parallel workers (default: 4)"},
                    "model": {"type": "string", "description": "Override model"}
                }
            }),
        )
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let dataset = input.get("dataset").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let config = crate::train_pipeline::PipelineConfig {
            dataset: std::path::PathBuf::from(&dataset),
            workers: input.get("workers").and_then(|v| v.as_u64()).unwrap_or(4) as usize,
            batch_size: 10,
            distribution: input.get("distribution").and_then(|v| v.as_str()).unwrap_or("default").to_string(),
            max_iterations: 10,
            model: input.get("model").and_then(|v| v.as_str()).map(String::from),
            format: input.get("format").and_then(|v| v.as_str()).unwrap_or("openai").to_string(),
            parser: input.get("parser").and_then(|v| v.as_str()).unwrap_or("hermes").to_string(),
            compress: input.get("compress").and_then(|v| v.as_bool()).unwrap_or(false),
            target_tokens: input.get("target_tokens").and_then(|v| v.as_u64()).unwrap_or(15250) as usize,
            run_name: format!("run-{}", chrono::Utc::now().format("%Y%m%d-%H%M")),
            output_dir: std::path::PathBuf::from("training-runs")
                .join(format!("run-{}", chrono::Utc::now().format("%Y%m%d-%H%M"))),
            resume: false,
            config_path: None,
        };

        match crate::train_pipeline::run_pipeline(&config) {
            Ok(result) => ToolResult {
                content: json!({
                    "run_name": config.run_name,
                    "trajectories": result.trajectories.to_string_lossy(),
                    "exported": result.exported.map(|p| p.to_string_lossy().to_string()),
                    "compressed": result.compressed.map(|p| p.to_string_lossy().to_string()),
                }).to_string(),
                is_error: false,
            },
            Err(e) => ToolResult {
                content: json!({"error": e}).to_string(),
                is_error: true,
            },
        }
    }
}
```

- [ ] **Step 2: Register tools in `src/tools/mod.rs`**

Add module declaration:
```rust
pub mod training;
```

Add to `ToolRegistry::new()` after the existing tool registrations:
```rust
// Training tools (always available)
Box::new(training::BatchGenerateTool),
Box::new(training::ExportTrajectoriesTool),
Box::new(training::CompressTrajectoriesTool),
Box::new(training::TrainPipelineTool),
```

- [ ] **Step 3: Verify build**

Run: `cargo build`
Expected: Clean build.

- [ ] **Step 4: Commit**

```bash
git add src/tools/training.rs src/tools/mod.rs
git commit -m "feat: add 4 training agent tools (batch_generate, export, compress, train_pipeline)"
```

---

### Task 10: Trajectory Compression Python Script

**Files:**
- Create: `training/compress.py`
- Create: `training/requirements.txt`

- [ ] **Step 1: Write compression script**

Create `training/compress.py`:
```python
#!/usr/bin/env python3
"""Trajectory compression: fit training data into token budgets.

Usage: python training/compress.py <input.jsonl> [options]
"""
import argparse
import asyncio
import json
import sys
from dataclasses import dataclass, field
from pathlib import Path


@dataclass
class CompressionConfig:
    target_max_tokens: int = 15250
    summary_target_tokens: int = 750
    protect_first_system: bool = True
    protect_first_human: bool = True
    protect_first_gpt: bool = True
    protect_first_tool: bool = True
    protect_last_n_turns: int = 4
    summarization_model: str = "google/gemini-3-flash-preview"
    base_url: str = "https://openrouter.ai/api/v1"
    temperature: float = 0.3
    max_retries: int = 3
    skip_under_target: bool = True
    tokenizer_name: str = "moonshotai/Kimi-K2-Thinking"
    num_workers: int = 4
    max_concurrent_requests: int = 50
    per_trajectory_timeout: int = 300


@dataclass
class Metrics:
    total: int = 0
    compressed: int = 0
    skipped: int = 0
    over_limit: int = 0
    failed: int = 0
    tokens_before: int = 0
    tokens_after: int = 0
    api_calls: int = 0
    errors: int = 0


def count_tokens(text: str, tokenizer) -> int:
    return len(tokenizer.encode(text))


def count_trajectory_tokens(trajectory: list, tokenizer) -> list:
    return [count_tokens(turn.get("value", ""), tokenizer) for turn in trajectory]


async def summarize(text: str, config: CompressionConfig) -> str:
    import httpx
    import os

    api_key = os.environ.get("OPENROUTER_API_KEY", "")
    if not api_key:
        raise ValueError("OPENROUTER_API_KEY not set")

    for attempt in range(config.max_retries):
        try:
            async with httpx.AsyncClient(timeout=60) as client:
                resp = await client.post(
                    f"{config.base_url}/chat/completions",
                    headers={"Authorization": f"Bearer {api_key}"},
                    json={
                        "model": config.summarization_model,
                        "temperature": config.temperature,
                        "messages": [
                            {"role": "system", "content": "Summarize the following conversation turns concisely. Focus on: goal, progress, key decisions, files modified, and next steps."},
                            {"role": "user", "content": text},
                        ],
                    },
                )
                resp.raise_for_status()
                return resp.json()["choices"][0]["message"]["content"]
        except Exception as e:
            if attempt == config.max_retries - 1:
                raise
            await asyncio.sleep(2 ** attempt)
    return ""


async def compress_trajectory(trajectory: list, config: CompressionConfig, tokenizer, metrics: Metrics) -> list:
    turn_tokens = count_trajectory_tokens(trajectory, tokenizer)
    total = sum(turn_tokens)
    metrics.tokens_before += total

    if config.skip_under_target and total <= config.target_max_tokens:
        metrics.skipped += 1
        metrics.tokens_after += total
        return trajectory

    # Find protected regions
    head_end = 0
    roles_seen = set()
    for i, turn in enumerate(trajectory):
        role = turn.get("from", "")
        if role == "system" and config.protect_first_system and "system" not in roles_seen:
            roles_seen.add("system")
            head_end = i + 1
        elif role == "human" and config.protect_first_human and "human" not in roles_seen:
            roles_seen.add("human")
            head_end = i + 1
        elif role == "gpt" and config.protect_first_gpt and "gpt" not in roles_seen:
            roles_seen.add("gpt")
            head_end = i + 1
        elif role == "tool" and config.protect_first_tool and "tool" not in roles_seen:
            roles_seen.add("tool")
            head_end = i + 1
        else:
            break

    tail_start = max(head_end, len(trajectory) - config.protect_last_n_turns)

    tokens_to_save = total - config.target_max_tokens
    target_to_compress = tokens_to_save + config.summary_target_tokens

    # Accumulate middle turns
    accumulated = 0
    compress_end = head_end
    for i in range(head_end, tail_start):
        accumulated += turn_tokens[i]
        compress_end = i + 1
        if accumulated >= target_to_compress:
            break

    if compress_end <= head_end:
        metrics.over_limit += 1
        metrics.tokens_after += total
        return trajectory

    # Build text for summarization
    summary_input = ""
    for i in range(head_end, compress_end):
        value = trajectory[i].get("value", "")[:3000]
        summary_input += f"[{trajectory[i].get('from', '?')}]: {value}\n\n"

    try:
        summary = await summarize(summary_input, config)
        metrics.api_calls += 1
    except Exception:
        metrics.errors += 1
        metrics.over_limit += 1
        metrics.tokens_after += total
        return trajectory

    # Build compressed trajectory
    result = list(trajectory[:head_end])

    # Add summary notice to system message
    if result and result[0].get("from") == "system":
        result[0] = dict(result[0])
        result[0]["value"] += "\n\nSome of your previous tool responses may be summarized to preserve context."

    result.append({"from": "human", "value": f"[CONTEXT SUMMARY]: {summary}"})
    result.extend(trajectory[tail_start:])

    compressed_tokens = sum(count_tokens(t.get("value", ""), tokenizer) for t in result)
    metrics.tokens_after += compressed_tokens
    metrics.compressed += 1

    if compressed_tokens > config.target_max_tokens:
        metrics.over_limit += 1

    return result


async def main():
    parser = argparse.ArgumentParser(description="Compress trajectories")
    parser.add_argument("input", help="Input JSONL file")
    parser.add_argument("--output", help="Output file")
    parser.add_argument("--target-tokens", type=int, default=15250)
    parser.add_argument("--summary-tokens", type=int, default=750)
    parser.add_argument("--protect-last", type=int, default=4)
    parser.add_argument("--model", default="google/gemini-3-flash-preview")
    parser.add_argument("--tokenizer", default="moonshotai/Kimi-K2-Thinking")
    parser.add_argument("--workers", type=int, default=4)
    args = parser.parse_args()

    config = CompressionConfig(
        target_max_tokens=args.target_tokens,
        summary_target_tokens=args.summary_tokens,
        protect_last_n_turns=args.protect_last,
        summarization_model=args.model,
        tokenizer_name=args.tokenizer,
        num_workers=args.workers,
    )

    output = args.output or args.input.replace(".jsonl", "_compressed.jsonl")

    # Load tokenizer
    try:
        from transformers import AutoTokenizer
        tokenizer = AutoTokenizer.from_pretrained(config.tokenizer_name, trust_remote_code=True)
    except ImportError:
        print("Error: pip install transformers", file=sys.stderr)
        sys.exit(1)

    # Process
    metrics = Metrics()
    entries = []
    with open(args.input) as f:
        for line in f:
            if line.strip():
                entries.append(json.loads(line))

    metrics.total = len(entries)
    sem = asyncio.Semaphore(config.max_concurrent_requests)

    async def process_one(entry):
        async with sem:
            conversations = entry.get("conversations", entry.get("messages", []))
            try:
                compressed = await asyncio.wait_for(
                    compress_trajectory(conversations, config, tokenizer, metrics),
                    timeout=config.per_trajectory_timeout,
                )
                result = dict(entry)
                if "conversations" in entry:
                    result["conversations"] = compressed
                else:
                    result["messages"] = compressed
                return result
            except asyncio.TimeoutError:
                metrics.failed += 1
                return entry
            except Exception:
                metrics.failed += 1
                return entry

    results = await asyncio.gather(*[process_one(e) for e in entries])

    with open(output, "w") as f:
        for r in results:
            f.write(json.dumps(r, ensure_ascii=False) + "\n")

    # Write metrics
    metrics_path = Path(output).with_suffix(".metrics.json")
    with open(metrics_path, "w") as f:
        json.dump({
            "total": metrics.total,
            "compressed": metrics.compressed,
            "skipped": metrics.skipped,
            "over_limit": metrics.over_limit,
            "failed": metrics.failed,
            "tokens_before": metrics.tokens_before,
            "tokens_after": metrics.tokens_after,
            "tokens_saved": metrics.tokens_before - metrics.tokens_after,
            "api_calls": metrics.api_calls,
            "errors": metrics.errors,
        }, f, indent=2)

    print(f"Done. {metrics.compressed} compressed, {metrics.skipped} skipped, {metrics.failed} failed.")
    print(f"Tokens: {metrics.tokens_before} → {metrics.tokens_after} (saved {metrics.tokens_before - metrics.tokens_after})")


if __name__ == "__main__":
    asyncio.run(main())
```

- [ ] **Step 2: Write requirements.txt**

Create `training/requirements.txt`:
```
transformers>=4.40.0
httpx>=0.27.0
```

- [ ] **Step 3: Commit**

```bash
git add training/compress.py training/requirements.txt
git commit -m "feat: add trajectory compression Python script with token-budget fitting"
```

---

### Task 11: Config Fields & Final Integration

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Add training config fields**

Add to the `Config` struct in `src/config.rs`:
```rust
// Training
#[serde(default = "default_training_workers")]
pub training_default_workers: usize,
#[serde(default = "default_training_batch_size")]
pub training_default_batch_size: usize,
#[serde(default = "default_training_max_iterations")]
pub training_default_max_iterations: usize,
#[serde(default = "default_training_distribution")]
pub training_default_distribution: String,
#[serde(default = "default_training_output_dir")]
pub training_output_dir: String,
#[serde(default = "default_training_compress_target_tokens")]
pub training_compress_target_tokens: usize,
#[serde(default = "default_training_compress_model")]
pub training_compress_model: String,
#[serde(default = "default_training_compress_tokenizer")]
pub training_compress_tokenizer: String,
#[serde(default = "default_training_environments_dir")]
pub training_environments_dir: String,
#[serde(default = "default_training_distributions_file")]
pub training_distributions_file: String,
```

Add default functions:
```rust
fn default_training_workers() -> usize { 4 }
fn default_training_batch_size() -> usize { 10 }
fn default_training_max_iterations() -> usize { 10 }
fn default_training_distribution() -> String { "default".into() }
fn default_training_output_dir() -> String { "./training-runs".into() }
fn default_training_compress_target_tokens() -> usize { 15250 }
fn default_training_compress_model() -> String { "google/gemini-3-flash-preview".into() }
fn default_training_compress_tokenizer() -> String { "moonshotai/Kimi-K2-Thinking".into() }
fn default_training_environments_dir() -> String { "./training/environments".into() }
fn default_training_distributions_file() -> String { "./training/distributions.yaml".into() }
```

- [ ] **Step 2: Verify build**

Run: `cargo build`
Expected: Clean build with no warnings.

- [ ] **Step 3: Commit**

```bash
git add src/config.rs
git commit -m "feat: add training config fields with defaults"
```

---

### Task 12: Final Build Verification

- [ ] **Step 1: Run all tests**

Run: `cargo test -- --nocapture`
Expected: All tests pass.

- [ ] **Step 2: Verify CLI help**

Run: `cargo run -- --help`
Expected: Shows `batch`, `export`, `train` subcommands.

Run: `cargo run -- export --help`
Expected: Shows format, parser, filter options.

- [ ] **Step 3: Verify parser registry**

Run: `cargo test parsers -- --nocapture`
Expected: All 11 parsers tested and passing.

- [ ] **Step 4: Final commit**

```bash
git add -A
git commit -m "chore: final build verification for MLOps training pipeline"
```
