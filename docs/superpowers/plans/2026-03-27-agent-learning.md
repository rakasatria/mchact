# Agent Learning Implementation Plan (Plan C)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add skill auto-creation (create_skill tool + nudge system) and browser vision analysis (browser_vision tool) to mchact.

**Architecture:** Two independent agent tools plus a nudge injection in the agent engine. create_skill writes SKILL.md files to the skills directory. browser_vision captures screenshots via the existing browser session and sends them to the vision fallback provider. The nudge system injects a one-time suggestion into the system prompt after complex conversations.

**Tech Stack:** Rust (async_trait, serde_json, base64, reqwest for vision API)

**Spec:** `docs/superpowers/specs/2026-03-27-mlops-training-design.md` (Section 6)

**Depends on:** Plan A + B complete. Uses existing BrowserTool, vision_fallback config, skills_data_dir.

---

### Task 1: create_skill Tool

**Files:**
- Create: `src/tools/create_skill.rs`
- Modify: `src/tools/mod.rs`

- [ ] **Step 1: Create `src/tools/create_skill.rs`**

```rust
use async_trait::async_trait;
use mchact_core::llm_types::ToolDefinition;
use serde_json::json;
use std::path::{Path, PathBuf};

use super::{schema_object, Tool, ToolResult};

pub struct CreateSkillTool {
    skills_dir: String,
}

impl CreateSkillTool {
    pub fn new(skills_dir: &str) -> Self {
        Self {
            skills_dir: skills_dir.to_string(),
        }
    }
}

fn validate_skill_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("skill_name cannot be empty".into());
    }
    if name.len() > 64 {
        return Err(format!("skill_name too long ({} chars, max 64)", name.len()));
    }
    // Must be kebab-case: lowercase letters, digits, hyphens
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err("skill_name must be kebab-case (lowercase letters, digits, hyphens)".into());
    }
    if name.starts_with('-') || name.ends_with('-') {
        return Err("skill_name cannot start or end with a hyphen".into());
    }
    Ok(())
}

fn build_skill_md(
    name: &str,
    description: &str,
    instructions: &str,
    platforms: &[String],
    prerequisites_commands: &[String],
    prerequisites_env_vars: &[String],
    tags: &[String],
) -> String {
    let mut frontmatter = String::new();
    frontmatter.push_str("---\n");
    frontmatter.push_str(&format!("name: {name}\n"));
    frontmatter.push_str(&format!("description: {description}\n"));

    if !platforms.is_empty() {
        let platforms_str: Vec<_> = platforms.iter().map(|s| s.as_str()).collect();
        frontmatter.push_str(&format!("platforms: [{}]\n", platforms_str.join(", ")));
    }

    if !prerequisites_commands.is_empty() || !prerequisites_env_vars.is_empty() {
        frontmatter.push_str("prerequisites:\n");
        if !prerequisites_commands.is_empty() {
            let cmds: Vec<_> = prerequisites_commands.iter().map(|s| s.as_str()).collect();
            frontmatter.push_str(&format!("  commands: [{}]\n", cmds.join(", ")));
        }
        if !prerequisites_env_vars.is_empty() {
            let vars: Vec<_> = prerequisites_env_vars.iter().map(|s| s.as_str()).collect();
            frontmatter.push_str(&format!("  env_vars: [{}]\n", vars.join(", ")));
        }
    }

    if !tags.is_empty() {
        let tags_str: Vec<_> = tags.iter().map(|s| s.as_str()).collect();
        frontmatter.push_str(&format!("tags: [{}]\n", tags_str.join(", ")));
    }

    frontmatter.push_str("source: auto-created\n");
    frontmatter.push_str(&format!(
        "created_at: {}\n",
        chrono::Utc::now().to_rfc3339()
    ));
    frontmatter.push_str("---\n\n");
    frontmatter.push_str(instructions);

    // Ensure trailing newline
    if !frontmatter.ends_with('\n') {
        frontmatter.push('\n');
    }

    frontmatter
}

#[async_trait]
impl Tool for CreateSkillTool {
    fn name(&self) -> &str {
        "create_skill"
    }

    fn definition(&self) -> ToolDefinition {
        schema_object(
            "create_skill",
            "Create a reusable SKILL.md from a solved problem. The skill is saved to the skills directory and can be activated in future conversations.",
            json!({
                "type": "object",
                "required": ["skill_name", "description", "instructions"],
                "properties": {
                    "skill_name": {
                        "type": "string",
                        "description": "Short kebab-case identifier (max 64 chars, e.g. 'deploy-docker-app')"
                    },
                    "description": {
                        "type": "string",
                        "description": "One-line description (max 1024 chars)"
                    },
                    "instructions": {
                        "type": "string",
                        "description": "Full skill instructions (Markdown body of SKILL.md)"
                    },
                    "platforms": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Supported platforms: macos, linux, windows (omit for all)"
                    },
                    "prerequisites": {
                        "type": "object",
                        "properties": {
                            "commands": {
                                "type": "array",
                                "items": {"type": "string"},
                                "description": "Required CLI commands (e.g. docker, python3)"
                            },
                            "env_vars": {
                                "type": "array",
                                "items": {"type": "string"},
                                "description": "Required environment variables (e.g. GITHUB_TOKEN)"
                            }
                        }
                    },
                    "tags": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Tags for discovery (e.g. deployment, automation)"
                    }
                }
            }),
        )
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let skill_name = input
            .get("skill_name")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let description = input
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let instructions = input
            .get("instructions")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Validate
        if let Err(e) = validate_skill_name(skill_name) {
            return ToolResult {
                content: json!({"error": e}).to_string(),
                is_error: true,
                status_code: None,
                bytes: 0,
                duration_ms: None,
                error_type: None,
                metadata: None,
            };
        }

        if description.len() > 1024 {
            return ToolResult {
                content: json!({"error": format!("description too long ({} chars, max 1024)", description.len())}).to_string(),
                is_error: true,
                status_code: None,
                bytes: 0,
                duration_ms: None,
                error_type: None,
                metadata: None,
            };
        }

        if instructions.is_empty() {
            return ToolResult {
                content: json!({"error": "instructions cannot be empty"}).to_string(),
                is_error: true,
                status_code: None,
                bytes: 0,
                duration_ms: None,
                error_type: None,
                metadata: None,
            };
        }

        let platforms: Vec<String> = input
            .get("platforms")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        let prereq_commands: Vec<String> = input
            .pointer("/prerequisites/commands")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        let prereq_env_vars: Vec<String> = input
            .pointer("/prerequisites/env_vars")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        let tags: Vec<String> = input
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        // Build SKILL.md content
        let content = build_skill_md(
            skill_name,
            description,
            instructions,
            &platforms,
            &prereq_commands,
            &prereq_env_vars,
            &tags,
        );

        // Write to skills_dir/skill_name/SKILL.md
        let skill_dir = PathBuf::from(&self.skills_dir).join(skill_name);
        if let Err(e) = std::fs::create_dir_all(&skill_dir) {
            return ToolResult {
                content: json!({"error": format!("Cannot create skill directory: {e}")}).to_string(),
                is_error: true,
                status_code: None,
                bytes: 0,
                duration_ms: None,
                error_type: None,
                metadata: None,
            };
        }

        let skill_path = skill_dir.join("SKILL.md");
        if skill_path.exists() {
            return ToolResult {
                content: json!({"error": format!("Skill '{skill_name}' already exists at {}", skill_path.display())}).to_string(),
                is_error: true,
                status_code: None,
                bytes: 0,
                duration_ms: None,
                error_type: None,
                metadata: None,
            };
        }

        if let Err(e) = std::fs::write(&skill_path, &content) {
            return ToolResult {
                content: json!({"error": format!("Cannot write SKILL.md: {e}")}).to_string(),
                is_error: true,
                status_code: None,
                bytes: 0,
                duration_ms: None,
                error_type: None,
                metadata: None,
            };
        }

        ToolResult {
            content: json!({
                "skill_name": skill_name,
                "path": skill_path.to_string_lossy(),
                "description": description,
                "message": format!("Skill '{skill_name}' created. Use activate_skill to load it in future conversations.")
            })
            .to_string(),
            is_error: false,
            status_code: None,
            bytes: 0,
            duration_ms: None,
            error_type: None,
            metadata: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_skill_name_valid() {
        assert!(validate_skill_name("deploy-docker-app").is_ok());
        assert!(validate_skill_name("my-skill-123").is_ok());
        assert!(validate_skill_name("a").is_ok());
    }

    #[test]
    fn test_validate_skill_name_invalid() {
        assert!(validate_skill_name("").is_err());
        assert!(validate_skill_name("My_Skill").is_err()); // uppercase
        assert!(validate_skill_name("has space").is_err());
        assert!(validate_skill_name("-leading").is_err());
        assert!(validate_skill_name("trailing-").is_err());
        let long = "a".repeat(65);
        assert!(validate_skill_name(&long).is_err());
    }

    #[test]
    fn test_build_skill_md_minimal() {
        let md = build_skill_md("test", "A test skill", "Do the thing.", &[], &[], &[], &[]);
        assert!(md.contains("---\nname: test\n"));
        assert!(md.contains("description: A test skill\n"));
        assert!(md.contains("source: auto-created\n"));
        assert!(md.contains("Do the thing."));
    }

    #[test]
    fn test_build_skill_md_full() {
        let md = build_skill_md(
            "deploy",
            "Deploy stuff",
            "Instructions here",
            &["linux".into(), "macos".into()],
            &["docker".into()],
            &["GITHUB_TOKEN".into()],
            &["deployment".into(), "ci".into()],
        );
        assert!(md.contains("platforms: [linux, macos]"));
        assert!(md.contains("commands: [docker]"));
        assert!(md.contains("env_vars: [GITHUB_TOKEN]"));
        assert!(md.contains("tags: [deployment, ci]"));
    }

    #[tokio::test]
    async fn test_create_skill_writes_file() {
        let dir = std::env::temp_dir().join(format!("mchact_skill_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let tool = CreateSkillTool::new(&dir.to_string_lossy());
        let result = tool
            .execute(json!({
                "skill_name": "test-skill",
                "description": "A test",
                "instructions": "Step 1: do thing"
            }))
            .await;

        assert!(!result.is_error);
        let skill_path = dir.join("test-skill/SKILL.md");
        assert!(skill_path.exists());
        let content = std::fs::read_to_string(&skill_path).unwrap();
        assert!(content.contains("name: test-skill"));
        assert!(content.contains("Step 1: do thing"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_create_skill_rejects_duplicate() {
        let dir = std::env::temp_dir().join(format!("mchact_skill_dup_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("existing-skill")).unwrap();
        std::fs::write(dir.join("existing-skill/SKILL.md"), "existing").unwrap();

        let tool = CreateSkillTool::new(&dir.to_string_lossy());
        let result = tool
            .execute(json!({
                "skill_name": "existing-skill",
                "description": "A test",
                "instructions": "new content"
            }))
            .await;

        assert!(result.is_error);
        assert!(result.content.contains("already exists"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_create_skill_validates_name() {
        let dir = std::env::temp_dir().join(format!("mchact_skill_val_{}", uuid::Uuid::new_v4()));
        let tool = CreateSkillTool::new(&dir.to_string_lossy());

        let result = tool
            .execute(json!({
                "skill_name": "Bad Name!",
                "description": "test",
                "instructions": "test"
            }))
            .await;
        assert!(result.is_error);
        assert!(result.content.contains("kebab-case"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
```

