use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const MIN_STATUS_CHECK_INTERVAL_SECS: u64 = 1800;

// ---------------------------------------------------------------------------
// Environment Discovery
// ---------------------------------------------------------------------------

/// Metadata parsed from a Python RL environment file's YAML frontmatter comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentInfo {
    pub name: String,
    pub class_name: String,
    pub file_path: PathBuf,
    pub description: String,
}

/// Parse a single `.py` file's YAML frontmatter comment block.
///
/// Frontmatter format:
/// ```python
/// # ---
/// # name: web_research
/// # class: WebResearchEnv
/// # description: Web research and information synthesis tasks
/// # ---
/// ```
pub fn parse_env_frontmatter(path: &Path) -> Option<EnvironmentInfo> {
    let content = fs::read_to_string(path).ok()?;

    let mut in_frontmatter = false;
    let mut name: Option<String> = None;
    let mut class_name: Option<String> = None;
    let mut description: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed == "# ---" {
            if !in_frontmatter {
                in_frontmatter = true;
                continue;
            } else {
                // Closing delimiter — stop parsing
                break;
            }
        }

        if !in_frontmatter {
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("# ") {
            if let Some((key, value)) = rest.split_once(':') {
                let key = key.trim();
                let value = value.trim().to_string();
                match key {
                    "name" => name = Some(value),
                    "class" => class_name = Some(value),
                    "description" => description = Some(value),
                    _ => {}
                }
            }
        }
    }

    Some(EnvironmentInfo {
        name: name?,
        class_name: class_name?,
        file_path: path.to_path_buf(),
        description: description.unwrap_or_default(),
    })
}

/// Scan `dir` for `.py` files that contain valid RL environment frontmatter.
///
/// Returns an empty `Vec` (not an error) if `dir` does not exist.
/// Results are sorted by `name` for deterministic ordering.
pub fn discover_environments(dir: &Path) -> Result<Vec<EnvironmentInfo>, String> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(dir)
        .map_err(|e| format!("failed to read directory {}: {}", dir.display(), e))?;

    let mut envs: Vec<EnvironmentInfo> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension()?.to_str()? == "py" {
                parse_env_frontmatter(&path)
            } else {
                None
            }
        })
        .collect();

    envs.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(envs)
}

// ---------------------------------------------------------------------------
// Run State Types
// ---------------------------------------------------------------------------

/// Current lifecycle status of an RL training run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RlRunStatus {
    Pending,
    Starting,
    Running,
    Stopping,
    Stopped,
    Completed,
    Failed,
}

impl fmt::Display for RlRunStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            RlRunStatus::Pending => "pending",
            RlRunStatus::Starting => "starting",
            RlRunStatus::Running => "running",
            RlRunStatus::Stopping => "stopping",
            RlRunStatus::Stopped => "stopped",
            RlRunStatus::Completed => "completed",
            RlRunStatus::Failed => "failed",
        };
        write!(f, "{s}")
    }
}

/// Full description of a single RL run instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RlRunInfo {
    pub run_id: String,
    pub environment: String,
    pub status: RlRunStatus,
    pub error_message: Option<String>,
    pub wandb_project: Option<String>,
    pub wandb_run_name: Option<String>,
    pub start_time_epoch: u64,
    pub config: Value,
}

// ---------------------------------------------------------------------------
// Locked Config
// ---------------------------------------------------------------------------

/// Returns the full locked infrastructure configuration.
///
/// These fields cannot be overridden by the user to ensure reproducibility and
/// prevent accidental changes to critical infrastructure settings.
pub fn locked_config() -> Value {
    serde_json::json!({
        "env": {
            "framework": "grpo",
            "backend": "vllm",
            "dtype": "bfloat16",
            "gradient_checkpointing": true
        },
        "openai": {
            "api_version": "v1",
            "timeout_secs": 120,
            "max_retries": 3
        },
        "tinker": {
            "reward_normalization": "z_score",
            "clip_ratio": 0.2,
            "entropy_coef": 0.01
        },
        "slurm": {
            "partition": "gpu",
            "exclusive": true,
            "requeue": true
        },
        "testing": {
            "seed": 42,
            "deterministic": true
        }
    })
}

