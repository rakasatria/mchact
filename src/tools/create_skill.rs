use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;

use mchact_core::llm_types::ToolDefinition;

use super::{schema_object, Tool, ToolResult};

pub struct CreateSkillTool {
    skills_dir: std::path::PathBuf,
}

impl CreateSkillTool {
    pub fn new(skills_dir: &str) -> Self {
        Self {
            skills_dir: std::path::PathBuf::from(skills_dir),
        }
    }

    fn validate_skill_name(name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("skill_name must not be empty".into());
        }
        if name.len() > 64 {
            return Err(format!(
                "skill_name must be at most 64 characters, got {}",
                name.len()
            ));
        }
        if name.starts_with('-') || name.ends_with('-') {
            return Err("skill_name must not start or end with a hyphen".into());
        }
        for ch in name.chars() {
            if !ch.is_ascii_lowercase() && !ch.is_ascii_digit() && ch != '-' {
                return Err(format!(
                    "skill_name must only contain lowercase letters, digits, and hyphens; got '{ch}'"
                ));
            }
        }
        Ok(())
    }

    fn build_skill_md(
        skill_name: &str,
        description: &str,
        instructions: &str,
        platforms: &[String],
        prerequisites_commands: &[String],
        prerequisites_env_vars: &[String],
        tags: &[String],
        created_at: &str,
    ) -> String {
        let mut lines: Vec<String> = vec!["---".into()];
        lines.push(format!("name: {skill_name}"));
        lines.push(format!("description: {description}"));

        if !platforms.is_empty() {
            let platform_list = platforms
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("platforms: [{platform_list}]"));
        }

        let has_prereqs =
            !prerequisites_commands.is_empty() || !prerequisites_env_vars.is_empty();
        if has_prereqs {
            lines.push("prerequisites:".into());
            if !prerequisites_commands.is_empty() {
                let cmds = prerequisites_commands
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                lines.push(format!("  commands: [{cmds}]"));
            }
            if !prerequisites_env_vars.is_empty() {
                let vars = prerequisites_env_vars
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                lines.push(format!("  env_vars: [{vars}]"));
            }
        }

        if !tags.is_empty() {
            let tag_list = tags
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("tags: [{tag_list}]"));
        }

        lines.push("source: auto-created".into());
        lines.push(format!("created_at: {created_at}"));
        lines.push("---".into());
        lines.push(String::new());
        lines.push(instructions.to_string());

        lines.join("\n")
    }
}