- [ ] **Step 2: Register in `src/tools/mod.rs`**

Add module declaration:
```rust
pub mod create_skill;
```

Add to `ToolRegistry::new()` after the RL tools:
```rust
Box::new(create_skill::CreateSkillTool::new(&config.skills_data_dir())),
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib create_skill -- --nocapture`
Expected: All 6 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/tools/create_skill.rs src/tools/mod.rs
git commit -m "feat: add create_skill tool for autonomous skill creation"
```

---

### Task 2: Skill Nudge System

**Files:**
- Modify: `src/config.rs`
- Modify: `src/agent_engine.rs`

- [ ] **Step 1: Add nudge config fields to `src/config.rs`**

Add to the Config struct:
```rust
#[serde(default = "default_skill_nudge_enabled")]
pub skill_nudge_enabled: bool,
#[serde(default = "default_skill_nudge_threshold_tool_calls")]
pub skill_nudge_threshold_tool_calls: u32,
#[serde(default = "default_skill_nudge_threshold_turns")]
pub skill_nudge_threshold_turns: u32,
#[serde(default = "default_skill_nudge_threshold_duration_secs")]
pub skill_nudge_threshold_duration_secs: u64,
```

Default functions:
```rust
fn default_skill_nudge_enabled() -> bool { true }
fn default_skill_nudge_threshold_tool_calls() -> u32 { 10 }
fn default_skill_nudge_threshold_turns() -> u32 { 15 }
fn default_skill_nudge_threshold_duration_secs() -> u64 { 300 }
```

Add to `test_defaults()` constructor and any test constructors in `tests/config_validation.rs`.

- [ ] **Step 2: Add nudge injection to `src/agent_engine.rs`**

Find the `build_system_prompt` function. Add a nudge parameter and inject it at the end of the system prompt if present.

The nudge state is stored as a simple file: `{data_dir}/runtime/skill_nudge_pending.txt`. After a conversation exceeds thresholds, the message is written there. On the next conversation start, if the file exists, its content is appended to the system prompt and the file is deleted.

Add two functions:

```rust
/// Check if last conversation exceeded skill nudge thresholds and write pending nudge.
pub(crate) fn maybe_write_skill_nudge(
    config: &Config,
    data_dir: &str,
    tool_call_count: u32,
    turn_count: u32,
    duration_secs: u64,
) {
    if !config.skill_nudge_enabled {
        return;
    }
    let exceeds = tool_call_count >= config.skill_nudge_threshold_tool_calls
        || turn_count >= config.skill_nudge_threshold_turns
        || duration_secs >= config.skill_nudge_threshold_duration_secs;
    if !exceeds {
        return;
    }
    let nudge = format!(
        "\n\n[SKILL SUGGESTION] Your previous conversation was complex ({} tool calls, {} turns, {}s). \
         If the approach you used would be valuable for future tasks, consider using create_skill to save it as a reusable skill.",
        tool_call_count, turn_count, duration_secs
    );
    let nudge_path = std::path::Path::new(data_dir)
        .join("runtime")
        .join("skill_nudge_pending.txt");
    let _ = std::fs::create_dir_all(nudge_path.parent().unwrap_or(std::path::Path::new(".")));
    let _ = std::fs::write(&nudge_path, &nudge);
}

