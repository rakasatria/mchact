# mchact MLOps & Training Platform — Design Spec

**Status:** Draft
**Date:** 2026-03-27
**Inspired by:** hermes-agent (Nous Research) — production-ready RL/MLOps pipeline

---

## Overview

Six subsystems that transform mchact from an agent runtime into a training data platform:

1. **Batch Runner** — Multi-process parallel trajectory generation
2. **Export & Parsers** — Format conversion with 11 model-specific tool-call parsers
3. **Toolset Distributions** — Probability-based tool sampling for training diversity
4. **Trajectory Compression** — Token-budget fitting via Python worker
5. **RL Training** — Atropos/Tinker supervision with bundled environments
6. **Agent Learning** — Skill auto-creation + browser vision analysis

**Architecture principle:** Rust handles orchestration, checkpointing, format conversion, CLI, and agent tools. Python handles only what requires Python libraries (HuggingFace tokenizer, Atropos/Tinker/SGLang).

**Three interaction modes:**
- **CLI commands** — `mchact batch`, `mchact export`, `mchact compress`, `mchact rl` for scripts, CI/CD, power users
- **Pipeline shortcut** — `mchact train` chains batch → export → compress in one command
- **Agent tools** — `batch_generate`, `export_trajectories`, `compress_trajectories`, `rl_*` tools for conversational/agentic use within chat

---

## 0. Interaction Modes

### Mode 1: Individual CLI Commands

Building blocks for power users and scripts:

```sh
mchact batch prompts.jsonl --workers 4 --distribution research
mchact export training-runs/run-001/trajectories.jsonl --format sharegpt --parser hermes
mchact compress training-runs/run-001/trajectories.jsonl --target-tokens 15250
mchact rl start
```

### Mode 2: Pipeline Shortcut (`mchact train`)

Single command for the common workflow:

```
mchact train <dataset.jsonl> [options]
  --workers <N>              Parallel workers (default: 4)
  --distribution <name>      Toolset distribution (default: "default")
  --max-iterations <N>       Tool iterations per prompt (default: 10)
  --model <model>            Override model
  --format <openai|sharegpt> Export format (default: openai)
  --parser <name>            Tool-call parser for ShareGPT (default: hermes)
  --compress                 Enable compression after export
  --target-tokens <N>        Compression target (default: 15250)
  --run-name <name>          Output directory name
  --resume                   Resume from checkpoint
  --output <dir>             Output directory
```

Pipeline flow:
```
mchact train prompts.jsonl --format sharegpt --parser qwen3 --compress

Step 1/3: Generating trajectories...
  [████████████████████████] 1000/1000 prompts (4 workers)
  → training-runs/run-001/trajectories.jsonl (1000 entries)

Step 2/3: Exporting to ShareGPT (parser: qwen3)...
  → training-runs/run-001/trajectories_sharegpt.jsonl

Step 3/3: Compressing to 15250 tokens...
  → training-runs/run-001/trajectories_sharegpt_compressed.jsonl
  Compression: 800/1000 compressed, 150 skipped (under target), 50 kept as-is

Done. Output: training-runs/run-001/
  trajectories.jsonl              (OpenAI format, canonical)
  trajectories_sharegpt.jsonl     (ShareGPT + qwen3 parser)
  trajectories_sharegpt_compressed.jsonl
  statistics.json
  compression_metrics.json
```

If `--format openai` (default) and no `--compress`, the pipeline is just batch generation — identical to `mchact batch`.

### Mode 3: Agent Tools (Conversational/Agentic)

The agent has 8 training tools available in the main tool registry:

#### `batch_generate` — Generate trajectories from a dataset

```json
{
  "name": "batch_generate",
  "description": "Generate tool-calling trajectories from a prompt dataset. Spawns parallel workers that process each prompt through the agent engine.",
  "parameters": {
    "type": "object",
    "required": ["dataset"],
    "properties": {
      "dataset": {
        "type": "string",
        "description": "Path to JSONL file with 'prompt' field per line"
      },
      "workers": {
        "type": "integer",
        "description": "Parallel worker processes (default: 4)"
      },
      "distribution": {
        "type": "string",
        "description": "Toolset distribution name (default: 'default')"
      },
      "max_iterations": {
        "type": "integer",
        "description": "Max tool-call iterations per prompt (default: 10)"
      },
      "model": {
        "type": "string",
        "description": "Override model for generation"
      },
      "run_name": {
        "type": "string",
        "description": "Output directory name (auto-generated if omitted)"
      }
    }
  }
}
```

Returns: `{"run_name": "run-001", "output_dir": "training-runs/run-001", "total_prompts": 1000, "completed": 950, "failed": 50, "duration_seconds": 3600}`

#### `export_trajectories` — Convert format and apply parser

```json
{
  "name": "export_trajectories",
  "description": "Convert trajectories to a different format (OpenAI or ShareGPT) with model-specific tool-call parsers.",
  "parameters": {
    "type": "object",
    "required": ["input"],
    "properties": {
      "input": {
        "type": "string",
        "description": "Path to trajectories.jsonl"
      },
      "format": {
        "type": "string",
        "enum": ["openai", "sharegpt"],
        "description": "Output format (default: openai)"
      },
      "parser": {
        "type": "string",
        "enum": ["hermes", "longcat", "qwen", "llama3_json", "llama4_json", "mistral", "deepseek_v3", "deepseek_v3_1", "glm45", "glm47", "kimi_k2", "qwen3_coder"],
        "description": "Tool-call parser for ShareGPT format (default: hermes)"
      },
      "filter_completed": {
        "type": "boolean",
        "description": "Only include completed trajectories (default: false)"
      }
    }
  }
}
```