/// Merge user overrides into a "user" section alongside the locked config fields.
///
/// Locked fields are preserved at the top level; user overrides are placed
/// under a `"user"` key so they can be inspected separately.
pub fn merge_config(locked: Value, user_overrides: &HashMap<String, Value>) -> Value {
    let mut result = match locked {
        Value::Object(map) => map,
        other => {
            let mut map = serde_json::Map::new();
            map.insert("locked".to_string(), other);
            map
        }
    };

    if !user_overrides.is_empty() {
        let user_obj: serde_json::Map<String, Value> = user_overrides
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        result.insert("user".to_string(), Value::Object(user_obj));
    }

    Value::Object(result)
}

/// Returns `true` if `field` corresponds to a locked configuration key.
///
/// Checks top-level section names (`env`, `openai`, `tinker`, `slurm`,
/// `testing`) as well as known nested field names within `env` and `tinker`.
pub fn is_locked_field(field: &str) -> bool {
    const TOP_LEVEL: &[&str] = &["env", "openai", "tinker", "slurm", "testing"];

    const ENV_FIELDS: &[&str] = &[
        "framework",
        "backend",
        "dtype",
        "gradient_checkpointing",
    ];

    const TINKER_FIELDS: &[&str] = &["reward_normalization", "clip_ratio", "entropy_coef"];

    TOP_LEVEL.contains(&field) || ENV_FIELDS.contains(&field) || TINKER_FIELDS.contains(&field)
}

// ---------------------------------------------------------------------------
// WandB Metrics
// ---------------------------------------------------------------------------

/// Metrics fetched from a WandB run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WandbMetrics {
    pub step: Option<u64>,
    pub reward_mean: Option<f64>,
    pub percent_correct: Option<f64>,
    pub eval_percent_correct: Option<f64>,
}

/// Fetch the latest metrics for a WandB run via the GraphQL API.
///
/// Returns `Err` if the HTTP request fails or the response cannot be parsed.
pub async fn fetch_wandb_metrics(
    entity: &str,
    project: &str,
    run_name: &str,
) -> Result<WandbMetrics, String> {
    let query = format!(
        r#"{{
            "query": "{{ project(name: \"{project}\", entityName: \"{entity}\") {{ run(name: \"{run_name}\") {{ summaryMetrics }} }} }}"
        }}"#,
        entity = entity,
        project = project,
        run_name = run_name,
    );

    let client = reqwest::Client::new();
    let response = client
        .post("https://api.wandb.ai/graphql")
        .header("Content-Type", "application/json")
        .body(query)
        .send()
        .await
        .map_err(|e| format!("wandb request failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "wandb API returned status {}",
            response.status()
        ));
    }

    let body: Value = response
        .json()
        .await
        .map_err(|e| format!("failed to parse wandb response: {e}"))?;

    // Navigate: data.project.run.summaryMetrics (JSON string) -> parse
    let summary_str = body
        .pointer("/data/project/run/summaryMetrics")
        .and_then(|v| v.as_str())
        .unwrap_or("{}");

    let summary: Value =
        serde_json::from_str(summary_str).unwrap_or(Value::Object(Default::default()));

    let step = summary
        .get("_step")
        .and_then(|v| v.as_u64());
    let reward_mean = summary
        .get("reward_mean")
        .and_then(|v| v.as_f64());
    let percent_correct = summary
        .get("percent_correct")
        .and_then(|v| v.as_f64());
    let eval_percent_correct = summary
        .get("eval/percent_correct")
        .and_then(|v| v.as_f64());

    Ok(WandbMetrics {
        step,
        reward_mean,
        percent_correct,
        eval_percent_correct,
    })
}

// ---------------------------------------------------------------------------
// Run Supervisor
// ---------------------------------------------------------------------------

use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Live state for a single RL training run, including OS-level child processes.
pub struct RlRun {
    pub info: RlRunInfo,
    pub processes: Vec<Child>,
    pub start_instant: Instant,
    pub last_status_check: Option<Instant>,
}