/// Read and consume pending skill nudge (returns None if no nudge pending).
pub(crate) fn consume_skill_nudge(data_dir: &str) -> Option<String> {
    let nudge_path = std::path::Path::new(data_dir)
        .join("runtime")
        .join("skill_nudge_pending.txt");
    let content = std::fs::read_to_string(&nudge_path).ok()?;
    let _ = std::fs::remove_file(&nudge_path);
    if content.trim().is_empty() {
        None
    } else {
        Some(content)
    }
}
```

Then in the system prompt building area (where `build_system_prompt` is called), append any consumed nudge:
```rust
let nudge = consume_skill_nudge(&config.data_dir);
if let Some(nudge_text) = nudge {
    system_prompt.push_str(&nudge_text);
}
```

And after a conversation completes (after `process_with_agent` returns), call `maybe_write_skill_nudge` with the stats from the conversation.

- [ ] **Step 3: Run tests**

Run: `cargo test --lib agent_engine -- --nocapture`
Expected: All existing tests pass (nudge functions are called only at runtime, no new test breakage).

Run: `cargo build`
Expected: Clean build.

- [ ] **Step 4: Commit**

```bash
git add src/config.rs src/agent_engine.rs tests/config_validation.rs
git commit -m "feat: add skill nudge system — suggests create_skill after complex conversations"
```

---

### Task 3: browser_vision Tool

**Files:**
- Create: `src/tools/browser_vision.rs`
- Modify: `src/tools/mod.rs`

- [ ] **Step 1: Create `src/tools/browser_vision.rs`**

```rust
use async_trait::async_trait;
use mchact_core::llm_types::ToolDefinition;
use serde_json::json;
use std::path::PathBuf;