Returns: `{"output": "training-runs/run-001/trajectories_sharegpt.jsonl", "entries": 950, "filtered": 50}`

#### `compress_trajectories` — Fit trajectories to token budget

```json
{
  "name": "compress_trajectories",
  "description": "Compress trajectories to fit a token budget using LLM summarization. Requires Python 3.10+.",
  "parameters": {
    "type": "object",
    "required": ["input"],
    "properties": {
      "input": {
        "type": "string",
        "description": "Path to trajectories JSONL file"
      },
      "target_tokens": {
        "type": "integer",
        "description": "Max tokens per trajectory (default: 15250)"
      },
      "summary_tokens": {
        "type": "integer",
        "description": "Target summary size (default: 750)"
      },
      "model": {
        "type": "string",
        "description": "Summarization model (default: google/gemini-3-flash-preview)"
      }
    }
  }
}
```

Returns: `{"output": "..._compressed.jsonl", "compressed": 800, "skipped": 150, "failed": 50, "tokens_saved": 8000000}`

#### `train_pipeline` — Run full batch → export → compress chain

```json
{
  "name": "train_pipeline",
  "description": "Run the full training data pipeline: generate trajectories, export to target format, and optionally compress. Combines batch_generate + export_trajectories + compress_trajectories.",
  "parameters": {
    "type": "object",
    "required": ["dataset"],
    "properties": {
      "dataset": {
        "type": "string",
        "description": "Path to JSONL dataset"
      },
      "distribution": {
        "type": "string",
        "description": "Toolset distribution (default: 'default')"
      },
      "format": {
        "type": "string",
        "enum": ["openai", "sharegpt"],
        "description": "Export format (default: openai)"
      },
      "parser": {
        "type": "string",
        "description": "Tool-call parser for ShareGPT (default: hermes)"
      },
      "compress": {
        "type": "boolean",
        "description": "Run compression after export (default: false)"
      },
      "target_tokens": {
        "type": "integer",
        "description": "Compression target tokens (default: 15250)"
      },
      "workers": {
        "type": "integer",
        "description": "Parallel workers (default: 4)"
      },
      "model": {
        "type": "string",
        "description": "Override model"
      }
    }
  }
}
```

Returns: `{"run_name": "...", "trajectories": "...", "exported": "...", "compressed": "...", "statistics": {...}}`

#### `rl_start_training` — Start an RL training run

```json
{
  "name": "rl_start_training",
  "description": "Start an RL training run with the selected environment. Spawns 3 processes (API server, trainer, environment). Requires Python with atroposlib, tinker, wandb.",
  "parameters": {
    "type": "object",
    "required": ["environment"],
    "properties": {
      "environment": {
        "type": "string",
        "description": "Environment name (use rl_list_environments to see available)"
      },
      "config_overrides": {
        "type": "object",
        "description": "Override configurable fields (not locked fields)"
      }
    }
  }
}
```

Returns: `{"run_id": "a1b2c3d4", "environment": "web_research", "status": "starting", "wandb_run_name": "web_research-20260327-1430"}`

#### `rl_check_status` — Check training status and metrics

```json
{
  "name": "rl_check_status",
  "description": "Check the status of an RL training run. Rate-limited to once per 30 minutes. Returns WandB metrics if available.",
  "parameters": {
    "type": "object",
    "properties": {
      "run_id": {
        "type": "string",
        "description": "Run ID (omit to check latest run)"
      }
    }
  }
}
```

Returns: `{"run_id": "...", "status": "running", "running_time_minutes": 45.2, "wandb_metrics": {"step": 150, "reward_mean": 0.72, "percent_correct": 68.5}}`

#### `rl_stop_training` — Stop a training run

```json
{
  "name": "rl_stop_training",
  "description": "Stop an RL training run. Gracefully terminates all 3 processes.",
  "parameters": {
    "type": "object",
    "properties": {
      "run_id": {
        "type": "string",
        "description": "Run ID (omit to stop latest run)"
      }
    }
  }
}
```

Returns: `{"run_id": "...", "status": "stopped"}`

#### `rl_list_environments` — List available RL environments

```json
{
  "name": "rl_list_environments",
  "description": "List available RL training environments.",
  "parameters": {
    "type": "object",
    "properties": {}
  }
}
```

Returns: `{"environments": [{"name": "web_research", "description": "...", "file": "..."}], "count": 3}`

### Agent-Driven Workflow Examples

**Example 1: "Generate training data from my prompts for Qwen fine-tuning"**

Agent chains:
1. `batch_generate(dataset="prompts.jsonl", distribution="research")`
2. `export_trajectories(input="training-runs/run-001/trajectories.jsonl", format="sharegpt", parser="qwen3")`
3. Reports results to user

**Example 2: "Run the full pipeline with compression"**

Agent calls:
1. `train_pipeline(dataset="prompts.jsonl", format="sharegpt", parser="hermes", compress=true)`
2. Reports results to user

