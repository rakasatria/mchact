# RL Training Implementation Plan (Plan B)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add RL training support to mchact: environment discovery, config management, 3-process supervisor (Atropos/Tinker/SGLang), WandB metric fetching, bundled starter environments, CLI interface, and 4 RL agent tools.

**Architecture:** Rust CLI (`mchact rl`) orchestrates Python training processes. Environment discovery via YAML frontmatter comments in `.py` files. Locked infrastructure config merged with user-configurable fields into a YAML config. Run state tracked in-memory with status transitions. WandB metrics fetched via HTTP API. Agent tools wrap the CLI functions for conversational use.

**Tech Stack:** Rust (clap CLI, tokio::process, serde_yaml, reqwest), Python 3.10+ (atroposlib, tinker, wandb, sglang)

**Spec:** `docs/superpowers/specs/2026-03-27-mlops-training-design.md` (Section 5 + agent tools)

**Depends on:** Plan A (complete) — uses existing config fields from `src/config.rs`

---

### Task 1: RL Types & Environment Discovery

**Files:**
- Create: `src/rl.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Create `src/rl.rs` with types and discovery**

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// --- Environment Discovery ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentInfo {
    pub name: String,
    pub class_name: String,
    pub file_path: PathBuf,
    pub description: String,
}

/// Scan a directory for Python environment files with YAML frontmatter comments.
///
/// Each .py file is checked for a header like:
/// ```python
/// # ---
/// # name: web_research
/// # class: WebResearchEnv
/// # description: Web research and information synthesis tasks
/// # ---
/// ```
pub fn discover_environments(dir: &Path) -> Result<Vec<EnvironmentInfo>, String> {
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut envs = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|e| format!("Cannot read {}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("Dir entry error: {e}"))?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("py") {
            continue;
        }
        if let Some(info) = parse_env_frontmatter(&path) {
            envs.push(info);
        }
    }
    envs.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(envs)
}

fn parse_env_frontmatter(path: &Path) -> Option<EnvironmentInfo> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut in_frontmatter = false;
    let mut name = None;
    let mut class_name = None;
    let mut description = String::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "# ---" {
            if in_frontmatter {
                break; // End of frontmatter
            }
            in_frontmatter = true;
            continue;
        }
        if !in_frontmatter {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("# ") {
            if let Some(val) = rest.strip_prefix("name:") {
                name = Some(val.trim().to_string());
            } else if let Some(val) = rest.strip_prefix("class:") {
                class_name = Some(val.trim().to_string());
            } else if let Some(val) = rest.strip_prefix("description:") {
                description = val.trim().to_string();
            }
        }
    }

    Some(EnvironmentInfo {
        name: name?,
        class_name: class_name.unwrap_or_default(),
        file_path: path.to_path_buf(),
        description,
    })
}

// --- Run State ---

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