/// Thread-safe manager that tracks all active and historical RL training runs.
pub struct RlRunManager {
    runs: Mutex<HashMap<String, RlRun>>,
}

impl RlRunManager {
    pub fn new() -> Self {
        Self {
            runs: Mutex::new(HashMap::new()),
        }
    }

    /// Return a clone of `RlRunInfo` for the given run, or `None` if not found.
    pub fn get_run_info(&self, run_id: &str) -> Option<RlRunInfo> {
        let runs = self.runs.lock().unwrap_or_else(|p| p.into_inner());
        runs.get(run_id).map(|r| r.info.clone())
    }

    /// Return cloned `RlRunInfo` for all tracked runs.
    pub fn list_runs(&self) -> Vec<RlRunInfo> {
        let runs = self.runs.lock().unwrap_or_else(|p| p.into_inner());
        runs.values().map(|r| r.info.clone()).collect()
    }

    /// Update the status (and optional error message) of an existing run.
    pub fn update_status(&self, run_id: &str, status: RlRunStatus, error: Option<String>) {
        let mut runs = self.runs.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(run) = runs.get_mut(run_id) {
            run.info.status = status;
            run.info.error_message = error;
        }
    }

    /// Return `true` if the run has never had its status checked, or if at
    /// least `MIN_STATUS_CHECK_INTERVAL_SECS` seconds have elapsed since the
    /// last check.
    pub fn can_check_status(&self, run_id: &str) -> bool {
        let runs = self.runs.lock().unwrap_or_else(|p| p.into_inner());
        let Some(run) = runs.get(run_id) else {
            return false;
        };
        match run.last_status_check {
            None => true,
            Some(t) => t.elapsed() >= Duration::from_secs(MIN_STATUS_CHECK_INTERVAL_SECS),
        }
    }

    /// Record the current instant as the time of the most recent status check.
    pub fn mark_status_checked(&self, run_id: &str) {
        let mut runs = self.runs.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(run) = runs.get_mut(run_id) {
            run.last_status_check = Some(Instant::now());
        }
    }

    /// Return elapsed wall-clock time in minutes since the run was created.
    /// Returns `0.0` if the run does not exist.
    pub fn running_time_minutes(&self, run_id: &str) -> f64 {
        let runs = self.runs.lock().unwrap_or_else(|p| p.into_inner());
        match runs.get(run_id) {
            Some(run) => run.start_instant.elapsed().as_secs_f64() / 60.0,
            None => 0.0,
        }
    }

    /// Spawn the three training processes for a new run.
    ///
    /// Steps:
    /// 1. Write merged config YAML to `training_dir/run_{id}_config.yaml`.
    /// 2. Insert a `Starting` run record.
    /// 3. Spawn `run-api` (Process 1), wait 5 s, verify it is alive.
    /// 4. Spawn `python3 launch_training.py --config <path>` (Process 2), wait 30 s.
    /// 5. Spawn `python3 <env_file> serve --config <path>` (Process 3).
    /// 6. Transition run to `Running`.
    pub fn start_run(
        &self,
        run_id: &str,
        environment: &EnvironmentInfo,
        config: Value,
        wandb_run_name: Option<String>,
        training_dir: &Path,
    ) -> Result<(), String> {
        // 1. Write config YAML to disk
        let config_filename = format!("run_{run_id}_config.yaml");
        let config_path = training_dir.join(&config_filename);
        let yaml_str = serde_yaml::to_string(&config)
            .map_err(|e| format!("failed to serialize config: {e}"))?;
        fs::write(&config_path, &yaml_str)
            .map_err(|e| format!("failed to write config file: {e}"))?;

        // 2. Insert Starting run record
        let now_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let info = RlRunInfo {
            run_id: run_id.to_string(),
            environment: environment.name.clone(),
            status: RlRunStatus::Starting,
            error_message: None,
            wandb_project: None,
            wandb_run_name,
            start_time_epoch: now_epoch,
            config: config.clone(),
        };

        let run = RlRun {
            info,
            processes: Vec::new(),
            start_instant: Instant::now(),
            last_status_check: None,
        };

        {
            let mut runs = self.runs.lock().unwrap_or_else(|p| p.into_inner());
            runs.insert(run_id.to_string(), run);
        }

        // 3. Spawn Process 1: run-api
        let mut api_proc = Command::new("run-api")
            .current_dir(training_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("failed to spawn run-api: {e}"))?;

        // Wait 5 seconds then verify alive
        std::thread::sleep(Duration::from_secs(5));
        match api_proc.try_wait() {
            Ok(Some(status)) => {
                return Err(format!(
                    "run-api exited unexpectedly after 5 s (status: {status})"
                ));
            }
            Err(e) => return Err(format!("failed to check run-api status: {e}")),
            Ok(None) => {} // still running — good
        }

        // 4. Spawn Process 2: python3 launch_training.py --config <path>
        let tinker_api_key = std::env::var("TINKER_API_KEY").unwrap_or_default();
        let trainer_proc = Command::new("python3")
            .arg("launch_training.py")
            .arg("--config")
            .arg(&config_path)
            .current_dir(training_dir)
            .env("TINKER_API_KEY", &tinker_api_key)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("failed to spawn launch_training.py: {e}"))?;