#[async_trait]
impl Tool for CreateSkillTool {
    fn name(&self) -> &str {
        "create_skill"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "create_skill".into(),
            description: "Create a reusable SKILL.md from a solved problem. Saves the skill to the local skills directory so it can be activated and reused later.".into(),
            input_schema: schema_object(
                json!({
                    "skill_name": {
                        "type": "string",
                        "description": "Unique kebab-case name for the skill (max 64 chars, lowercase letters, digits, and hyphens only)"
                    },
                    "description": {
                        "type": "string",
                        "description": "One-line description of what the skill does (max 1024 chars)"
                    },
                    "instructions": {
                        "type": "string",
                        "description": "Full instructions for the skill (Markdown body, must be non-empty)"
                    },
                    "platforms": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional list of compatible platforms (e.g. [\"linux\", \"macos\"])"
                    },
                    "prerequisites": {
                        "type": "object",
                        "description": "Optional prerequisites needed to run this skill",
                        "properties": {
                            "commands": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "CLI commands that must be available (e.g. [\"docker\", \"git\"])"
                            },
                            "env_vars": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Environment variables that must be set (e.g. [\"GITHUB_TOKEN\"])"
                            }
                        }
                    },
                    "tags": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional tags for categorization (e.g. [\"deployment\", \"ci\"])"
                    }
                }),
                &["skill_name", "description", "instructions"],
            ),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let skill_name = match input.get("skill_name").and_then(|v| v.as_str()) {
            Some(v) if !v.trim().is_empty() => v.trim(),
            _ => return ToolResult::error("Missing required parameter: skill_name".into()),
        };

        if let Err(e) = Self::validate_skill_name(skill_name) {
            return ToolResult::error(e).with_error_type("validation_error");
        }

        let description = match input.get("description").and_then(|v| v.as_str()) {
            Some(v) => {
                if v.len() > 1024 {
                    return ToolResult::error(format!(
                        "description must be at most 1024 characters, got {}",
                        v.len()
                    ))
                    .with_error_type("validation_error");
                }
                v
            }
            None => return ToolResult::error("Missing required parameter: description".into()),
        };

        let instructions = match input.get("instructions").and_then(|v| v.as_str()) {
            Some(v) if !v.trim().is_empty() => v,
            Some(_) => {
                return ToolResult::error("instructions must not be empty".into())
                    .with_error_type("validation_error")
            }
            None => return ToolResult::error("Missing required parameter: instructions".into()),
        };

        let platforms = extract_string_array(&input, "platforms");

        let prerequisites_commands = input
            .get("prerequisites")
            .and_then(|p| p.get("commands"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let prerequisites_env_vars = input
            .get("prerequisites")
            .and_then(|p| p.get("env_vars"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let tags = extract_string_array(&input, "tags");

        let out_dir = self.skills_dir.join(skill_name);
        let out_file = out_dir.join("SKILL.md");

        if out_file.exists() {
            return ToolResult::error(format!(
                "Skill '{}' already exists at {}. Use a different name or delete the existing skill first.",
                skill_name,
                out_file.display()
            ))
            .with_error_type("duplicate_skill");
        }

        if let Err(e) = std::fs::create_dir_all(&out_dir) {
            return ToolResult::error(format!("Failed to create skill directory: {e}"))
                .with_error_type("write_failed");
        }

        let created_at = Utc::now().to_rfc3339();
        let content = Self::build_skill_md(
            skill_name,
            description,
            instructions,
            &platforms,
            &prerequisites_commands,
            &prerequisites_env_vars,
            &tags,
            &created_at,
        );

        if let Err(e) = std::fs::write(&out_file, &content) {
            return ToolResult::error(format!("Failed to write SKILL.md: {e}"))
                .with_error_type("write_failed");
        }

        let path = out_file.display().to_string();
        let result_json = json!({
            "skill_name": skill_name,
            "path": path,
            "description": description,
            "message": format!("Skill '{}' created successfully at {}", skill_name, path)
        });

        ToolResult::success(result_json.to_string())
    }
}

fn extract_string_array(input: &serde_json::Value, key: &str) -> Vec<String> {
    input
        .get(key)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn test_validate_skill_name_valid() {
        assert!(CreateSkillTool::validate_skill_name("my-skill").is_ok());
        assert!(CreateSkillTool::validate_skill_name("deploy-to-k8s").is_ok());
        assert!(CreateSkillTool::validate_skill_name("skill123").is_ok());
        assert!(CreateSkillTool::validate_skill_name("a").is_ok());
        assert!(CreateSkillTool::validate_skill_name("abc-123-xyz").is_ok());
    }

    #[test]
    fn test_validate_skill_name_invalid() {
        assert!(CreateSkillTool::validate_skill_name("").is_err());
        assert!(CreateSkillTool::validate_skill_name("-leading").is_err());
        assert!(CreateSkillTool::validate_skill_name("trailing-").is_err());
        assert!(CreateSkillTool::validate_skill_name("UPPERCASE").is_err());
        assert!(CreateSkillTool::validate_skill_name("has space").is_err());
        assert!(CreateSkillTool::validate_skill_name("has_underscore").is_err());
        assert!(CreateSkillTool::validate_skill_name("a".repeat(65).as_str()).is_err());
    }

    #[test]
    fn test_build_skill_md_minimal() {
        let content = CreateSkillTool::build_skill_md(
            "my-skill",
            "A short description",
            "Do the thing.",
            &[],
            &[],
            &[],
            &[],
            "2026-03-27T14:00:00Z",
        );
        assert!(content.starts_with("---\n"));
        assert!(content.contains("name: my-skill"));
        assert!(content.contains("description: A short description"));
        assert!(content.contains("source: auto-created"));
        assert!(content.contains("created_at: 2026-03-27T14:00:00Z"));
        assert!(content.contains("Do the thing."));
        assert!(!content.contains("platforms:"));
        assert!(!content.contains("prerequisites:"));
        assert!(!content.contains("tags:"));
    }

    #[test]
    fn test_build_skill_md_full() {
        let content = CreateSkillTool::build_skill_md(
            "deploy-app",
            "Deploy application to production",
            "Step 1: build\nStep 2: deploy",
            &["linux".into(), "macos".into()],
            &["docker".into()],
            &["GITHUB_TOKEN".into()],
            &["deployment".into(), "ci".into()],
            "2026-03-27T14:00:00Z",
        );
        assert!(content.contains("name: deploy-app"));
        assert!(content.contains("platforms: [linux, macos]"));
        assert!(content.contains("prerequisites:"));
        assert!(content.contains("  commands: [docker]"));
        assert!(content.contains("  env_vars: [GITHUB_TOKEN]"));
        assert!(content.contains("tags: [deployment, ci]"));
        assert!(content.contains("Step 1: build"));
    }

    #[tokio::test]
    async fn test_create_skill_writes_file() {
        let tmp = TempDir::new().unwrap();
        let tool = CreateSkillTool::new(tmp.path().to_str().unwrap());

        let result = tool
            .execute(json!({
                "skill_name": "test-skill",
                "description": "Test skill description",
                "instructions": "## Instructions\n\nDo the test thing.",
                "tags": ["test"]
            }))
            .await;

        assert!(!result.is_error, "Expected success, got: {}", result.content);

        let skill_file = tmp.path().join("test-skill").join("SKILL.md");
        assert!(skill_file.exists(), "SKILL.md should have been written");

        let written = std::fs::read_to_string(&skill_file).unwrap();
        assert!(written.contains("name: test-skill"));
        assert!(written.contains("Test skill description"));
        assert!(written.contains("Do the test thing."));

        let result_json: serde_json::Value =
            serde_json::from_str(&result.content).expect("should return valid JSON");
        assert_eq!(result_json["skill_name"], "test-skill");
        assert!(result_json["path"].as_str().unwrap().ends_with("SKILL.md"));
    }

    #[tokio::test]
    async fn test_create_skill_rejects_duplicate() {
        let tmp = TempDir::new().unwrap();
        let tool = CreateSkillTool::new(tmp.path().to_str().unwrap());

        let input = json!({
            "skill_name": "duplicate-skill",
            "description": "A skill",
            "instructions": "Instructions here."
        });

        let first = tool.execute(input.clone()).await;
        assert!(!first.is_error, "First call should succeed");

        let second = tool.execute(input).await;
        assert!(second.is_error, "Second call should fail with duplicate error");
        assert!(
            second.content.contains("already exists"),
            "Error should mention already exists: {}",
            second.content
        );
    }
}