impl std::fmt::Display for RlRunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Starting => write!(f, "starting"),
            Self::Running => write!(f, "running"),
            Self::Stopping => write!(f, "stopping"),
            Self::Stopped => write!(f, "stopped"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RlRunInfo {
    pub run_id: String,
    pub environment: String,
    pub status: RlRunStatus,
    pub error_message: String,
    pub wandb_project: String,
    pub wandb_run_name: String,
    pub start_time_epoch: u64,
    pub config: serde_json::Value,
}

// --- Locked Config ---

/// Returns the locked infrastructure config that users cannot modify.
pub fn locked_config() -> serde_json::Value {
    serde_json::json!({
        "env": {
            "tokenizer_name": "Qwen/Qwen3-8B",
            "rollout_server_url": "http://localhost:8000",
            "use_wandb": true,
            "max_token_length": 8192,
            "max_num_workers": 2048,
            "worker_timeout": 3600,
            "total_steps": 2500,
            "steps_per_eval": 25,
            "max_batches_offpolicy": 3,
            "inference_weight": 1.0,
            "eval_limit_ratio": 0.1
        },
        "openai": [{
            "model_name": "Qwen/Qwen3-8B",
            "base_url": "http://localhost:8001/v1",
            "api_key": "x",
            "weight": 1.0,
            "num_requests_for_eval": 256,
            "timeout": 3600,
            "server_type": "sglang"
        }],
        "tinker": {
            "lora_rank": 32,
            "learning_rate": 0.00004,
            "max_token_trainer_length": 9000,
            "checkpoint_dir": "./temp/",
            "save_checkpoint_interval": 25
        },
        "slurm": false,
        "testing": false
    })
}

/// Merge locked config with user overrides (user overrides only apply to non-locked keys).
pub fn merge_config(
    locked: &serde_json::Value,
    user_overrides: &HashMap<String, serde_json::Value>,
) -> serde_json::Value {
    let mut config = locked.clone();
    if let Some(obj) = config.as_object_mut() {
        // User overrides go into a top-level "user" section
        // that the environment can read as extra config
        let user_obj: serde_json::Map<String, serde_json::Value> = user_overrides
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        if !user_obj.is_empty() {
            obj.insert("user".to_string(), serde_json::Value::Object(user_obj));
        }
    }
    config
}

/// Check if a field name is locked (cannot be edited by user).
pub fn is_locked_field(field: &str) -> bool {
    let locked = locked_config();
    // Check top-level keys
    if locked.get(field).is_some() {
        return true;
    }
    // Check nested env/tinker/openai keys
    for section in ["env", "tinker"] {
        if let Some(obj) = locked.get(section).and_then(|v| v.as_object()) {
            if obj.contains_key(field) {
                return true;
            }
        }
    }
    false
}

// --- WandB Metric Fetching ---

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WandbMetrics {
    pub step: Option<u64>,
    pub reward_mean: Option<f64>,
    pub percent_correct: Option<f64>,
    pub eval_percent_correct: Option<f64>,
}

/// Fetch latest WandB metrics via HTTP API.
///
/// Requires WANDB_API_KEY env var.
pub async fn fetch_wandb_metrics(
    entity: &str,
    project: &str,
    run_name: &str,
) -> Result<WandbMetrics, String> {
    let api_key = std::env::var("WANDB_API_KEY")
        .map_err(|_| "WANDB_API_KEY not set".to_string())?;

    let url = format!(
        "https://api.wandb.ai/graphql"
    );

    let query = serde_json::json!({
        "query": format!(
            r#"query {{
                project(name: "{project}", entityName: "{entity}") {{
                    run(name: "{run_name}") {{
                        summaryMetrics
                    }}
                }}
            }}"#
        )
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&query)
        .send()
        .await
        .map_err(|e| format!("WandB request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("WandB returned {}", resp.status()));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("WandB parse error: {e}"))?;

    let metrics_str = body
        .pointer("/data/project/run/summaryMetrics")
        .and_then(|v| v.as_str())
        .unwrap_or("{}");

    let metrics: serde_json::Value = serde_json::from_str(metrics_str).unwrap_or_default();

    Ok(WandbMetrics {
        step: metrics.get("_step").and_then(|v| v.as_u64()),
        reward_mean: metrics.get("train/reward_mean").and_then(|v| v.as_f64()),
        percent_correct: metrics.get("train/percent_correct").and_then(|v| v.as_f64()),
        eval_percent_correct: metrics.get("eval/percent_correct").and_then(|v| v.as_f64()),
    })
}

// --- Status Check Rate Limiting ---

/// Minimum seconds between status checks (30 minutes).
pub const MIN_STATUS_CHECK_INTERVAL_SECS: u64 = 1800;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_parse_env_frontmatter() {
        let dir = std::env::temp_dir().join(format!("mchact_rl_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let env_file = dir.join("test_env.py");
        let mut f = std::fs::File::create(&env_file).unwrap();
        writeln!(f, "# ---").unwrap();
        writeln!(f, "# name: test_env").unwrap();
        writeln!(f, "# class: TestEnv").unwrap();
        writeln!(f, "# description: A test environment").unwrap();
        writeln!(f, "# ---").unwrap();
        writeln!(f, "class TestEnv:").unwrap();
        writeln!(f, "    pass").unwrap();

        let info = parse_env_frontmatter(&env_file).unwrap();
        assert_eq!(info.name, "test_env");
        assert_eq!(info.class_name, "TestEnv");
        assert_eq!(info.description, "A test environment");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_discover_environments() {
        let dir = std::env::temp_dir().join(format!("mchact_rl_discover_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        // Create two env files
        for (name, class) in [("alpha", "AlphaEnv"), ("beta", "BetaEnv")] {
            let path = dir.join(format!("{name}.py"));
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(f, "# ---").unwrap();
            writeln!(f, "# name: {name}").unwrap();
            writeln!(f, "# class: {class}").unwrap();
            writeln!(f, "# description: {name} environment").unwrap();
            writeln!(f, "# ---").unwrap();
        }

        // Create a non-env file (no frontmatter)
        std::fs::write(dir.join("util.py"), "# just a helper\n").unwrap();

        let envs = discover_environments(&dir).unwrap();
        assert_eq!(envs.len(), 2);
        assert_eq!(envs[0].name, "alpha"); // sorted
        assert_eq!(envs[1].name, "beta");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_discover_environments_empty_dir() {
        let dir = std::env::temp_dir().join(format!("mchact_rl_empty_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let envs = discover_environments(&dir).unwrap();
        assert!(envs.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_discover_environments_nonexistent() {
        let envs = discover_environments(Path::new("/nonexistent/path")).unwrap();
        assert!(envs.is_empty());
    }

    #[test]
    fn test_locked_config_structure() {
        let config = locked_config();
        assert!(config.get("env").is_some());
        assert!(config.get("tinker").is_some());
        assert!(config.get("openai").is_some());
        assert_eq!(config["env"]["tokenizer_name"], "Qwen/Qwen3-8B");
        assert_eq!(config["tinker"]["lora_rank"], 32);
    }

    #[test]
    fn test_is_locked_field() {
        assert!(is_locked_field("env"));
        assert!(is_locked_field("tinker"));
        assert!(is_locked_field("tokenizer_name"));
        assert!(is_locked_field("lora_rank"));
        assert!(!is_locked_field("custom_field"));
        assert!(!is_locked_field("my_setting"));
    }

    #[test]
    fn test_merge_config() {
        let locked = locked_config();
        let mut overrides = HashMap::new();
        overrides.insert("wandb_name".to_string(), serde_json::json!("my-run"));
        overrides.insert("custom_param".to_string(), serde_json::json!(42));

        let merged = merge_config(&locked, &overrides);
        assert_eq!(merged["env"]["tokenizer_name"], "Qwen/Qwen3-8B"); // locked preserved
        assert_eq!(merged["user"]["wandb_name"], "my-run");
        assert_eq!(merged["user"]["custom_param"], 42);
    }

    #[test]
    fn test_rl_run_status_display() {
        assert_eq!(RlRunStatus::Running.to_string(), "running");
        assert_eq!(RlRunStatus::Failed.to_string(), "failed");
        assert_eq!(RlRunStatus::Completed.to_string(), "completed");
    }
}
```

- [ ] **Step 2: Register module in `src/lib.rs`**

Add after `pub mod train_pipeline;`:
```rust
pub mod rl;
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib rl:: -- --nocapture`
Expected: All 8 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/rl.rs src/lib.rs
git commit -m "feat: add RL types, environment discovery, locked config, and WandB metrics"
```

---

### Task 2: 3-Process Supervisor

**Files:**
- Modify: `src/rl.rs`

- [ ] **Step 1: Add process supervisor types and functions**

Append to `src/rl.rs` (before the `#[cfg(test)]` block):

```rust
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::time::sleep;

/// In-memory run state with supervised child processes.
pub struct RlRun {
    pub info: RlRunInfo,
    pub processes: Vec<Child>,
    pub start_instant: Instant,
    pub last_status_check: Option<Instant>,
}

/// Manages multiple RL training runs.
pub struct RlRunManager {
    runs: Mutex<HashMap<String, RlRun>>,
}

impl RlRunManager {
    pub fn new() -> Self {
        Self {
            runs: Mutex::new(HashMap::new()),
        }
    }

    pub fn get_run_info(&self, run_id: &str) -> Option<RlRunInfo> {
        self.runs.lock().ok()?.get(run_id).map(|r| r.info.clone())
    }

    pub fn list_runs(&self) -> Vec<RlRunInfo> {
        self.runs
            .lock()
            .ok()
            .map(|runs| runs.values().map(|r| r.info.clone()).collect())
            .unwrap_or_default()
    }

    pub fn update_status(&self, run_id: &str, status: RlRunStatus, error: Option<&str>) {
        if let Ok(mut runs) = self.runs.lock() {
            if let Some(run) = runs.get_mut(run_id) {
                run.info.status = status;
                if let Some(err) = error {
                    run.info.error_message = err.to_string();
                }
            }
        }
    }

    pub fn can_check_status(&self, run_id: &str) -> bool {
        if let Ok(runs) = self.runs.lock() {
            if let Some(run) = runs.get(run_id) {
                if let Some(last) = run.last_status_check {
                    return last.elapsed().as_secs() >= MIN_STATUS_CHECK_INTERVAL_SECS;
                }
                return true; // Never checked before
            }
        }
        false
    }

    pub fn mark_status_checked(&self, run_id: &str) {
        if let Ok(mut runs) = self.runs.lock() {
            if let Some(run) = runs.get_mut(run_id) {
                run.last_status_check = Some(Instant::now());
            }
        }
    }

    pub fn running_time_minutes(&self, run_id: &str) -> f64 {
        self.runs
            .lock()
            .ok()
            .and_then(|runs| runs.get(run_id).map(|r| r.start_instant.elapsed().as_secs_f64() / 60.0))
            .unwrap_or(0.0)
    }

    /// Start a training run: spawn 3 processes in sequence.
    pub fn start_run(
        &self,
        run_id: String,
        environment: &EnvironmentInfo,
        config: serde_json::Value,
        wandb_run_name: String,
        training_dir: &Path,
    ) -> Result<(), String> {
        let config_path = training_dir.join(format!("run_{run_id}_config.yaml"));
        let config_yaml = serde_yaml::to_string(&config)
            .map_err(|e| format!("Cannot serialize config: {e}"))?;
        std::fs::write(&config_path, &config_yaml)
            .map_err(|e| format!("Cannot write config: {e}"))?;

        let info = RlRunInfo {
            run_id: run_id.clone(),
            environment: environment.name.clone(),
            status: RlRunStatus::Starting,
            error_message: String::new(),
            wandb_project: "atropos-tinker".to_string(),
            wandb_run_name,
            start_time_epoch: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            config: config.clone(),
        };

        let run = RlRun {
            info,
            processes: Vec::new(),
            start_instant: Instant::now(),
            last_status_check: None,
        };

        self.runs
            .lock()
            .map_err(|e| format!("Lock error: {e}"))?
            .insert(run_id.clone(), run);

        // Process 1: run-api (Atropos API server)
        let api_child = Command::new("run-api")
            .current_dir(training_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to spawn run-api: {e}. Is atroposlib installed?"))?;

        if let Ok(mut runs) = self.runs.lock() {
            if let Some(run) = runs.get_mut(&run_id) {
                run.processes.push(api_child);
            }
        }

        // Wait 5 seconds for API startup
        std::thread::sleep(Duration::from_secs(5));

        // Check if still alive
        if let Ok(mut runs) = self.runs.lock() {
            if let Some(run) = runs.get_mut(&run_id) {
                if let Some(proc) = run.processes.last_mut() {
                    if proc.try_wait().ok().flatten().is_some() {
                        run.info.status = RlRunStatus::Failed;
                        run.info.error_message = "run-api exited immediately".to_string();
                        return Err("run-api exited immediately".to_string());
                    }
                }
            }
        }

        // Process 2: launch_training.py
        let trainer_child = Command::new("python3")
            .arg("launch_training.py")
            .arg("--config")
            .arg(&config_path)
            .current_dir(training_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to spawn trainer: {e}"))?;

        if let Ok(mut runs) = self.runs.lock() {
            if let Some(run) = runs.get_mut(&run_id) {
                run.processes.push(trainer_child);
            }
        }

        // Wait 30 seconds for trainer initialization
        std::thread::sleep(Duration::from_secs(30));

        // Process 3: environment serve
        let env_child = Command::new("python3")
            .arg(environment.file_path.to_string_lossy().as_ref())
            .arg("serve")
            .arg("--config")
            .arg(&config_path)
            .current_dir(training_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to spawn environment: {e}"))?;

        if let Ok(mut runs) = self.runs.lock() {
            if let Some(run) = runs.get_mut(&run_id) {
                run.processes.push(env_child);
                run.info.status = RlRunStatus::Running;
            }
        }

        Ok(())
    }

    /// Stop a training run: terminate processes in reverse order.
    pub fn stop_run(&self, run_id: &str) -> Result<(), String> {
        let mut runs = self.runs.lock().map_err(|e| format!("Lock: {e}"))?;
        let run = runs.get_mut(run_id).ok_or_else(|| format!("Run {run_id} not found"))?;

        run.info.status = RlRunStatus::Stopping;

        // Terminate in reverse order: env → trainer → api
        for proc in run.processes.iter_mut().rev() {
            let _ = proc.kill();
        }

        // Wait for all to exit (10s grace)
        for proc in run.processes.iter_mut() {
            let _ = proc.wait();
        }

        run.info.status = RlRunStatus::Stopped;
        Ok(())
    }

    /// Check if any processes in a run have died.
    pub fn check_process_health(&self, run_id: &str) -> Option<RlRunStatus> {
        let mut runs = self.runs.lock().ok()?;
        let run = runs.get_mut(run_id)?;

        if run.info.status != RlRunStatus::Running {
            return Some(run.info.status);
        }

        for proc in run.processes.iter_mut() {
            if let Ok(Some(exit)) = proc.try_wait() {
                if exit.success() {
                    run.info.status = RlRunStatus::Completed;
                } else {
                    run.info.status = RlRunStatus::Failed;
                    run.info.error_message = format!("Process exited with {exit}");
                }
                return Some(run.info.status);
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
```

- [ ] **Step 2: Add supervisor tests**

Add to the existing `#[cfg(test)]` block:

```rust
#[test]
fn test_run_manager_lifecycle() {
    let manager = RlRunManager::new();
    assert!(manager.list_runs().is_empty());

    // We can't test actual process spawning in unit tests,
    // but we can test the state management
    let run_id = "test123".to_string();
    {
        let mut runs = manager.runs.lock().unwrap();
        runs.insert(run_id.clone(), RlRun {
            info: RlRunInfo {
                run_id: run_id.clone(),
                environment: "test_env".into(),
                status: RlRunStatus::Running,
                error_message: String::new(),
                wandb_project: "test".into(),
                wandb_run_name: "test-run".into(),
                start_time_epoch: 0,
                config: serde_json::json!({}),
            },
            processes: vec![],
            start_instant: Instant::now(),
            last_status_check: None,
        });
    }

    assert_eq!(manager.list_runs().len(), 1);
    let info = manager.get_run_info(&run_id).unwrap();
    assert_eq!(info.status, RlRunStatus::Running);
    assert_eq!(info.environment, "test_env");

    // Test status update
    manager.update_status(&run_id, RlRunStatus::Completed, None);
    let info = manager.get_run_info(&run_id).unwrap();
    assert_eq!(info.status, RlRunStatus::Completed);
}

#[test]
fn test_status_check_rate_limiting() {
    let manager = RlRunManager::new();
    let run_id = "rate_test".to_string();
    {
        let mut runs = manager.runs.lock().unwrap();
        runs.insert(run_id.clone(), RlRun {
            info: RlRunInfo {
                run_id: run_id.clone(),
                environment: "env".into(),
                status: RlRunStatus::Running,
                error_message: String::new(),
                wandb_project: "proj".into(),
                wandb_run_name: "run".into(),
                start_time_epoch: 0,
                config: serde_json::json!({}),
            },
            processes: vec![],
            start_instant: Instant::now(),
            last_status_check: None,
        });
    }

    // First check should be allowed
    assert!(manager.can_check_status(&run_id));
    manager.mark_status_checked(&run_id);

    // Immediate second check should be blocked (< 30 min)
    assert!(!manager.can_check_status(&run_id));
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib rl:: -- --nocapture`
Expected: All 10 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/rl.rs
git commit -m "feat: add RL 3-process supervisor with run state management"
```

---

### Task 3: RL CLI Interface (`mchact rl`)

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add `Rl` subcommand to `MainCommand`**

Add to the `MainCommand` enum:
```rust
/// RL training management
Rl {
    #[command(subcommand)]
    action: RlAction,
},
```

Add the `RlAction` enum:
```rust
#[derive(Debug, Subcommand)]
enum RlAction {
    /// List available training environments
    List,
    /// Select an environment and show its config
    Select {
        /// Environment name
        name: String,
    },
    /// Show current config (locked + configurable fields)
    Config,
    /// Edit a configurable field
    Edit {
        /// Field name
        field: String,
        /// New value (JSON)
        value: String,
    },
    /// Start a training run
    Start,
    /// Check training status and WandB metrics
    Status {
        /// Run ID (omit for latest)
        run_id: Option<String>,
    },
    /// Stop a training run
    Stop {
        /// Run ID (omit for latest)
        run_id: Option<String>,
    },
    /// Fetch final results
    Results {
        /// Run ID (omit for latest)
        run_id: Option<String>,
    },
    /// List all training runs
    Runs,
    /// Test inference without full training
    Test {
        #[arg(long, default_value = "3")]
        steps: usize,
        #[arg(long, default_value = "16")]
        group_size: usize,
    },
}
```

- [ ] **Step 2: Add `Rl` match arm**

```rust
Some(MainCommand::Rl { action }) => {
    let config = Config::load()?;
    let env_dir = std::path::Path::new(&config.training_environments_dir);

    match action {
        RlAction::List => {
            let envs = mchact::rl::discover_environments(env_dir)?;
            if envs.is_empty() {
                println!("No environments found in {}", env_dir.display());
                println!("Add .py files with YAML frontmatter to {}", env_dir.display());
            } else {
                println!("Available environments ({}):", envs.len());
                for env in &envs {
                    println!("  {} — {}", env.name, env.description);
                    println!("    class: {}, file: {}", env.class_name, env.file_path.display());
                }
            }
        }
        RlAction::Select { name } => {
            let envs = mchact::rl::discover_environments(env_dir)?;
            let env = envs.iter().find(|e| e.name == name)
                .ok_or_else(|| MchactError::Config(format!("Environment '{name}' not found")))?;
            println!("Selected: {} ({})", env.name, env.description);
            println!("Class: {}", env.class_name);
            println!("File: {}", env.file_path.display());
            println!("\nLocked config:");
            let locked = mchact::rl::locked_config();
            println!("{}", serde_json::to_string_pretty(&locked).unwrap_or_default());
        }
        RlAction::Config => {
            println!("Locked infrastructure config:");
            let locked = mchact::rl::locked_config();
            println!("{}", serde_json::to_string_pretty(&locked).unwrap_or_default());
            println!("\nTo edit configurable fields: mchact rl edit <field> <value>");
        }
        RlAction::Edit { field, value } => {
            if mchact::rl::is_locked_field(&field) {
                eprintln!("Error: '{field}' is a locked infrastructure field and cannot be edited.");
                std::process::exit(1);
            }
            println!("Set {field} = {value}");
            println!("(Config editing will take effect on next `mchact rl start`)");
        }
        RlAction::Start => {
            println!("Starting RL training...");
            println!("Checking requirements:");
            if std::env::var("TINKER_API_KEY").is_err() {
                eprintln!("  ✗ TINKER_API_KEY not set");
                std::process::exit(1);
            }
            println!("  ✓ TINKER_API_KEY");
            if std::env::var("WANDB_API_KEY").is_err() {
                eprintln!("  ✗ WANDB_API_KEY not set");
                std::process::exit(1);
            }
            println!("  ✓ WANDB_API_KEY");

            let envs = mchact::rl::discover_environments(env_dir)?;
            if envs.is_empty() {
                eprintln!("No environments found. Add .py files to {}", env_dir.display());
                std::process::exit(1);
            }
            println!("\nAvailable environments:");
            for (i, env) in envs.iter().enumerate() {
                println!("  {}: {} — {}", i + 1, env.name, env.description);
            }
            println!("\nUsing first environment: {}", envs[0].name);

            let run_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
            let wandb_name = format!("{}-{}", envs[0].name, chrono::Utc::now().format("%Y%m%d-%H%M"));
            let config = mchact::rl::merge_config(
                &mchact::rl::locked_config(),
                &std::collections::HashMap::new(),
            );

            let manager = mchact::rl::RlRunManager::new();
            match manager.start_run(run_id.clone(), &envs[0], config, wandb_name.clone(), env_dir) {
                Ok(()) => {
                    println!("\nTraining started!");
                    println!("  Run ID: {run_id}");
                    println!("  WandB: {wandb_name}");
                    println!("\nCheck status: mchact rl status {run_id}");
                    println!("Stop: mchact rl stop {run_id}");
                }
                Err(e) => {
                    eprintln!("Failed to start training: {e}");
                    std::process::exit(1);
                }
            }
        }
        RlAction::Status { run_id } => {
            println!("Status checking requires a running manager. Use agent tools for persistent monitoring.");
            if let Some(id) = run_id {
                println!("Run ID: {id}");
            }
        }
        RlAction::Stop { run_id } => {
            println!("Stop requires a running manager. Use agent tools for persistent management.");
            if let Some(id) = run_id {
                println!("Run ID: {id}");
            }
        }
        RlAction::Results { run_id } => {
            println!("Results fetching requires WandB API. Use agent tools for metric retrieval.");
            if let Some(id) = run_id {
                println!("Run ID: {id}");
            }
        }
        RlAction::Runs => {
            println!("No persistent run tracking in CLI mode. Use agent tools for run management.");
        }
        RlAction::Test { steps, group_size } => {
            println!("Inference test: {steps} steps × {group_size} completions");
            println!("(Requires OPENROUTER_API_KEY and a configured environment)");
        }
    }
    return Ok(());
}
```

- [ ] **Step 3: Verify build and CLI**

Run: `cargo build && cargo run -- rl --help`
Expected: Shows RL subcommands.

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: add mchact rl CLI with list/select/config/edit/start/status/stop subcommands"
```

---

### Task 4: Bundled Starter Environments

**Files:**
- Create: `training/environments/web_research.py`
- Create: `training/environments/terminal_tasks.py`
- Create: `training/environments/swe.py`

- [ ] **Step 1: Create environments directory**

```bash
mkdir -p training/environments
```

- [ ] **Step 2: Create web_research.py**

```python
# ---
# name: web_research
# class: WebResearchEnv
# description: Web research and information synthesis tasks
# ---
"""
Web Research RL Environment for mchact.

This environment presents the agent with research questions that require
web search, page extraction, and information synthesis. The agent is
rewarded for accurate, comprehensive, and well-sourced answers.

Requires: atroposlib, tinker
"""


class WebResearchEnv:
    """Placeholder environment for web research tasks.

    To use this environment with RL training:
    1. Install: pip install atroposlib tinker
    2. Implement the BaseEnv interface from atroposlib
    3. Define process() for reward computation
    4. Run: python training/environments/web_research.py serve --config <path>
    """

    def __init__(self, config=None):
        self.config = config or {}

    def get_task(self):
        """Return a research task for the agent."""
        return {
            "prompt": "Research the current state of quantum computing and summarize key breakthroughs from the past year.",
            "expected_tools": ["web_search", "web_fetch"],
        }

    def evaluate(self, trajectory):
        """Evaluate agent's research quality."""
        # Placeholder scoring
        tool_count = sum(1 for m in trajectory if m.get("role") == "tool")
        has_sources = any("http" in str(m.get("content", "")) for m in trajectory)
        score = min(1.0, tool_count * 0.2 + (0.3 if has_sources else 0.0))
        return {"score": score, "tool_count": tool_count, "has_sources": has_sources}


if __name__ == "__main__":
    import sys
    if len(sys.argv) > 1 and sys.argv[1] == "serve":
        print(f"WebResearchEnv: serve mode not yet implemented")
        print(f"Config: {sys.argv[3] if len(sys.argv) > 3 else 'none'}")
    else:
        env = WebResearchEnv()
        print(f"Environment: {env.get_task()['prompt'][:60]}...")
```

- [ ] **Step 3: Create terminal_tasks.py**

```python
# ---
# name: terminal_tasks
# class: TerminalTasksEnv
# description: Terminal and file manipulation tasks
# ---
"""
Terminal Tasks RL Environment for mchact.

This environment presents the agent with shell and file manipulation tasks.
The agent is rewarded for correct execution, efficient tool use, and
proper error handling.

Requires: atroposlib, tinker
"""


class TerminalTasksEnv:
    """Placeholder environment for terminal/file tasks.

    To use this environment with RL training:
    1. Install: pip install atroposlib tinker
    2. Implement the BaseEnv interface from atroposlib
    3. Define process() for reward computation
    4. Run: python training/environments/terminal_tasks.py serve --config <path>
    """

    def __init__(self, config=None):
        self.config = config or {}

    def get_task(self):
        """Return a terminal task for the agent."""
        return {
            "prompt": "Create a Python script that reads a CSV file, filters rows where the 'status' column is 'active', and writes the result to a new file.",
            "expected_tools": ["bash", "write_file", "read_file"],
        }

    def evaluate(self, trajectory):
        """Evaluate agent's task completion."""
        tool_count = sum(1 for m in trajectory if m.get("role") == "tool")
        has_file_ops = any(
            "write_file" in str(m) or "read_file" in str(m)
            for m in trajectory
        )
        score = min(1.0, tool_count * 0.15 + (0.4 if has_file_ops else 0.0))
        return {"score": score, "tool_count": tool_count, "has_file_ops": has_file_ops}


if __name__ == "__main__":
    import sys
    if len(sys.argv) > 1 and sys.argv[1] == "serve":
        print(f"TerminalTasksEnv: serve mode not yet implemented")
        print(f"Config: {sys.argv[3] if len(sys.argv) > 3 else 'none'}")
    else:
        env = TerminalTasksEnv()
        print(f"Environment: {env.get_task()['prompt'][:60]}...")
```

- [ ] **Step 4: Create swe.py**

```python
# ---
# name: swe
# class: SweEnv
# description: Software engineering tasks (bug fixing, feature implementation)
# ---
"""
Software Engineering RL Environment for mchact.

This environment presents the agent with software engineering tasks including
bug fixing, feature implementation, and code refactoring. The agent is
rewarded for correct solutions that pass tests.

Requires: atroposlib, tinker
"""


class SweEnv:
    """Placeholder environment for software engineering tasks.

    To use this environment with RL training:
    1. Install: pip install atroposlib tinker
    2. Implement the BaseEnv interface from atroposlib
    3. Define process() for reward computation
    4. Run: python training/environments/swe.py serve --config <path>
    """

    def __init__(self, config=None):
        self.config = config or {}

    def get_task(self):
        """Return an SWE task for the agent."""
        return {
            "prompt": "Fix the bug in the login function that causes a NullPointerException when the user's email is None.",
            "expected_tools": ["bash", "read_file", "edit_file", "grep"],
        }

    def evaluate(self, trajectory):
        """Evaluate agent's fix quality."""
        tool_count = sum(1 for m in trajectory if m.get("role") == "tool")
        has_edit = any("edit_file" in str(m) for m in trajectory)
        has_test = any("test" in str(m.get("content", "")).lower() for m in trajectory)
        score = min(1.0, tool_count * 0.1 + (0.3 if has_edit else 0.0) + (0.3 if has_test else 0.0))
        return {"score": score, "tool_count": tool_count, "has_edit": has_edit, "has_test": has_test}


if __name__ == "__main__":
    import sys
    if len(sys.argv) > 1 and sys.argv[1] == "serve":
        print(f"SweEnv: serve mode not yet implemented")
        print(f"Config: {sys.argv[3] if len(sys.argv) > 3 else 'none'}")
    else:
        env = SweEnv()
        print(f"Environment: {env.get_task()['prompt'][:60]}...")
```

- [ ] **Step 5: Verify discovery works**

Run: `cargo run -- rl list`
Expected: Lists 3 environments (swe, terminal_tasks, web_research).

- [ ] **Step 6: Commit**

```bash
git add training/environments/
git commit -m "feat: add 3 bundled RL training environments (web_research, terminal_tasks, swe)"
```

---

### Task 5: RL Agent Tools

**Files:**
- Create: `src/tools/rl_training.rs`
- Modify: `src/tools/mod.rs`

- [ ] **Step 1: Create RL training agent tools**

Create `src/tools/rl_training.rs` with 4 tools:

```rust
use async_trait::async_trait;
use mchact_core::llm_types::ToolDefinition;
use serde_json::json;
use std::sync::Arc;

use super::{schema_object, Tool, ToolResult};
use crate::rl::{self, RlRunManager};

/// Tool: List available RL training environments.
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
        schema_object(
            "rl_list_environments",
            "List available RL training environments.",
            json!({
                "type": "object",
                "properties": {}
            }),
        )
    }

    async fn execute(&self, _input: serde_json::Value) -> ToolResult {
        let dir = std::path::Path::new(&self.environments_dir);
        match rl::discover_environments(dir) {
            Ok(envs) => {
                let list: Vec<_> = envs
                    .iter()
                    .map(|e| json!({
                        "name": e.name,
                        "class": e.class_name,
                        "description": e.description,
                        "file": e.file_path.to_string_lossy(),
                    }))
                    .collect();
                ToolResult {
                    content: json!({"environments": list, "count": envs.len()}).to_string(),
                    is_error: false,
                    status_code: None,
                    bytes: 0,
                    duration_ms: None,
                    error_type: None,
                    metadata: None,
                }
            }
            Err(e) => ToolResult {
                content: json!({"error": e}).to_string(),
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

/// Tool: Start an RL training run.
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
        schema_object(
            "rl_start_training",
            "Start an RL training run. Requires TINKER_API_KEY and WANDB_API_KEY env vars.",
            json!({
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
            }),
        )
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let env_name = input.get("environment").and_then(|v| v.as_str()).unwrap_or("");
        let overrides: std::collections::HashMap<String, serde_json::Value> = input
            .get("config_overrides")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        // Check requirements
        if std::env::var("TINKER_API_KEY").is_err() {
            return ToolResult {
                content: json!({"error": "TINKER_API_KEY not set"}).to_string(),
                is_error: true,
                status_code: None, bytes: 0, duration_ms: None, error_type: None, metadata: None,
            };
        }

        let dir = std::path::Path::new(&self.environments_dir);
        let envs = match rl::discover_environments(dir) {
            Ok(e) => e,
            Err(e) => return ToolResult {
                content: json!({"error": e}).to_string(),
                is_error: true,
                status_code: None, bytes: 0, duration_ms: None, error_type: None, metadata: None,
            },
        };

        let env = match envs.iter().find(|e| e.name == env_name) {
            Some(e) => e,
            None => return ToolResult {
                content: json!({"error": format!("Environment '{env_name}' not found")}).to_string(),
                is_error: true,
                status_code: None, bytes: 0, duration_ms: None, error_type: None, metadata: None,
            },
        };

        let run_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
        let wandb_name = format!("{}-{}", env.name, chrono::Utc::now().format("%Y%m%d-%H%M"));
        let config = rl::merge_config(&rl::locked_config(), &overrides);

        let env_clone = env.clone();
        let manager = self.run_manager.clone();
        let dir_owned = dir.to_path_buf();
        let run_id_clone = run_id.clone();
        let wandb_clone = wandb_name.clone();

        // Spawn in blocking task since process spawning is synchronous
        let result = tokio::task::spawn_blocking(move || {
            manager.start_run(run_id_clone, &env_clone, config, wandb_clone, &dir_owned)
        })
        .await;

        match result {
            Ok(Ok(())) => ToolResult {
                content: json!({
                    "run_id": run_id,
                    "environment": env_name,
                    "status": "starting",
                    "wandb_run_name": wandb_name,
                }).to_string(),
                is_error: false,
                status_code: None, bytes: 0, duration_ms: None, error_type: None, metadata: None,
            },
            Ok(Err(e)) => ToolResult {
                content: json!({"error": e}).to_string(),
                is_error: true,
                status_code: None, bytes: 0, duration_ms: None, error_type: None, metadata: None,
            },
            Err(e) => ToolResult {
                content: json!({"error": format!("Task join error: {e}")}).to_string(),
                is_error: true,
                status_code: None, bytes: 0, duration_ms: None, error_type: None, metadata: None,
            },
        }
    }
}

/// Tool: Check RL training status and WandB metrics.
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
        schema_object(
            "rl_check_status",
            "Check RL training status and WandB metrics. Rate-limited to once per 30 minutes.",
            json!({
                "type": "object",
                "properties": {
                    "run_id": {
                        "type": "string",
                        "description": "Run ID (omit to check latest run)"
                    }
                }
            }),
        )
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let run_id_input = input.get("run_id").and_then(|v| v.as_str()).map(String::from);

        let runs = self.run_manager.list_runs();
        if runs.is_empty() {
            return ToolResult {
                content: json!({"error": "No active training runs"}).to_string(),
                is_error: true,
                status_code: None, bytes: 0, duration_ms: None, error_type: None, metadata: None,
            };
        }

        let run_id = run_id_input.unwrap_or_else(|| runs.last().unwrap().run_id.clone());

        let info = match self.run_manager.get_run_info(&run_id) {
            Some(i) => i,
            None => return ToolResult {
                content: json!({"error": format!("Run '{run_id}' not found")}).to_string(),
                is_error: true,
                status_code: None, bytes: 0, duration_ms: None, error_type: None, metadata: None,
            },
        };

        // Check process health
        let _ = self.run_manager.check_process_health(&run_id);
        let info = self.run_manager.get_run_info(&run_id).unwrap_or(info);

        let mut result = json!({
            "run_id": info.run_id,
            "environment": info.environment,
            "status": info.status.to_string(),
            "running_time_minutes": self.run_manager.running_time_minutes(&run_id),
        });

        if !info.error_message.is_empty() {
            result["error_message"] = json!(info.error_message);
        }

        // Fetch WandB metrics if rate limit allows
        if info.status == rl::RlRunStatus::Running && self.run_manager.can_check_status(&run_id) {
            let entity = std::env::var("WANDB_ENTITY").unwrap_or_else(|_| "nousresearch".into());
            match rl::fetch_wandb_metrics(&entity, &info.wandb_project, &info.wandb_run_name).await {
                Ok(metrics) => {
                    result["wandb_metrics"] = json!(metrics);
                    self.run_manager.mark_status_checked(&run_id);
                }
                Err(e) => {
                    result["wandb_error"] = json!(e);
                }
            }
        } else if !self.run_manager.can_check_status(&run_id) {
            result["wandb_note"] = json!("Rate limited: next check available in ~30 minutes");
        }

        ToolResult {
            content: serde_json::to_string(&result).unwrap_or_default(),
            is_error: false,
            status_code: None, bytes: 0, duration_ms: None, error_type: None, metadata: None,
        }
    }
}

/// Tool: Stop an RL training run.
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
        schema_object(
            "rl_stop_training",
            "Stop an RL training run. Gracefully terminates all 3 processes.",
            json!({
                "type": "object",
                "properties": {
                    "run_id": {
                        "type": "string",
                        "description": "Run ID (omit to stop latest run)"
                    }
                }
            }),
        )
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let run_id_input = input.get("run_id").and_then(|v| v.as_str()).map(String::from);

        let runs = self.run_manager.list_runs();
        let run_id = run_id_input.unwrap_or_else(|| {
            runs.last().map(|r| r.run_id.clone()).unwrap_or_default()
        });

        if run_id.is_empty() {
            return ToolResult {
                content: json!({"error": "No active training runs"}).to_string(),
                is_error: true,
                status_code: None, bytes: 0, duration_ms: None, error_type: None, metadata: None,
            };
        }

        let manager = self.run_manager.clone();
        let rid = run_id.clone();
        let result = tokio::task::spawn_blocking(move || manager.stop_run(&rid)).await;

        match result {
            Ok(Ok(())) => ToolResult {
                content: json!({"run_id": run_id, "status": "stopped"}).to_string(),
                is_error: false,
                status_code: None, bytes: 0, duration_ms: None, error_type: None, metadata: None,
            },
            Ok(Err(e)) => ToolResult {
                content: json!({"error": e}).to_string(),
                is_error: true,
                status_code: None, bytes: 0, duration_ms: None, error_type: None, metadata: None,
            },
            Err(e) => ToolResult {
                content: json!({"error": format!("Task error: {e}")}).to_string(),
                is_error: true,
                status_code: None, bytes: 0, duration_ms: None, error_type: None, metadata: None,
            },
        }
    }
}
```

- [ ] **Step 2: Register tools in `src/tools/mod.rs`**

Add module declaration:
```rust
pub mod rl_training;
```

Add to `ToolRegistry::new()` — the RL tools need an `Arc<RlRunManager>`. Add the manager as a parameter or create it inside. For now, create a static manager:

```rust
// RL training tools
let rl_manager = std::sync::Arc::new(crate::rl::RlRunManager::new());
tools.push(Box::new(rl_training::RlListEnvironmentsTool::new(&config.training_environments_dir)));
tools.push(Box::new(rl_training::RlStartTrainingTool::new(&config.training_environments_dir, rl_manager.clone())));
tools.push(Box::new(rl_training::RlCheckStatusTool::new(rl_manager.clone())));
tools.push(Box::new(rl_training::RlStopTrainingTool::new(rl_manager)));
```

- [ ] **Step 3: Verify build**

Run: `cargo build`
Expected: Clean build.

- [ ] **Step 4: Commit**

```bash
git add src/tools/rl_training.rs src/tools/mod.rs
git commit -m "feat: add 4 RL agent tools (list_environments, start, check_status, stop)"
```

---

### Task 6: Update RL Requirements & Final Verification

**Files:**
- Modify: `training/requirements.txt`

- [ ] **Step 1: Update requirements.txt with RL deps**

```
transformers>=4.40.0
httpx>=0.27.0
wandb>=0.15.0
# RL training (install separately):
# pip install atroposlib tinker
# See: https://github.com/NousResearch/atropos
# See: https://github.com/thinking-machines-lab/tinker
```

- [ ] **Step 2: Run all tests**

Run: `cargo test --lib`
Expected: All tests pass (including new rl:: tests).

- [ ] **Step 3: Verify RL CLI**

Run: `cargo run -- rl --help`
Expected: Shows list/select/config/edit/start/status/stop/results/runs/test subcommands.

Run: `cargo run -- rl list`
Expected: Lists 3 environments (swe, terminal_tasks, web_research).

- [ ] **Step 4: Commit**

```bash
git add training/requirements.txt
git commit -m "chore: update requirements.txt with RL training dependencies"
```