**Example 3: "Train on web research and tell me when accuracy hits 70%"**

Agent chains:
1. `rl_start_training(environment="web_research")`
2. Waits, periodically calls `rl_check_status()`
3. Reports: "Step 150, accuracy 68.5%. Not yet at 70%."
4. Checks again after 30 minutes
5. Reports: "Step 250, accuracy 71.2%. Hit your 70% target. Want me to stop training?"
6. On user confirmation: `rl_stop_training()`

**Example 4: "What training runs are active?"**

Agent calls `rl_check_status()` for each active run, summarizes.

### Tool Registration

All 8 training tools registered in `src/tools/mod.rs` conditionally:
- `batch_generate`, `export_trajectories`, `compress_trajectories`, `train_pipeline` — always available
- `rl_start_training`, `rl_check_status`, `rl_stop_training`, `rl_list_environments` — available when `training.environments_dir` is configured

Training tools are available in the **main agent registry only** (not sub-agents — training is a top-level operation).

---

## 1. Batch Runner (`mchact batch`)

### CLI Interface

```
mchact batch <dataset.jsonl> [options]
  --workers <N>              Parallel worker processes (default: 4)
  --batch-size <N>           Prompts per batch (default: 10)
  --distribution <name>      Toolset distribution (default: "default")
  --max-iterations <N>       Tool-call iterations per prompt (default: 10)
  --model <model>            Override model
  --run-name <name>          Output directory name (default: auto-generated)
  --resume                   Resume from last checkpoint
  --max-samples <N>          Truncate dataset to N prompts
  --output <dir>             Output directory (default: ./training-runs/<run-name>/)
```

### Dataset Format (Input)

```jsonl
{"prompt": "Research the latest developments in quantum computing"}
{"prompt": "Write a Python script that parses CSV files", "toolsets": ["terminal", "file"]}
{"prompt": "Find and fix the bug in this repo", "image": "docker:python:3.11"}
```

Fields:
- `prompt` (required): The task description
- `toolsets` (optional): Override distribution sampling for this prompt
- `image` (optional): Docker image override for sandboxed execution

### Worker Process Model

```
mchact batch (coordinator)
│
├── reads dataset.jsonl
├── splits into batches of --batch-size
├── for each batch:
│   ├── spawns: mchact worker --batch-file batch_N.jsonl --config ...
│   ├── worker loads prompts
│   ├── for each prompt:
│   │   ├── sample toolsets from distribution (or use per-prompt override)
│   │   ├── create tool registry with sampled tools only
│   │   ├── run process_with_agent(prompt, tools, max_iterations)
│   │   │   - skip_context_files=true, skip_memory=true (no pollution)
│   │   │   - no SOUL.md injection (training data should be generic)
│   │   ├── extract tool_stats and reasoning_stats from messages
│   │   ├── convert to trajectory format (OpenAI messages)
│   │   └── write entry to batch_N.jsonl
│   └── worker exits with code 0 (success) or 1 (fatal error)
├── reads all batch_N.jsonl
├── normalizes tool stats (all tools present in every entry)
├── validates (filter entries with hallucinated tool names)
├── writes: trajectories.jsonl + statistics.json
└── writes: checkpoint.json (for --resume)
```

### Checkpoint Format

```json
{
  "run_name": "run-20260327-1430",
  "completed_prompts": [0, 1, 2, 5, 6],
  "batch_stats": {
    "0": {"processed": 10, "skipped": 0, "failed": 1}
  },
  "last_updated": "2026-03-27T14:35:00Z"
}
```

**Resume logic:** Content-based. Scans completed batch files for prompt text, filters dataset to unprocessed prompts only. Robust against partial batch files.

### Output Trajectory Entry (OpenAI Format)

```json
{
  "prompt_index": 0,
  "messages": [
    {"role": "system", "content": "..."},
    {"role": "user", "content": "..."},
    {
      "role": "assistant",
      "content": "Let me search for that.",
      "tool_calls": [
        {
          "id": "call_a1b2c3d4",
          "type": "function",
          "function": {
            "name": "web_search",
            "arguments": "{\"query\": \"quantum computing 2026\"}"
          }
        }
      ]
    },
    {
      "role": "tool",
      "tool_call_id": "call_a1b2c3d4",
      "content": "{\"results\": [...]}"
    }
  ],
  "metadata": {
    "batch_num": 0,
    "timestamp": "2026-03-27T14:30:00Z",
    "model": "claude-sonnet-4-5-20250929"
  },
  "completed": true,
  "partial": false,
  "api_calls": 5,
  "toolsets_used": ["web", "terminal"],
  "tool_stats": {
    "web_search": {"count": 2, "success": 2, "failure": 0},
    "bash": {"count": 3, "success": 3, "failure": 0},
    "read_file": {"count": 0, "success": 0, "failure": 0}
  },
  "tool_error_counts": {
    "web_search": 0,
    "bash": 0,
    "read_file": 0
  }
}
```

### Tool Stats Extraction

For each tool response message, determine success/failure:

1. Parse content as JSON
2. If JSON dict with non-null `"error"` field → **failure**
3. If JSON dict with `"content"` sub-dict containing non-null `"error"` → **failure** (terminal tool pattern)
4. If JSON dict with `"success": false` → **failure**
5. If empty content → **failure**
6. If content starts with `"Error:"` (case-insensitive) → **failure**
7. Otherwise → **success**

