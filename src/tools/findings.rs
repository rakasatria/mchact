use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;

use super::{schema_object, Tool, ToolResult};
use mchact_core::llm_types::ToolDefinition;
use mchact_storage::db::call_blocking;
use mchact_storage::DynDataStore;

fn extract_runtime_ids(input: &serde_json::Value) -> Option<(String, String)> {
    let meta = input.get("__subagent_runtime")?;
    let run_id = meta
        .get("run_id")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned)?;
    let orchestration_id = meta
        .get("orchestration_id")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| run_id.clone());
    Some((orchestration_id, run_id))
}

// ---------------------------------------------------------------------------
// FindingsWriteTool
// ---------------------------------------------------------------------------

pub struct FindingsWriteTool {
    db: Arc<DynDataStore>,
}

impl FindingsWriteTool {
    pub fn new(db: Arc<DynDataStore>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Tool for FindingsWriteTool {
    fn name(&self) -> &str {
        "findings_write"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "findings_write".into(),
            description: "Post a finding to the shared MoA blackboard so sibling sub-agents can read it. Use this to share discoveries, intermediate results, or observations with other workers in the same orchestrated run.".into(),
            input_schema: schema_object(
                json!({
                    "finding": {
                        "type": "string",
                        "description": "The finding or discovery to share with sibling agents"
                    },
                    "category": {
                        "type": "string",
                        "description": "Optional category to label the finding (default: \"general\")"
                    }
                }),
                &["finding"],
            ),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let finding = match input.get("finding").and_then(|v| v.as_str()) {
            Some(f) if !f.trim().is_empty() => f.trim().to_string(),
            _ => return ToolResult::error("Missing or empty required parameter: finding".into()),
        };

        let category = input
            .get("category")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("general")
            .to_string();

        let (orchestration_id, run_id) = match extract_runtime_ids(&input) {
            Some(ids) => ids,
            None => {
                return ToolResult::error(
                    "findings_write requires __subagent_runtime context (only available to sub-agents)".into(),
                )
            }
        };

        let db = self.db.clone();
        let finding_clone = finding.clone();
        let category_clone = category.clone();
        let id = match call_blocking(db, move |db| {
            db.insert_finding(&orchestration_id, &run_id, &finding_clone, &category_clone)
        })
        .await
        {
            Ok(id) => id,
            Err(e) => return ToolResult::error(format!("Failed to post finding: {e}")),
        };

        ToolResult::success(format!("Finding #{id} posted to shared blackboard."))
    }
}

// ---------------------------------------------------------------------------
// FindingsReadTool
// ---------------------------------------------------------------------------

pub struct FindingsReadTool {
    db: Arc<DynDataStore>,
}

impl FindingsReadTool {
    pub fn new(db: Arc<DynDataStore>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Tool for FindingsReadTool {
    fn name(&self) -> &str {
        "findings_read"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "findings_read".into(),
            description: "Read all findings on the shared MoA blackboard for the current orchestrated run. Returns discoveries posted by all sibling sub-agents.".into(),
            input_schema: schema_object(json!({}), &[]),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let (orchestration_id, _run_id) = match extract_runtime_ids(&input) {
            Some(ids) => ids,
            None => {
                return ToolResult::error(
                    "findings_read requires __subagent_runtime context (only available to sub-agents)".into(),
                )
            }
        };

        let db = self.db.clone();
        let findings = match call_blocking(db, move |db| db.get_findings(&orchestration_id)).await {
            Ok(f) => f,
            Err(e) => return ToolResult::error(format!("Failed to read findings: {e}")),
        };

        if findings.is_empty() {
            return ToolResult::success(
                "No findings posted to the shared blackboard yet.".into(),
            );
        }

        let mut output = format!("{} finding(s) on shared blackboard:\n\n", findings.len());
        for f in &findings {
            output.push_str(&format!(
                "#{} [{}] (run: {}) at {}\n{}\n\n",
                f.id, f.category, f.run_id, f.created_at, f.finding
            ));
        }

        ToolResult::success(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mchact_storage::db::Database;
    use serde_json::json;

    fn make_db() -> Arc<Database> {
        let dir = std::env::temp_dir().join(format!(
            "mchact_findings_{}",
            uuid::Uuid::new_v4()
        ));
        Arc::new(Database::new(dir.to_str().unwrap()).unwrap())
    }

    fn runtime_input(run_id: &str) -> serde_json::Value {
        json!({
            "__subagent_runtime": {
                "run_id": run_id,
                "depth": 1
            }
        })
    }

    fn runtime_input_with_orch(run_id: &str, orchestration_id: &str) -> serde_json::Value {
        json!({
            "__subagent_runtime": {
                "run_id": run_id,
                "orchestration_id": orchestration_id,
                "depth": 1
            }
        })
    }

    #[tokio::test]
    async fn test_write_missing_runtime_returns_error() {
        let db = make_db();
        let tool = FindingsWriteTool::new(db);
        let result = tool.execute(json!({"finding": "test"})).await;
        assert!(result.is_error);
        assert!(result.content.contains("__subagent_runtime"));
    }

    #[tokio::test]
    async fn test_write_missing_finding_returns_error() {
        let db = make_db();
        let tool = FindingsWriteTool::new(db);
        let result = tool.execute(runtime_input("run-1")).await;
        assert!(result.is_error);
        assert!(result.content.contains("finding"));
    }

    #[tokio::test]
    async fn test_write_success() {
        let db = make_db();
        let tool = FindingsWriteTool::new(db);
        let mut input = runtime_input("run-1");
        input["finding"] = json!("discovered something important");
        let result = tool.execute(input).await;
        assert!(!result.is_error, "Error: {}", result.content);
        assert!(result.content.contains("blackboard"));
        assert!(result.content.contains('#'));
    }

    #[tokio::test]
    async fn test_write_default_category() {
        let db = make_db();
        let write_tool = FindingsWriteTool::new(db.clone());
        let read_tool = FindingsReadTool::new(db);

        let mut input = runtime_input("run-cat");
        input["finding"] = json!("no category provided");
        let write_result = write_tool.execute(input).await;
        assert!(!write_result.is_error, "{}", write_result.content);

        let read_result = read_tool.execute(runtime_input("run-cat")).await;
        assert!(!read_result.is_error, "{}", read_result.content);
        assert!(read_result.content.contains("[general]"));
    }

    #[tokio::test]
    async fn test_read_missing_runtime_returns_error() {
        let db = make_db();
        let tool = FindingsReadTool::new(db);
        let result = tool.execute(json!({})).await;
        assert!(result.is_error);
        assert!(result.content.contains("__subagent_runtime"));
    }

    #[tokio::test]
    async fn test_read_empty_blackboard() {
        let db = make_db();
        let tool = FindingsReadTool::new(db);
        let result = tool.execute(runtime_input("run-empty")).await;
        assert!(!result.is_error, "Error: {}", result.content);
        assert!(result.content.contains("No findings"));
    }

    #[tokio::test]
    async fn test_write_and_read_roundtrip() {
        let db = make_db();
        let write_tool = FindingsWriteTool::new(db.clone());
        let read_tool = FindingsReadTool::new(db);

        let mut w1 = runtime_input("run-a");
        w1["finding"] = json!("first discovery");
        w1["category"] = json!("research");
        write_tool.execute(w1).await;

        let mut w2 = runtime_input("run-b");
        w2["finding"] = json!("second discovery");
        write_tool.execute(w2).await;

        let read_result = read_tool.execute(runtime_input("run-a")).await;
        assert!(!read_result.is_error, "{}", read_result.content);
        // Both findings share the same orchestration_id (run_id fallback = "run-a" and "run-b"
        // differ, so only "run-a" findings appear unless orchestration_id groups them)
        // Since no orchestration_id, each run_id is its own orchestration — only run-a's finding
        assert!(read_result.content.contains("first discovery"));
    }

    #[tokio::test]
    async fn test_orchestration_id_groups_findings_from_multiple_runs() {
        let db = make_db();
        let write_tool = FindingsWriteTool::new(db.clone());
        let read_tool = FindingsReadTool::new(db);

        let orch = "orch-123";

        let mut w1 = runtime_input_with_orch("worker-1", orch);
        w1["finding"] = json!("finding from worker 1");
        write_tool.execute(w1).await;

        let mut w2 = runtime_input_with_orch("worker-2", orch);
        w2["finding"] = json!("finding from worker 2");
        write_tool.execute(w2).await;

        let read_result = read_tool
            .execute(runtime_input_with_orch("worker-1", orch))
            .await;
        assert!(!read_result.is_error, "{}", read_result.content);
        assert!(read_result.content.contains("finding from worker 1"));
        assert!(read_result.content.contains("finding from worker 2"));
        assert!(read_result.content.contains("2 finding(s)"));
    }
}