use super::{auth_context_from_input, schema_object, Tool, ToolResult};
use crate::config::Config;

pub struct BrowserVisionTool {
    data_dir: PathBuf,
    vision_provider: String,
    vision_model: String,
    vision_api_key: Option<String>,
    vision_base_url: String,
}

impl BrowserVisionTool {
    pub fn new(config: &Config) -> Self {
        Self {
            data_dir: PathBuf::from(&config.data_dir),
            vision_provider: config.vision_fallback_provider.clone(),
            vision_model: config.vision_fallback_model.clone(),
            vision_api_key: config.vision_fallback_api_key.clone(),
            vision_base_url: config.vision_fallback_base_url.clone(),
        }
    }

    fn browser_profile_dir(&self, chat_id: i64) -> PathBuf {
        self.data_dir.join("groups").join(chat_id.to_string()).join("browser-profile")
    }

    async fn capture_screenshot(
        &self,
        chat_id: i64,
        selector: Option<&str>,
    ) -> Result<Vec<u8>, String> {
        let session_name = format!(
            "mchact-chat-{}",
            if chat_id < 0 {
                format!("neg{}", chat_id.unsigned_abs())
            } else {
                chat_id.to_string()
            }
        );

        // Build browser command for screenshot
        let mut args = vec![
            "--session".to_string(),
            session_name,
            "screenshot".to_string(),
        ];

        let tmp_path = std::env::temp_dir().join(format!(
            "mchact_vision_{}_{}.png",
            chat_id,
            uuid::Uuid::new_v4().to_string().get(..8).unwrap_or("0")
        ));

        args.push(tmp_path.to_string_lossy().to_string());

        if let Some(sel) = selector {
            args.push("--selector".to_string());
            args.push(sel.to_string());
        }

        let program = mchact_tools::command_runner::agent_browser_program();
        let output = tokio::process::Command::new(&program)
            .args(&args)
            .output()
            .await
            .map_err(|e| format!("Failed to run agent-browser: {e}. Is agent-browser installed?"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Screenshot failed: {stderr}"));
        }

        let bytes = tokio::fs::read(&tmp_path)
            .await
            .map_err(|e| format!("Cannot read screenshot: {e}"))?;
        let _ = tokio::fs::remove_file(&tmp_path).await;

        Ok(bytes)
    }

    async fn analyze_with_vision(
        &self,
        image_bytes: &[u8],
        query: &str,
    ) -> Result<String, String> {
        let api_key = self.vision_api_key.as_deref().or_else(|| {
            std::env::var("OPENAI_API_KEY").ok().as_deref().map(|_| ())
                .and(None) // fallback handled below
        });

        let api_key = self
            .vision_api_key
            .clone()
            .or_else(|| std::env::var("OPENAI_API_KEY").ok())
            .ok_or_else(|| {
                "No vision API key configured. Set vision_fallback_api_key in config or OPENAI_API_KEY env var.".to_string()
            })?;

        let base64_image = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            image_bytes,
        );

        let body = json!({
            "model": self.vision_model,
            "messages": [{
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": format!("Analyze this browser screenshot. {query}")
                    },
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": format!("data:image/png;base64,{base64_image}")
                        }
                    }
                ]
            }],
            "max_tokens": 1024
        });

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("{}/chat/completions", self.vision_base_url))
            .header("Authorization", format!("Bearer {api_key}"))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Vision API request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Vision API returned {status}: {text}"));
        }

        let result: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Cannot parse vision response: {e}"))?;

        let content = result
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
            .unwrap_or("No analysis returned.");

        Ok(content.to_string())
    }
}