Non-zero exit codes from bash are NOT failures — the model can self-correct.

### Tool Stats Normalization

Every trajectory entry must include ALL registered tools in `tool_stats`, even unused ones:

```rust
fn normalize_tool_stats(
    raw: &HashMap<String, ToolStat>,
    all_tools: &HashSet<String>,
) -> HashMap<String, ToolStat> {
    let mut normalized = HashMap::new();
    for tool in all_tools {
        normalized.insert(
            tool.clone(),
            raw.get(tool).cloned().unwrap_or(ToolStat::zero()),
        );
    }
    // Include any unexpected tools too (new tools added at runtime)
    for (tool, stat) in raw {
        normalized.entry(tool.clone()).or_insert_with(|| stat.clone());
    }
    normalized
}
```

This prevents schema mismatch errors in HuggingFace datasets.

### Validation

Filter corrupted entries before writing final `trajectories.jsonl`:
- Parse each JSONL line as JSON (skip invalid JSON)
- Check `tool_stats` keys against registered tool names
- Any key not in the registry = hallucinated tool name → filter entry
- Log filtered entries with reason

### Reasoning Stats Extraction

```rust
struct ReasoningStats {
    total_assistant_turns: u64,
    turns_with_reasoning: u64,
    turns_without_reasoning: u64,
    has_any_reasoning: bool,
}
```

Detection: check for `<REASONING_SCRATCHPAD>` tags in content OR non-empty `reasoning` field (native thinking tokens).

### Statistics Output (`statistics.json`)

```json
{
  "run_name": "run-20260327-1430",
  "distribution": "research",
  "total_prompts": 1000,
  "total_batches": 100,
  "batch_size": 10,
  "model": "claude-sonnet-4-5-20250929",
  "completed_at": "2026-03-27T18:30:00Z",
  "duration_seconds": 14400.5,
  "tool_statistics": {
    "web_search": {
      "count": 2500,
      "success": 2400,
      "failure": 100,
      "success_rate": 96.0,
      "failure_rate": 4.0
    }
  },
  "reasoning_statistics": {
    "total_assistant_turns": 5000,
    "turns_with_reasoning": 4200,
    "turns_without_reasoning": 800
  }
}
```

Rate calculation: `success_rate = round(success / (success + failure) * 100, 2)`. If zero calls, both rates are 0.0.

---

## 2. Export & Tool-Call Parsers (`mchact export`)

### CLI Interface

```
mchact export <trajectories.jsonl> [options]
  --format <openai|sharegpt>    Output format (default: openai)
  --parser <name>               Tool-call parser for ShareGPT (default: hermes)
  --output <file>               Output file (default: stdout)
  --filter-completed            Only export completed trajectories
  --filter-min-tools <N>        Minimum tool calls to include
```

### OpenAI Format

Passthrough — already the canonical storage format. No conversion needed.

### ShareGPT Format

Converts OpenAI messages into ShareGPT `conversations` array:

```
role: system    → {"from": "system", "value": "..."}
role: user      → {"from": "human", "value": "..."}
role: assistant → {"from": "gpt", "value": "..."}
role: tool      → {"from": "tool", "value": "..."}
```

**Key transformation (matching hermes exactly):**

Every `gpt` turn gets a `<think>` block, even if empty:
```json
{
  "from": "gpt",
  "value": "<think>\n{reasoning_or_empty}\n</think>\n{content}\n<tool_call>\n{json}\n</tool_call>"
}
```

Tool responses wrapped in `<tool_response>` tags:
```json
{
  "from": "tool",
  "value": "<tool_response>\n{\"tool_call_id\":\"...\",\"name\":\"...\",\"content\":...}\n</tool_response>"
}
```

**Reasoning tag conversion:** `<REASONING_SCRATCHPAD>` → `<think>`, native `reasoning` field → wrapped in `<think>` tags.

### ToolCallParser Trait

```rust
trait ToolCallParser: Send + Sync {
    /// Parser registration name(s)
    fn names(&self) -> &[&str];

    /// Format tool calls into model-specific training format
    fn format_tool_calls(
        &self,
        content: Option<&str>,
        tool_calls: &[ToolCall],
    ) -> String;

    /// Format tool response into model-specific training format
    fn format_tool_response(
        &self,
        call_id: &str,
        name: &str,
        content: &str,
    ) -> String;

    /// Parse model-specific format back into structured tool calls (for import)
    fn parse(&self, text: &str) -> (Option<String>, Option<Vec<ToolCall>>);
}
```

### 11 Tool-Call Parsers

All formats documented from hermes-agent production code:

#### 1. Hermes Parser (`hermes`)

**Tool call:**
```xml
<tool_call>
{"name": "func_name", "arguments": {"key": "value"}}
</tool_call>
```

**Tool response:**
```xml
<tool_response>
{"tool_call_id": "call_xxx", "name": "func_name", "content": "..."}
</tool_response>
```

**Parsing:** Regex `<tool_call>\s*(.*?)\s*</tool_call>` with DOTALL. Also matches unclosed `<tool_call>\s*(.*)` at EOF (truncated generation). Supports multiple tool calls per turn. Tool call ID generated as `call_{uuid_hex[:8]}`.