        // Wait 30 seconds before starting the environment server
        std::thread::sleep(Duration::from_secs(30));

        // 5. Spawn Process 3: python3 <env_file> serve --config <path>
        let env_proc = Command::new("python3")
            .arg(&environment.file_path)
            .arg("serve")
            .arg("--config")
            .arg(&config_path)
            .current_dir(training_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("failed to spawn environment server: {e}"))?;

        // 6. Store all processes and mark Running
        {
            let mut runs = self.runs.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(run) = runs.get_mut(run_id) {
                run.processes.push(api_proc);
                run.processes.push(trainer_proc);
                run.processes.push(env_proc);
                run.info.status = RlRunStatus::Running;
            }
        }

        Ok(())
    }

    /// Kill all processes for a run and wait for them to exit.
    ///
    /// Processes are killed in reverse order (env → trainer → api) so that
    /// dependent processes are stopped before their dependencies.
    pub fn stop_run(&self, run_id: &str) -> Result<(), String> {
        self.update_status(run_id, RlRunStatus::Stopping, None);

        let mut runs = self.runs.lock().unwrap_or_else(|p| p.into_inner());
        let run = runs
            .get_mut(run_id)
            .ok_or_else(|| format!("run {run_id} not found"))?;

        // Kill in reverse order
        for proc in run.processes.iter_mut().rev() {
            let _ = proc.kill();
        }

        // Wait for all to exit
        for proc in run.processes.iter_mut() {
            let _ = proc.wait();
        }

        run.info.status = RlRunStatus::Stopped;
        Ok(())
    }

    /// Check the health of all child processes for a run.
    ///
    /// Returns:
    /// - `None` if the run does not exist.
    /// - The current status unchanged if the run is not in `Running` state.
    /// - `Completed` if any process exited with code 0.
    /// - `Failed` (with error message) if any process exited with non-zero code.
    /// - `Running` if all processes are still alive.
    pub fn check_process_health(&self, run_id: &str) -> Option<RlRunStatus> {
        let mut runs = self.runs.lock().unwrap_or_else(|p| p.into_inner());
        let run = runs.get_mut(run_id)?;

        if run.info.status != RlRunStatus::Running {
            return Some(run.info.status);
        }

        for proc in run.processes.iter_mut() {
            match proc.try_wait() {
                Ok(Some(exit_status)) => {
                    if exit_status.success() {
                        run.info.status = RlRunStatus::Completed;
                        return Some(RlRunStatus::Completed);
                    } else {
                        let msg = format!(
                            "process exited with non-zero status: {}",
                            exit_status
                                .code()
                                .map(|c| c.to_string())
                                .unwrap_or_else(|| "unknown".to_string())
                        );
                        run.info.status = RlRunStatus::Failed;
                        run.info.error_message = Some(msg);
                        return Some(RlRunStatus::Failed);
                    }
                }
                Ok(None) => {} // still running — continue
                Err(e) => {
                    let msg = format!("failed to poll process status: {e}");
                    run.info.status = RlRunStatus::Failed;
                    run.info.error_message = Some(msg);
                    return Some(RlRunStatus::Failed);
                }
            }
        }

        Some(RlRunStatus::Running)
    }
}