#[async_trait]
impl Tool for BrowserVisionTool {
    fn name(&self) -> &str {
        "browser_vision"
    }

    fn definition(&self) -> ToolDefinition {
        schema_object(
            "browser_vision",
            "Capture and analyze a browser screenshot using a vision model. Requires an active browser session (use the browser tool first).",
            json!({
                "type": "object",
                "required": ["query"],
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "What to look for or analyze in the screenshot"
                    },
                    "selector": {
                        "type": "string",
                        "description": "Optional CSS selector to screenshot a specific element instead of the full page"
                    }
                }
            }),
        )
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let query = input
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("Describe what you see.");
        let selector = input.get("selector").and_then(|v| v.as_str());

        let auth = auth_context_from_input(&input);
        let chat_id = auth.caller_chat_id;

        // Capture screenshot
        let image_bytes = match self.capture_screenshot(chat_id, selector).await {
            Ok(bytes) => bytes,
            Err(e) => {
                return ToolResult {
                    content: json!({"error": format!("Screenshot capture failed: {e}. Is a browser session active? Use the browser tool first.")}).to_string(),
                    is_error: true,
                    status_code: None,
                    bytes: 0,
                    duration_ms: None,
                    error_type: None,
                    metadata: None,
                };
            }
        };

        if image_bytes.is_empty() {
            return ToolResult {
                content: json!({"error": "Screenshot was empty. The browser session may not be active."}).to_string(),
                is_error: true,
                status_code: None,
                bytes: 0,
                duration_ms: None,
                error_type: None,
                metadata: None,
            };
        }

        // Analyze with vision model
        match self.analyze_with_vision(&image_bytes, query).await {
            Ok(analysis) => ToolResult {
                content: json!({
                    "analysis": analysis,
                    "screenshot_size_bytes": image_bytes.len(),
                    "vision_model": self.vision_model,
                })
                .to_string(),
                is_error: false,
                status_code: None,
                bytes: 0,
                duration_ms: None,
                error_type: None,
                metadata: None,
            },
            Err(e) => ToolResult {
                content: json!({"error": format!("Vision analysis failed: {e}")}).to_string(),
                is_error: true,
                status_code: None,
                bytes: 0,
                duration_ms: None,
                error_type: None,
                metadata: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browser_vision_definition() {
        let config = crate::config::Config::test_defaults();
        let tool = BrowserVisionTool::new(&config);
        let def = tool.definition();
        assert_eq!(def.name, "browser_vision");
        assert!(def.description.contains("screenshot"));
        assert!(def.input_schema["required"]
            .as_array()
            .unwrap()
            .contains(&json!("query")));
    }
}
```

- [ ] **Step 2: Register in `src/tools/mod.rs`**

Add module declaration:
```rust
pub mod browser_vision;
```

Add to `ToolRegistry::new()`:
```rust
Box::new(browser_vision::BrowserVisionTool::new(config)),
```

- [ ] **Step 3: Verify build**

Run: `cargo build`
Expected: Clean build.

- [ ] **Step 4: Commit**

```bash
git add src/tools/browser_vision.rs src/tools/mod.rs
git commit -m "feat: add browser_vision tool for screenshot analysis via vision model"
```

---

### Task 4: Final Verification

- [ ] **Step 1: Run all tests**

Run: `cargo test --lib`
Expected: All tests pass.

- [ ] **Step 2: Verify new tools appear**

Check that all new tools are registered by examining the tool count.

Run: `cargo build`
Expected: Clean build.

- [ ] **Step 3: Commit any remaining changes**

```bash
git add -A
git commit -m "chore: Plan C complete — agent learning (create_skill, nudge, browser_vision)"
```