#### 2. Longcat Parser (`longcat`)

Same as Hermes but with `<longcat_tool_call>` / `</longcat_tool_call>` tags.

#### 3. Qwen Parser (`qwen`)

Inherits Hermes directly — identical format and parsing.

#### 4. Llama Parser (`llama3_json`, `llama4_json`)

**Tool call:** Raw JSON object (no XML wrapper):
```json
{"name": "func_name", "arguments": {"key": "value"}}
```

Optional `<|python_tag|>` prefix before JSON. Supports `"parameters"` as alias for `"arguments"`.

**Parsing:** Uses incremental JSON decode — scans for `{` characters, attempts `json.loads` from each position. Content is text before first `<|python_tag|>` or first `{`.

#### 5. Mistral Parser (`mistral`)

**Two format variants (auto-detected):**

Pre-v11:
```
[TOOL_CALLS][{"name": "func", "arguments": {...}}]
```

v11+:
```
[TOOL_CALLS]func_name{"arg": "val"}[TOOL_CALLS]func_name2{"arg": "val"}
```

**Detection:** If content after `[TOOL_CALLS]` starts with `[` or `{` → pre-v11 (JSON), otherwise → v11+ (name + JSON).

**Tool call ID:** 9-character random alphanumeric string (Mistral-specific).

#### 6. DeepSeek V3 Parser (`deepseek_v3`)

**Tool call:**
```
<｜tool▁calls▁begin｜>
<｜tool▁call▁begin｜>type<｜tool▁sep｜>function_name
```json
{"arg": "value"}
```
<｜tool▁call▁end｜>
<｜tool▁calls▁end｜>
```

Uses fullwidth Unicode angle brackets and block element characters. JSON wrapped in markdown code block.

**Regex:** `<｜tool▁call▁begin｜>(?P<type>.*?)<｜tool▁sep｜>(?P<function_name>.*?)\s*\`\`\`json\s*(?P<function_arguments>.*?)\s*\`\`\`\s*<｜tool▁call▁end｜>` with DOTALL. Uses `\s*` instead of literal `\n` for robustness.

#### 7. DeepSeek V3.1 Parser (`deepseek_v3_1`, `deepseek_v31`)

**Tool call:**
```
<｜tool▁call▁begin｜>function_name<｜tool▁sep｜>{"arg": "value"}<｜tool▁call▁end｜>
```

Key differences from V3: No type field, no code block wrapper, reversed order (name before separator, arguments after).

#### 8. GLM 4.5 Parser (`glm45`)

**Tool call:**
```xml
<tool_call>function_name
<arg_key>param1</arg_key><arg_value>value1</arg_value>
<arg_key>param2</arg_key><arg_value>value2</arg_value>
</tool_call>
```

Key-value XML tags instead of JSON. Values deserialized with fallback chain: `json::from_str()` → literal eval → raw string.

#### 9. GLM 4.7 Parser (`glm47`)

Extends GLM 4.5 with more flexible newline handling between `</arg_key>` and `<arg_value>` tags.

#### 10. Kimi K2 Parser (`kimi_k2`)

**Tool call:**
```
<|tool_calls_section_begin|>
<|tool_call_begin|>functions.func_name:0<|tool_call_argument_begin|>{"arg": "val"}<|tool_call_end|>
<|tool_calls_section_end|>
```

Dual start tokens: `<|tool_calls_section_begin|>` and `<|tool_call_section_begin|>` (singular). Function name extracted from dotted ID: `"functions.get_weather:0"` → `"get_weather"`.

#### 11. Qwen 3-Coder Parser (`qwen3_coder`)

**Tool call:**
```xml
<tool_call>
<function=function_name>
<parameter=param1>value1</parameter>
<parameter=param2>value2</parameter>
</function>
</tool_call>
```

Nested XML structure. Per-parameter type conversion: `null` → None, JSON parse, literal eval, raw string fallback. Supports unclosed tags at EOF. Tool call ID: `call_{uuid_hex[:24]}`.

---

## 3. Toolset Distributions

### Built-In Distributions

Loaded from `training/distributions.yaml`. Shipped defaults:

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
  description: "Shell and files"
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

### Sampling Algorithm

```rust
fn sample_toolsets(distribution: &Distribution) -> Vec<String> {
    let mut rng = rand::thread_rng();
    let mut selected: Vec<String> = distribution
        .tools
        .iter()
        .filter(|(_, &prob)| rng.gen_range(0.0..100.0) < prob)
        .map(|(name, _)| name.clone())
        .collect();

    // Guarantee at least one tool
    if selected.is_empty() {
        let (best_name, _) = distribution
            .tools
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap();
        selected.push(best_name.clone());
    }

    selected
}
```

Each tool rolled independently (not mutually exclusive). If none selected, fallback to highest probability.

### Custom Distributions

Users add entries to `training/distributions.yaml`:
```yaml
my_custom:
  description: "My custom mix"
  tools:
    web_search: 85
    bash: 70
    read_file: 70
    browser: 40
```

---

## 4. Trajectory Compression (`mchact compress`)

### CLI Interface

