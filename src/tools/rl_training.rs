use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;

use super::{schema_object, Tool, ToolResult};
use crate::rl::{self, RlRunManager};
use mchact_core::llm_types::ToolDefinition;

// ---------------------------------------------------------------------------
// RlListEnvironmentsTool
// ---------------------------------------------------------------------------

pub struct RlListEnvironmentsTool {
    environments_dir: String,
}

impl RlListEnvironmentsTool {
    pub fn new(environments_dir: &str) -> Self {
        Self {
            environments_dir: environments_dir.to_string(),
        }
    }
}

#[async_trait]
impl Tool for RlListEnvironmentsTool {
    fn name(&self) -> &str {
        "rl_list_environments"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "rl_list_environments".into(),
            description:
                "List all available RL training environments discovered from the environments \
                 directory. Each entry includes the environment name, class name, file path, \
                 and description."
                    .into(),
            input_schema: schema_object(json!({}), &[]),
        }
    }

    async fn execute(&self, _input: serde_json::Value) -> ToolResult {
        let dir = Path::new(&self.environments_dir);
        match rl::discover_environments(dir) {
            Ok(envs) => {
                let list: Vec<serde_json::Value> = envs
                    .iter()
                    .map(|e| {
                        json!({
                            "name": e.name,
                            "class_name": e.class_name,
                            "file_path": e.file_path.display().to_string(),
                            "description": e.description,
                        })
                    })
                    .collect();
                ToolResult {
                    content: json!({ "environments": list, "count": list.len() }).to_string(),
                    is_error: false,
                    status_code: None,
                    bytes: 0,
                    duration_ms: None,
                    error_type: None,
                    metadata: None,
                }
            }
            Err(e) => ToolResult::error(format!("Failed to discover environments: {e}")),
        }
    }
}

// ---------------------------------------------------------------------------
// RlStartTrainingTool
// ---------------------------------------------------------------------------

pub struct RlStartTrainingTool {
    environments_dir: String,
    run_manager: Arc<RlRunManager>,
}

impl RlStartTrainingTool {
    pub fn new(environments_dir: &str, run_manager: Arc<RlRunManager>) -> Self {
        Self {
            environments_dir: environments_dir.to_string(),
            run_manager,
        }
    }
}

#[async_trait]
impl Tool for RlStartTrainingTool {
    fn name(&self) -> &str {
        "rl_start_training"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "rl_start_training".into(),
            description:
                "Start an RL training run for a named environment. Spawns the training \
                 infrastructure (API server, trainer, environment server) and returns the \
                 assigned run ID. Requires TINKER_API_KEY to be set in the environment."
                    .into(),
            input_schema: schema_object(
                json!({
                    "environment": {
                        "type": "string",
                        "description": "Name of the RL environment to train (must match a discovered environment)"
                    },
                    "config_overrides": {
                        "type": "object",
                        "description": "Optional key-value overrides for user-configurable training parameters"
                    }
                }),
                &["environment"],
            ),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        // Check TINKER_API_KEY
        if std::env::var("TINKER_API_KEY")
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            return ToolResult::error(
                "TINKER_API_KEY environment variable is not set. \
                 Please set it before starting a training run."
                    .into(),
            );
        }

        let env_name = match input.get("environment").and_then(|v| v.as_str()) {
            Some(s) => s.to_owned(),
            None => return ToolResult::error("Missing required parameter: environment".into()),
        };

        // Discover environments
        let dir = Path::new(&self.environments_dir);
        let envs = match rl::discover_environments(dir) {
            Ok(e) => e,
            Err(e) => return ToolResult::error(format!("Failed to discover environments: {e}")),
        };

        // Find the requested environment
        let env_info = match envs.into_iter().find(|e| e.name == env_name) {
            Some(e) => e,
            None => {
                return ToolResult::error(format!(
                    "Environment '{env_name}' not found. Use rl_list_environments to see available options."
                ))
            }
        };

        // Parse optional config overrides
        let config_overrides: HashMap<String, serde_json::Value> = input
            .get("config_overrides")
            .and_then(|v| v.as_object())
            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        // Generate run_id (first 8 chars of a UUID)
        let run_id = {
            use std::time::{SystemTime, UNIX_EPOCH};
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos();
            // Use uuid crate if available, otherwise derive a pseudo-unique id
            format!("{:08x}", nanos ^ std::process::id())
        };

        // Merge config
        let locked = rl::locked_config();
        let merged_config = rl::merge_config(locked, &config_overrides);

        let training_dir = dir.to_path_buf();
        let run_id_clone = run_id.clone();
        let manager = Arc::clone(&self.run_manager);
        let wandb_run_name = Some(format!("{}_{}", env_info.name, run_id));

        let result = tokio::task::spawn_blocking(move || {
            manager.start_run(
                &run_id_clone,
                &env_info,
                merged_config,
                wandb_run_name,
                &training_dir,
            )
        })
        .await;

        match result {
            Ok(Ok(())) => ToolResult {
                content: json!({
                    "run_id": run_id,
                    "environment": env_name,
                    "status": "starting",
                    "message": "Training run started successfully."
                })
                .to_string(),
                is_error: false,
                status_code: None,
                bytes: 0,
                duration_ms: None,
                error_type: None,
                metadata: None,
            },
            Ok(Err(e)) => ToolResult::error(format!("Failed to start training run: {e}")),
            Err(e) => ToolResult::error(format!("Internal error starting training run: {e}")),
        }
    }
}