impl Default for RlRunManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_env_file(dir: &Path, filename: &str, content: &str) -> PathBuf {
        let path = dir.join(filename);
        fs::write(&path, content).expect("failed to write test file");
        path
    }

    const VALID_FRONTMATTER: &str = r#"# ---
# name: web_research
# class: WebResearchEnv
# description: Web research and information synthesis tasks
# ---

class WebResearchEnv:
    pass
"#;

    #[test]
    fn test_parse_env_frontmatter() {
        let dir = TempDir::new().unwrap();
        let path = write_env_file(dir.path(), "web_research.py", VALID_FRONTMATTER);

        let info = parse_env_frontmatter(&path).expect("should parse successfully");
        assert_eq!(info.name, "web_research");
        assert_eq!(info.class_name, "WebResearchEnv");
        assert_eq!(info.description, "Web research and information synthesis tasks");
        assert_eq!(info.file_path, path);
    }

    #[test]
    fn test_discover_environments() {
        let dir = TempDir::new().unwrap();

        write_env_file(dir.path(), "env_a.py", VALID_FRONTMATTER);

        let second = r#"# ---
# name: coding_tasks
# class: CodingEnv
# description: Code generation and debugging tasks
# ---
class CodingEnv:
    pass
"#;
        write_env_file(dir.path(), "env_b.py", second);

        // Non-env file — should be ignored
        write_env_file(
            dir.path(),
            "helper.py",
            "# plain python file\ndef helper(): pass\n",
        );

        let envs = discover_environments(dir.path()).expect("discover should succeed");
        assert_eq!(envs.len(), 2, "should find exactly 2 environments");

        // Results must be sorted by name
        assert_eq!(envs[0].name, "coding_tasks");
        assert_eq!(envs[1].name, "web_research");
    }

    #[test]
    fn test_discover_environments_empty_dir() {
        let dir = TempDir::new().unwrap();
        let envs = discover_environments(dir.path()).expect("discover should succeed on empty dir");
        assert!(envs.is_empty());
    }

    #[test]
    fn test_discover_environments_nonexistent() {
        let path = Path::new("/tmp/__nonexistent_rl_test_dir__");
        let envs =
            discover_environments(path).expect("nonexistent dir should return empty vec, not error");
        assert!(envs.is_empty());
    }

    #[test]
    fn test_locked_config_structure() {
        let config = locked_config();
        assert!(config.get("env").is_some(), "env key must exist");
        assert!(config.get("tinker").is_some(), "tinker key must exist");
        assert!(config.get("openai").is_some(), "openai key must exist");
        assert!(config.get("slurm").is_some(), "slurm key must exist");
        assert!(config.get("testing").is_some(), "testing key must exist");
    }

    #[test]
    fn test_is_locked_field() {
        // Top-level sections
        assert!(is_locked_field("env"));
        assert!(is_locked_field("tinker"));
        assert!(is_locked_field("openai"));
        assert!(is_locked_field("slurm"));
        assert!(is_locked_field("testing"));

        // Nested fields
        assert!(is_locked_field("framework"));
        assert!(is_locked_field("clip_ratio"));

        // User-defined fields — should not be locked
        assert!(!is_locked_field("my_custom_param"));
        assert!(!is_locked_field("learning_rate"));
        assert!(!is_locked_field("epochs"));
    }

    #[test]
    fn test_merge_config() {
        let locked = locked_config();
        let mut overrides = HashMap::new();
        overrides.insert("learning_rate".to_string(), serde_json::json!(1e-4));
        overrides.insert("epochs".to_string(), serde_json::json!(10));

        let merged = merge_config(locked.clone(), &overrides);

        // Locked top-level keys preserved
        assert!(merged.get("env").is_some());
        assert!(merged.get("tinker").is_some());

        // User overrides placed under "user" section
        let user = merged.get("user").expect("user section must exist");
        assert_eq!(user.get("learning_rate"), Some(&serde_json::json!(1e-4)));
        assert_eq!(user.get("epochs"), Some(&serde_json::json!(10)));
    }

    #[test]
    fn test_rl_run_status_display() {
        assert_eq!(RlRunStatus::Pending.to_string(), "pending");
        assert_eq!(RlRunStatus::Starting.to_string(), "starting");
        assert_eq!(RlRunStatus::Running.to_string(), "running");
        assert_eq!(RlRunStatus::Stopping.to_string(), "stopping");
        assert_eq!(RlRunStatus::Stopped.to_string(), "stopped");
        assert_eq!(RlRunStatus::Completed.to_string(), "completed");
        assert_eq!(RlRunStatus::Failed.to_string(), "failed");
    }

    /// Verify that `RlRunManager` basic lifecycle operations work without real
    /// processes: insert a run manually, then test list/get/update_status.
    #[test]
    fn test_run_manager_lifecycle() {
        let manager = RlRunManager::new();

        // No runs yet
        assert!(manager.list_runs().is_empty());
        assert!(manager.get_run_info("run-1").is_none());

        // Insert a run directly into the map (simulates a starting run)
        {
            let run = RlRun {
                info: RlRunInfo {
                    run_id: "run-1".to_string(),
                    environment: "test_env".to_string(),
                    status: RlRunStatus::Starting,
                    error_message: None,
                    wandb_project: None,
                    wandb_run_name: Some("wandb-run-1".to_string()),
                    start_time_epoch: 1_700_000_000,
                    config: serde_json::json!({}),
                },
                processes: Vec::new(),
                start_instant: Instant::now(),
                last_status_check: None,
            };
            let mut runs = manager.runs.lock().unwrap();
            runs.insert("run-1".to_string(), run);
        }

        // list_runs returns it
        let all = manager.list_runs();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].run_id, "run-1");
        assert_eq!(all[0].status, RlRunStatus::Starting);

        // get_run_info returns a clone
        let info = manager.get_run_info("run-1").expect("should find run-1");
        assert_eq!(info.environment, "test_env");
        assert_eq!(info.wandb_run_name.as_deref(), Some("wandb-run-1"));

        // update_status transitions to Running
        manager.update_status("run-1", RlRunStatus::Running, None);
        let updated = manager.get_run_info("run-1").unwrap();
        assert_eq!(updated.status, RlRunStatus::Running);
        assert!(updated.error_message.is_none());

        // update_status can record an error
        manager.update_status(
            "run-1",
            RlRunStatus::Failed,
            Some("trainer crashed".to_string()),
        );
        let failed = manager.get_run_info("run-1").unwrap();
        assert_eq!(failed.status, RlRunStatus::Failed);
        assert_eq!(failed.error_message.as_deref(), Some("trainer crashed"));

        // Non-existent run
        assert!(manager.get_run_info("run-999").is_none());
    }

    /// Verify that `can_check_status` / `mark_status_checked` enforce rate
    /// limiting: first call returns true, immediately after marking it returns
    /// false (because < MIN_STATUS_CHECK_INTERVAL_SECS have elapsed).
    #[test]
    fn test_status_check_rate_limiting() {
        let manager = RlRunManager::new();

        // Insert a run
        {
            let run = RlRun {
                info: RlRunInfo {
                    run_id: "run-2".to_string(),
                    environment: "test_env".to_string(),
                    status: RlRunStatus::Running,
                    error_message: None,
                    wandb_project: None,
                    wandb_run_name: None,
                    start_time_epoch: 1_700_000_000,
                    config: serde_json::json!({}),
                },
                processes: Vec::new(),
                start_instant: Instant::now(),
                last_status_check: None,
            };
            let mut runs = manager.runs.lock().unwrap();
            runs.insert("run-2".to_string(), run);
        }

        // Non-existent run is always false
        assert!(!manager.can_check_status("run-999"));

        // First call — never checked before
        assert!(
            manager.can_check_status("run-2"),
            "should be checkable on first call"
        );

        // Mark as checked
        manager.mark_status_checked("run-2");

        // Immediately after marking, should NOT be checkable (cooldown active)
        assert!(
            !manager.can_check_status("run-2"),
            "should NOT be checkable immediately after mark_status_checked"
        );
    }
}