```
mchact compress <trajectories.jsonl> [options]
  --target-tokens <N>        Max tokens per trajectory (default: 15250)
  --summary-tokens <N>       Target summary size (default: 750)
  --protect-last <N>         Keep last N turns verbatim (default: 4)
  --model <model>            Summarization model (default: google/gemini-3-flash-preview)
  --tokenizer <name>         HuggingFace tokenizer (default: moonshotai/Kimi-K2-Thinking)
  --workers <N>              Parallel async workers (default: 4)
  --max-concurrent <N>       Rate limit for API calls (default: 50)
  --timeout <secs>           Per-trajectory timeout (default: 300)
  --output <file>            Output file (default: <input>_compressed.jsonl)
  --skip-under-target        Skip trajectories already under target (default: true)
  --metrics <file>           Metrics output (default: compression_metrics.json)
```

### Implementation

Python script at `training/compress.py`, invoked by mchact as a subprocess.

### Algorithm (matching hermes production)

```
For each trajectory:
1. Count tokens per turn using HuggingFace tokenizer
2. If total <= target_max_tokens → skip (mark as skipped_under_target)
3. Identify protected head:
   - protect_first_system (always)
   - protect_first_human (always)
   - protect_first_gpt (always)
   - protect_first_tool (always)
4. Identify protected tail: last protect_last_n_turns turns
5. Calculate: tokens_to_save = total_tokens - target_max_tokens
6. Calculate: target_to_compress = tokens_to_save + summary_target_tokens
7. Accumulate middle turns (between head and tail) until accumulated >= target_to_compress
8. Truncate very long turn values to 3000 chars for summarization input
9. Call LLM to summarize accumulated turns
   - Prefix summary with "[CONTEXT SUMMARY]:"
10. Build compressed trajectory:
    - Protected head turns
    - Add summary_notice to system message if add_summary_notice=true
    - Insert summary as human message
    - Protected tail turns
11. Calculate metrics (original/compressed tokens, turns removed, ratio)
```

### Metrics

```json
{
  "total_trajectories": 1000,
  "compressed": 800,
  "skipped_under_target": 150,
  "still_over_limit": 20,
  "failed": 30,
  "tokens": {
    "before": 20000000,
    "after": 12000000,
    "saved": 8000000
  },
  "turns": {
    "before": 50000,
    "after": 30000,
    "removed": 20000
  },
  "summarization": {
    "api_calls": 800,
    "errors": 5
  },
  "compression_ratios": [0.65, 0.72, 0.58],
  "duration_seconds": 1200.5
}
```

---

## 5. RL Training (`mchact rl`)

### CLI Interface

```
mchact rl list                         List available environments
mchact rl select <name>                Select environment, show config
mchact rl config                       Show current config (locked + configurable)
mchact rl edit <field> <value>         Edit configurable field
mchact rl start                        Start training run
mchact rl status [run-id]              Check status + WandB metrics
mchact rl stop [run-id]                Stop training run
mchact rl results [run-id]             Fetch final metrics
mchact rl runs                         List all runs
mchact rl test [--steps N]             Test inference without full training
```

### Requirements

Python packages (only needed when using `mchact rl`):
- `atroposlib` (from NousResearch)
- `tinker` (from thinking-machines-lab)
- `wandb` >= 0.15.0

Environment variables:
- `TINKER_API_KEY` (required for training)
- `WANDB_API_KEY` (required for monitoring)
- `OPENROUTER_API_KEY` (required for inference testing)

### 3-Process Supervisor

mchact spawns and monitors three Python processes in sequence:

```
Process 1: run-api (Atropos API server)
  ├── Command: ["run-api"]
  ├── CWD: training/ directory
  ├── Wait: 5 seconds for startup
  └── Health check: poll() returns None (still alive)

Process 2: launch_training.py (Tinker trainer + SGLang inference on :8001)
  ├── Command: ["python", "launch_training.py", "--config", config_path]
  ├── Env: TINKER_API_KEY passed
  ├── Wait: 30 seconds for initialization
  └── Health check: poll() returns None

Process 3: environment.py serve (RL environment)
  ├── Command: ["python", env_file, "serve", "--config", config_path]
  ├── Wait: 90 seconds before starting (after Process 2 ready)
  ├── Wait: 10 seconds for connection
  └── Health check: poll() returns None
```

**Monitoring loop:** Background task checks every 30 seconds. Detects process deaths, sets status to `completed` (exit 0) or `failed` (non-zero exit).

**Shutdown:** Terminates in reverse order (env → trainer → api). 10-second grace period before SIGKILL.

### Run State

```rust
struct RlRunState {
    run_id: String,              // uuid[:8]
    environment: String,         // Environment name
    config: serde_json::Value,   // Current config
    status: RlRunStatus,         // pending|starting|running|stopping|stopped|completed|failed
    error_message: String,
    wandb_project: String,
    wandb_run_name: String,
    start_time: Instant,
    processes: Vec<Child>,       // Supervised child processes
}

enum RlRunStatus {
    Pending,
    Starting,
    Running,
    Stopping,
    Stopped,
    Completed,
    Failed,
}
```

### Config Management

**Locked fields** (infrastructure, not user-configurable):