// ---------------------------------------------------------------------------
// RlCheckStatusTool
// ---------------------------------------------------------------------------

pub struct RlCheckStatusTool {
    run_manager: Arc<RlRunManager>,
}

impl RlCheckStatusTool {
    pub fn new(run_manager: Arc<RlRunManager>) -> Self {
        Self { run_manager }
    }
}

#[async_trait]
impl Tool for RlCheckStatusTool {
    fn name(&self) -> &str {
        "rl_check_status"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "rl_check_status".into(),
            description:
                "Check the status of an RL training run. If no run_id is provided, returns \
                 the status of the most recently started run. Includes process health check \
                 and WandB metrics when available (rate-limited to once per 30 minutes)."
                    .into(),
            input_schema: schema_object(
                json!({
                    "run_id": {
                        "type": "string",
                        "description": "Run ID to check. Omit to check the latest run."
                    }
                }),
                &[],
            ),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        // Resolve run_id: explicit or latest
        let run_id = if let Some(id) = input.get("run_id").and_then(|v| v.as_str()) {
            id.to_owned()
        } else {
            // Pick the most recently started run (highest start_time_epoch)
            let runs = self.run_manager.list_runs();
            match runs.into_iter().max_by_key(|r| r.start_time_epoch) {
                Some(r) => r.run_id.clone(),
                None => return ToolResult::error("No training runs found.".into()),
            }
        };

        // Check process health first (updates status in manager)
        let health_status = self.run_manager.check_process_health(&run_id);

        // Fetch updated run info
        let run_info = match self.run_manager.get_run_info(&run_id) {
            Some(info) => info,
            None => {
                return ToolResult::error(format!("Run '{run_id}' not found."));
            }
        };

        let running_minutes = self.run_manager.running_time_minutes(&run_id);

        // Optionally fetch WandB metrics if rate limit allows
        let wandb_metrics = if self.run_manager.can_check_status(&run_id) {
            if let (Some(project), Some(run_name)) =
                (&run_info.wandb_project, &run_info.wandb_run_name)
            {
                let entity =
                    std::env::var("WANDB_ENTITY").unwrap_or_else(|_| "nousresearch".to_string());
                match rl::fetch_wandb_metrics(&entity, project, run_name).await {
                    Ok(metrics) => {
                        self.run_manager.mark_status_checked(&run_id);
                        Some(json!({
                            "step": metrics.step,
                            "reward_mean": metrics.reward_mean,
                            "percent_correct": metrics.percent_correct,
                            "eval_percent_correct": metrics.eval_percent_correct,
                        }))
                    }
                    Err(e) => {
                        tracing::warn!("Failed to fetch WandB metrics for run {run_id}: {e}");
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        ToolResult {
            content: json!({
                "run_id": run_id,
                "environment": run_info.environment,
                "status": run_info.status.to_string(),
                "error_message": run_info.error_message,
                "wandb_project": run_info.wandb_project,
                "wandb_run_name": run_info.wandb_run_name,
                "running_minutes": running_minutes,
                "process_health": health_status.map(|s| s.to_string()),
                "wandb_metrics": wandb_metrics,
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

// ---------------------------------------------------------------------------
// RlStopTrainingTool
// ---------------------------------------------------------------------------

pub struct RlStopTrainingTool {
    run_manager: Arc<RlRunManager>,
}

impl RlStopTrainingTool {
    pub fn new(run_manager: Arc<RlRunManager>) -> Self {
        Self { run_manager }
    }
}

#[async_trait]
impl Tool for RlStopTrainingTool {
    fn name(&self) -> &str {
        "rl_stop_training"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "rl_stop_training".into(),
            description:
                "Stop a running RL training run by killing all associated processes. \
                 If no run_id is provided, stops the most recently started run."
                    .into(),
            input_schema: schema_object(
                json!({
                    "run_id": {
                        "type": "string",
                        "description": "Run ID to stop. Omit to stop the latest run."
                    }
                }),
                &[],
            ),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        // Resolve run_id: explicit or latest
        let run_id = if let Some(id) = input.get("run_id").and_then(|v| v.as_str()) {
            id.to_owned()
        } else {
            let runs = self.run_manager.list_runs();
            match runs.into_iter().max_by_key(|r| r.start_time_epoch) {
                Some(r) => r.run_id.clone(),
                None => return ToolResult::error("No training runs found.".into()),
            }
        };

        let run_id_clone = run_id.clone();
        let manager = Arc::clone(&self.run_manager);

        let result = tokio::task::spawn_blocking(move || manager.stop_run(&run_id_clone)).await;

        match result {
            Ok(Ok(())) => ToolResult {
                content: json!({
                    "run_id": run_id,
                    "status": "stopped",
                    "message": "Training run stopped successfully."
                })
                .to_string(),
                is_error: false,
                status_code: None,
                bytes: 0,
                duration_ms: None,
                error_type: None,
                metadata: None,
            },
            Ok(Err(e)) => ToolResult::error(format!("Failed to stop training run: {e}")),
            Err(e) => ToolResult::error(format!("Internal error stopping training run: {e}")),
        }
    }
}