```yaml
env:
  tokenizer_name: "Qwen/Qwen3-8B"
  rollout_server_url: "http://localhost:8000"
  use_wandb: true
  max_token_length: 8192
  max_num_workers: 2048
  worker_timeout: 3600
  total_steps: 2500
  steps_per_eval: 25
  max_batches_offpolicy: 3
  inference_weight: 1.0
  eval_limit_ratio: 0.1

openai:
  - model_name: "Qwen/Qwen3-8B"
    base_url: "http://localhost:8001/v1"
    api_key: "x"
    weight: 1.0
    num_requests_for_eval: 256
    timeout: 3600
    server_type: "sglang"

tinker:
  lora_rank: 32
  learning_rate: 0.00004
  max_token_trainer_length: 9000
  checkpoint_dir: "./temp/"
  save_checkpoint_interval: 25

slurm: false
testing: false
```

**Configurable fields:** Everything NOT in the locked set. Discovered from environment's Pydantic config class via a manifest file.

### Status Checks

Rate-limited: minimum 30-minute interval between status checks (prevent WandB API abuse).

Returns:
```json
{
  "run_id": "a1b2c3d4",
  "environment": "web_research",
  "status": "running",
  "running_time_minutes": 45.2,
  "wandb_metrics": {
    "step": 150,
    "reward_mean": 0.72,
    "percent_correct": 68.5,
    "eval_percent_correct": 65.2
  }
}
```

### Bundled Starter Environments

Shipped in `training/environments/`:

```
training/
├── environments/
│   ├── web_research.py       Web research tasks
│   ├── terminal_tasks.py     Terminal/file manipulation
│   └── swe.py                Software engineering tasks
├── compress.py               Trajectory compression script
├── distributions.yaml        Toolset distributions
└── requirements.txt          Python dependencies for RL
```

**Environment discovery:** Each `.py` file has a YAML frontmatter comment:
```python
# ---
# name: web_research
# class: WebResearchEnv
# description: Web research and information synthesis tasks
# ---
```

mchact scans for these comments (no AST parsing needed — simpler and language-agnostic).

### Inference Testing

```
mchact rl test --steps 3 --group-size 16
```

Runs environment's `process` mode against OpenRouter (not SGLang). Tests N steps × M completions per step across multiple model scales. Reports accuracy metrics.

---

## 6. Agent Learning

### 6a. Skill Auto-Creation (`create_skill` tool)

**Tool definition:**
```json
{
  "name": "create_skill",
  "description": "Create a reusable SKILL.md from a solved problem",
  "parameters": {
    "type": "object",
    "required": ["skill_name", "description", "instructions"],
    "properties": {
      "skill_name": {
        "type": "string",
        "description": "Short identifier (max 64 chars, kebab-case)"
      },
      "description": {
        "type": "string",
        "description": "One-line description (max 1024 chars)"
      },
      "instructions": {
        "type": "string",
        "description": "Full skill instructions (the body of SKILL.md)"
      },
      "platforms": {
        "type": "array",
        "items": {"type": "string"},
        "description": "Supported platforms: macos, linux, windows"
      },
      "prerequisites": {
        "type": "object",
        "properties": {
          "commands": {"type": "array", "items": {"type": "string"}},
          "env_vars": {"type": "array", "items": {"type": "string"}}
        }
      },
      "tags": {
        "type": "array",
        "items": {"type": "string"}
      }
    }
  }
}
```

**Output:** Writes SKILL.md to `{skills_dir}/{skill_name}/SKILL.md`:
```markdown
---
name: skill_name
description: One-line description
platforms: [macos, linux]
prerequisites:
  commands: [python3, git]
  env_vars: [GITHUB_TOKEN]
tags: [research, automation]
source: auto-created
created_at: 2026-03-27T14:00:00Z
---

{instructions}
```

**Registration:** Tool available in main tool registry (not sub-agent). Only the primary agent should create skills.

### Nudge System

After each conversation completes, check complexity thresholds:

```rust
struct SkillNudgeConfig {
    enabled: bool,                    // default: true
    threshold_tool_calls: u32,        // default: 10
    threshold_turns: u32,             // default: 15
    threshold_duration_secs: u64,     // default: 300
}
```

If ANY threshold exceeded, inject into the **next** conversation's system prompt:
```
[SKILL SUGGESTION] Your previous conversation was complex ({N} tool calls, {M} turns).
If the approach you used would be valuable for future tasks, consider using create_skill
to save it as a reusable skill.
```

The nudge is a one-time injection (cleared after delivery). The agent decides whether to act on it.

**Config fields:**
```yaml
skills:
  auto_nudge_enabled: true
  nudge_threshold_tool_calls: 10
  nudge_threshold_turns: 15
  nudge_threshold_duration_secs: 300
```

### 6b. Browser Vision (`browser_vision` tool)

**Tool definition:**
```json
{
  "name": "browser_vision",
  "description": "Capture and analyze a browser screenshot using a vision model",
  "parameters": {
    "type": "object",
    "required": ["query"],
    "properties": {
      "query": {
        "type": "string",
        "description": "What to analyze in the screenshot"
      },
      "selector": {
        "type": "string",
        "description": "Optional CSS selector to screenshot specific element"
      }
    }
  }
}
```

**Execution flow:**
1. Get active browser session for current chat (from existing BrowserTool's persistent sessions)
2. If no active session → return error "No browser session active. Use browser tool first."
3. Capture screenshot:
   - If `selector` provided: screenshot that element
   - Otherwise: full page screenshot
4. Encode screenshot as base64 PNG
5. Build vision request using existing `vision_fallback` config:
   - Provider: `config.vision_fallback_provider` (default: OpenAI)
   - Model: `config.vision_fallback_model` (default: gpt-4o)
   - Base URL: `config.vision_fallback_base_url`
6. Send to vision API with prompt: `"Analyze this browser screenshot. {query}"`
7. Return vision model's natural language description

**No new dependencies.** Uses existing:
- Browser session management from `BrowserTool`
- Vision provider from `vision_fallback_*` config fields
- LLM request infrastructure from `src/llm.rs`

---

## New Files

| File | Purpose | Language | Lines (est) |
|------|---------|---------|-------------|
| `src/batch.rs` | Batch coordinator (CLI, checkpointing, stats) | Rust | ~600 |
| `src/batch_worker.rs` | Worker process (prompt execution, trajectory output) | Rust | ~400 |
| `src/export.rs` | Format conversion CLI | Rust | ~200 |
| `src/train_pipeline.rs` | Pipeline shortcut (`mchact train`) — chains batch → export → compress | Rust | ~250 |
| `src/parsers/mod.rs` | Parser trait + registry | Rust | ~80 |
| `src/parsers/hermes.rs` | Hermes/Qwen/Longcat parsers | Rust | ~120 |
| `src/parsers/llama.rs` | Llama 3/4 parser | Rust | ~150 |
| `src/parsers/mistral.rs` | Mistral parser (dual format) | Rust | ~180 |
| `src/parsers/deepseek.rs` | DeepSeek V3 + V3.1 parsers | Rust | ~160 |
| `src/parsers/glm.rs` | GLM 4.5 + 4.7 parsers | Rust | ~180 |
| `src/parsers/kimi.rs` | Kimi K2 parser | Rust | ~120 |
| `src/parsers/qwen_coder.rs` | Qwen3 Coder parser | Rust | ~160 |
| `src/rl.rs` | RL CLI + 3-process supervisor | Rust | ~500 |
| `src/distributions.rs` | Distribution loading + sampling | Rust | ~150 |
| `src/tools/create_skill.rs` | Skill auto-creation tool | Rust | ~150 |
| `src/tools/browser_vision.rs` | Browser vision analysis tool | Rust | ~120 |
| `src/tools/training.rs` | Agent tools: batch_generate, export_trajectories, compress_trajectories, train_pipeline | Rust | ~400 |
| `src/tools/rl_training.rs` | Agent tools: rl_start_training, rl_check_status, rl_stop_training, rl_list_environments | Rust | ~350 |
| `training/compress.py` | Trajectory compression | Python | ~400 |
| `training/distributions.yaml` | Distribution definitions | YAML | ~200 |
| `training/requirements.txt` | Python deps for RL | Text | ~10 |
| `training/environments/web_research.py` | Starter environment | Python | ~300 |
| `training/environments/terminal_tasks.py` | Starter environment | Python | ~250 |
| `training/environments/swe.py` | Starter environment | Python | ~300 |

**Total new Rust:** ~4,270 lines
**Total new Python:** ~1,260 lines

## Modified Files

| File | Changes |
|------|---------|
| `src/main.rs` | Add `batch`, `export`, `compress`, `rl`, `train` subcommands + `worker` mode |
| `src/config.rs` | Add `training` and `skills.auto_nudge_*` config fields |
| `src/tools/mod.rs` | Register `create_skill`, `browser_vision`, 4 training tools, 4 RL tools (12 new tools total) |
| `src/agent_engine.rs` | Skill nudge injection after complex conversations |
| `src/lib.rs` | Add `batch`, `batch_worker`, `export`, `train_pipeline`, `rl`, `distributions`, `parsers` modules |
| `Cargo.toml` | Add `rand` dependency (for distribution sampling) |

## Config Fields

```yaml
training:
  # Batch runner
  default_workers: 4
  default_batch_size: 10
  default_max_iterations: 10
  default_distribution: "default"
  output_dir: "./training-runs"

  # Compression (passed to Python worker)
  compress_target_tokens: 15250
  compress_summary_tokens: 750
  compress_protect_last_turns: 4
  compress_model: "google/gemini-3-flash-preview"
  compress_tokenizer: "moonshotai/Kimi-K2-Thinking"

  # RL
  environments_dir: "./training/environments"
  distributions_file: "./training/distributions.yaml"

skills:
  auto_nudge_enabled: true
  nudge_threshold_tool_calls: 10
  nudge_threshold_turns: 15
  nudge_threshold_duration_secs: 300
```

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|-----------|
| Worker process crash | Lost batch progress | Checkpoint per-batch, `--resume` resumes from last checkpoint |
| LLM hallucinated tool names | Corrupted training data | Validate tool_stats keys against registry, filter invalid entries |
| Python not installed | `mchact compress` and `mchact rl` fail | Clear error message: "Python 3.10+ required for compression/RL features" |
| WandB API rate limit | Status check fails | 30-minute minimum interval between checks |
| Large dataset OOM | Coordinator runs out of memory | Stream JSONL line-by-line, never load full dataset |
| Inconsistent tool schemas across parsers | Training data schema mismatch | Normalize all tool_stats to include ALL registered tools |
| RL process dies silently | Training hangs | 30-second health check loop, auto-detect exit codes |
| Compression API failures | Incomplete compression | Retry with exponential backoff (3 attempts), save even if over limit |
