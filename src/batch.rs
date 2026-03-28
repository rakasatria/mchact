use std::collections::{HashMap, HashSet};
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::batch_worker::{BatchPrompt, ReasoningStats, ToolStat};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Checkpoint
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
pub struct Checkpoint {
    pub run_name: String,
    pub completed_prompts: Vec<u64>,
    pub batch_stats: HashMap<String, BatchStat>,
    pub last_updated: String,
}

#[derive(Serialize, Deserialize)]
pub struct BatchStat {
    pub processed: u64,
    pub skipped: u64,
    pub failed: u64,
}

// ---------------------------------------------------------------------------
// Run statistics
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
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

#[derive(Serialize, Deserialize)]
pub struct ToolStatWithRates {
    pub count: u64,
    pub success: u64,
    pub failure: u64,
    pub success_rate: f64,
    pub failure_rate: f64,
}

// ---------------------------------------------------------------------------
// Combine result
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct CombineResult {
    pub total: u64,
    pub valid: u64,
    pub filtered: u64,
}

// ---------------------------------------------------------------------------
// Dataset loading
// ---------------------------------------------------------------------------

/// Read a JSONL dataset file line by line.
///
/// Each line must contain a "prompt" field. Optional "toolsets" and "image"
/// fields are supported. Lines that cannot be parsed are skipped with a
/// warning. The result is truncated to `max_samples` if specified.
pub fn load_dataset(
    path: &Path,
    max_samples: Option<usize>,
) -> Result<Vec<BatchPrompt>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read dataset '{}': {e}", path.display()))?;

    let mut prompts: Vec<BatchPrompt> = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let value: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("  [dataset] line {}: parse error: {e}", line_num + 1);
                continue;
            }
        };

        let prompt = match value.get("prompt").and_then(|v| v.as_str()) {
            Some(p) => p.to_owned(),
            None => {
                eprintln!(
                    "  [dataset] line {}: missing 'prompt' field, skipping",
                    line_num + 1
                );
                continue;
            }
        };

        let toolsets = value
            .get("toolsets")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|s| s.as_str().map(|s| s.to_owned()))
                    .collect::<Vec<_>>()
            });

        let image = value
            .get("image")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned());

        let prompt_index = prompts.len() as u64;
        prompts.push(BatchPrompt {
            prompt_index,
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

// ---------------------------------------------------------------------------
// Batch splitting
// ---------------------------------------------------------------------------

/// Split a flat list of prompts into chunks of at most `batch_size`.
pub fn split_batches(prompts: Vec<BatchPrompt>, batch_size: usize) -> Vec<Vec<BatchPrompt>> {
    if batch_size == 0 {
        return vec![prompts];
    }
    prompts
        .chunks(batch_size)
        .map(|chunk| chunk.to_vec())
        .collect()
}

// ---------------------------------------------------------------------------
// Checkpoint persistence
// ---------------------------------------------------------------------------

fn checkpoint_path(output_dir: &Path) -> PathBuf {
    output_dir.join("checkpoint.json")
}

/// Load an existing checkpoint from `output_dir/checkpoint.json`, if present.
pub fn load_checkpoint(output_dir: &Path) -> Option<Checkpoint> {
    let path = checkpoint_path(output_dir);
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content)
        .map_err(|e| eprintln!("  [checkpoint] parse error: {e}"))
        .ok()
}

/// Load a checkpoint via ObjectStorage.
pub fn load_checkpoint_via_storage(
    storage: &dyn mchact_storage_backend::ObjectStorage,
    run_id: &str,
) -> Option<Checkpoint> {
    let key = format!("batch/{run_id}/checkpoint.json");
    let handle = tokio::runtime::Handle::current();
    let bytes = tokio::task::block_in_place(|| {
        handle.block_on(async { storage.get(&key).await })
    })
    .ok()?;
    serde_json::from_slice(&bytes)
        .map_err(|e| eprintln!("  [checkpoint] parse error: {e}"))
        .ok()
}

/// Persist a checkpoint to `output_dir/checkpoint.json`.
pub fn save_checkpoint(output_dir: &Path, checkpoint: &Checkpoint) -> Result<(), String> {
    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("failed to create output dir '{}': {e}", output_dir.display()))?;

    let path = checkpoint_path(output_dir);
    let json = serde_json::to_string_pretty(checkpoint)
        .map_err(|e| format!("failed to serialize checkpoint: {e}"))?;

    std::fs::write(&path, &json)
        .map_err(|e| format!("failed to write checkpoint '{}': {e}", path.display()))
}

/// Persist a checkpoint via ObjectStorage.
pub fn save_checkpoint_via_storage(
    storage: &dyn mchact_storage_backend::ObjectStorage,
    run_id: &str,
    checkpoint: &Checkpoint,
) -> Result<(), String> {
    let key = format!("batch/{run_id}/checkpoint.json");
    let json = serde_json::to_string_pretty(checkpoint)
        .map_err(|e| format!("failed to serialize checkpoint: {e}"))?;
    let handle = tokio::runtime::Handle::current();
    tokio::task::block_in_place(|| {
        handle.block_on(async { storage.put(&key, json.into_bytes()).await })
    })
    .map_err(|e| format!("failed to write checkpoint '{}': {e}", key))
}

// ---------------------------------------------------------------------------
// Resume helpers
// ---------------------------------------------------------------------------

/// Scan all `batch_*.jsonl` files in `output_dir` and collect the user prompt
/// text from every entry. This is used for content-based resume filtering.
pub fn find_completed_prompts(output_dir: &Path) -> HashSet<String> {
    let mut completed: HashSet<String> = HashSet::new();

    let pattern = output_dir.join("batch_*.jsonl");
    let pattern_str = pattern.to_string_lossy();

    let paths = match glob::glob(&pattern_str) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("  [resume] glob error: {e}");
            return completed;
        }
    };

    for entry in paths.flatten() {
        let content = match std::fs::read_to_string(&entry) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("  [resume] read '{}': {e}", entry.display());
                continue;
            }
        };

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
                // Look for user prompt in messages array
                if let Some(messages) = value.get("messages").and_then(|v| v.as_array()) {
                    for msg in messages {
                        let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");
                        if role == "user" {
                            if let Some(content_str) =
                                msg.get("content").and_then(|v| v.as_str())
                            {
                                completed.insert(content_str.to_owned());
                            }
                        }
                    }
                }
            }
        }
    }

    completed
}

/// Remove prompts whose text appears in the set of already-completed prompts.
pub fn filter_completed(
    prompts: Vec<BatchPrompt>,
    completed: &HashSet<String>,
) -> Vec<BatchPrompt> {
    prompts
        .into_iter()
        .filter(|p| !completed.contains(&p.prompt))
        .collect()
}

// ---------------------------------------------------------------------------
// Combining batches
// ---------------------------------------------------------------------------

/// Scan `batch_*.jsonl` files, validate entries, and write `trajectories.jsonl`.
///
/// Entries that reference tool names not present in `all_tool_names` are
/// considered hallucinated and filtered out.
pub fn combine_batches(
    output_dir: &Path,
    all_tool_names: &[String],
) -> Result<CombineResult, String> {
    let pattern = output_dir.join("batch_*.jsonl");
    let pattern_str = pattern.to_string_lossy();

    let paths = match glob::glob(&pattern_str) {
        Ok(p) => p,
        Err(e) => return Err(format!("glob error: {e}")),
    };

    let known_tools: HashSet<&str> = all_tool_names.iter().map(|s| s.as_str()).collect();

    let trajectories_path = output_dir.join("trajectories.jsonl");
    let mut result = CombineResult::default();
    let mut valid_lines: Vec<String> = Vec::new();

    for entry in paths.flatten() {
        let content = std::fs::read_to_string(&entry)
            .map_err(|e| format!("read '{}': {e}", entry.display()))?;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            result.total += 1;

            let value: serde_json::Value = match serde_json::from_str(trimmed) {
                Ok(v) => v,
                Err(_) => {
                    result.filtered += 1;
                    continue;
                }
            };

            // Validate tool_stats keys against known tools
            let has_hallucinated_tool = value
                .get("tool_stats")
                .and_then(|v| v.as_object())
                .map(|obj| {
                    obj.keys()
                        .any(|k| !k.is_empty() && !known_tools.contains(k.as_str()))
                })
                .unwrap_or(false);

            if has_hallucinated_tool {
                result.filtered += 1;
                continue;
            }

            result.valid += 1;
            valid_lines.push(trimmed.to_owned());
        }
    }

    let output = valid_lines.join("\n");
    let output = if output.is_empty() {
        output
    } else {
        format!("{output}\n")
    };

    std::fs::write(&trajectories_path, &output)
        .map_err(|e| format!("write '{}': {e}", trajectories_path.display()))?;

    Ok(result)
}

/// Combine batch files and write trajectories via ObjectStorage.
pub fn combine_batches_via_storage(
    storage: &dyn mchact_storage_backend::ObjectStorage,
    output_dir: &Path,
    run_id: &str,
    all_tool_names: &[String],
) -> Result<CombineResult, String> {
    let result = combine_batches(output_dir, all_tool_names)?;
    let trajectories_path = output_dir.join("trajectories.jsonl");
    if let Ok(content) = std::fs::read_to_string(&trajectories_path) {
        let key = format!("batch/{run_id}/trajectories.jsonl");
        let handle = tokio::runtime::Handle::current();
        let _ = tokio::task::block_in_place(|| {
            handle.block_on(async { storage.put(&key, content.into_bytes()).await })
        });
    }
    Ok(result)
}

// ---------------------------------------------------------------------------
// Statistics aggregation
// ---------------------------------------------------------------------------

/// Build per-tool statistics with success/failure rates from a list of entries.
///
/// Rates are rounded to 2 decimal places.
pub fn aggregate_tool_stats(
    entries: &[serde_json::Value],
) -> HashMap<String, ToolStatWithRates> {
    let mut raw: HashMap<String, ToolStat> = HashMap::new();

    for entry in entries {
        if let Some(tool_stats) = entry.get("tool_stats").and_then(|v| v.as_object()) {
            for (tool_name, stat_val) in tool_stats {
                let stat: ToolStat = match serde_json::from_value(stat_val.clone()) {
                    Ok(s) => s,
                    Err(_) => continue,
                };

                let entry = raw.entry(tool_name.clone()).or_insert_with(ToolStat::zero);
                entry.count += stat.count;
                entry.success += stat.success;
                entry.failure += stat.failure;
            }
        }
    }

    raw.into_iter()
        .map(|(name, stat)| {
            let success_rate = if stat.count > 0 {
                let rate = stat.success as f64 / stat.count as f64;
                (rate * 100.0).round() / 100.0
            } else {
                0.0
            };

            let failure_rate = if stat.count > 0 {
                let rate = stat.failure as f64 / stat.count as f64;
                (rate * 100.0).round() / 100.0
            } else {
                0.0
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

// ---------------------------------------------------------------------------
// Worker process spawning
// ---------------------------------------------------------------------------

/// Spawn worker sub-processes to process batches in parallel.
///
/// Each batch is written as a `batch_{i}.input.jsonl` file, then the current
/// executable is re-invoked as `worker --batch-file <path> ...` to process it.
/// Workers are launched in chunks of `max_concurrent`. After each chunk
/// completes the output file is renamed from `batch_{i}.input.jsonl.out.jsonl`
/// to `batch_{i}.jsonl`.
///
/// Returns the list of final output `batch_{i}.jsonl` paths.
#[allow(clippy::too_many_arguments)]
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
    let exe = std::env::current_exe()
        .map_err(|e| format!("failed to get current executable path: {e}"))?;

    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("failed to create output dir '{}': {e}", output_dir.display()))?;

    // Write all batch input files
    let mut input_paths: Vec<PathBuf> = Vec::with_capacity(batches.len());
    for (i, batch) in batches.iter().enumerate() {
        let input_path = output_dir.join(format!("batch_{i}.input.jsonl"));
        let file = std::fs::File::create(&input_path)
            .map_err(|e| format!("failed to create batch file '{}': {e}", input_path.display()))?;
        let mut writer = std::io::BufWriter::new(file);
        for prompt in batch {
            let line = serde_json::to_string(prompt)
                .map_err(|e| format!("failed to serialize prompt: {e}"))?;
            writeln!(writer, "{line}")
                .map_err(|e| format!("failed to write to batch file: {e}"))?;
        }
        writer
            .flush()
            .map_err(|e| format!("failed to flush batch file: {e}"))?;
        input_paths.push(input_path);
    }

    let chunk_size = if max_concurrent == 0 { 1 } else { max_concurrent };
    let mut output_paths: Vec<PathBuf> = Vec::with_capacity(batches.len());

    // Process in chunks
    for (chunk_start, chunk) in input_paths.chunks(chunk_size).enumerate() {
        let mut children: Vec<std::process::Child> = Vec::with_capacity(chunk.len());

        for input_path in chunk {
            let mut cmd = Command::new(&exe);
            if let Some(cfg) = config_path {
                cmd.arg("--config").arg(cfg);
            }
            cmd.arg("worker")
                .arg("--batch-file")
                .arg(input_path)
                .arg("--distribution")
                .arg(distribution)
                .arg("--max-iterations")
                .arg(max_iterations.to_string());
            if let Some(m) = model {
                cmd.arg("--model").arg(m);
            }
            if let Some(df) = distributions_file {
                cmd.arg("--distributions-file").arg(df);
            }

            let child = cmd
                .spawn()
                .map_err(|e| format!("failed to spawn worker for '{}': {e}", input_path.display()))?;
            children.push(child);
        }

        // Wait for all children in this chunk
        for (child_idx, mut child) in children.into_iter().enumerate() {
            let global_idx = chunk_start * chunk_size + child_idx;
            let status = child
                .wait()
                .map_err(|e| format!("failed to wait on worker {global_idx}: {e}"))?;
            if !status.success() {
                return Err(format!(
                    "worker {global_idx} exited with non-zero status: {:?}",
                    status.code()
                ));
            }
        }

        // Rename outputs for this chunk
        for (chunk_idx, input_path) in chunk.iter().enumerate() {
            let global_idx = chunk_start * chunk_size + chunk_idx;
            let raw_out = {
                let mut p = input_path.as_os_str().to_owned();
                p.push(".out.jsonl");
                PathBuf::from(p)
            };
            let final_out = output_dir.join(format!("batch_{global_idx}.jsonl"));
            std::fs::rename(&raw_out, &final_out).map_err(|e| {
                format!(
                    "failed to rename '{}' -> '{}': {e}",
                    raw_out.display(),
                    final_out.display()
                )
            })?;
            output_paths.push(final_out);
        }
    }

    Ok(output_paths)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // -----------------------------------------------------------------------
    // test_split_batches
    // -----------------------------------------------------------------------

    #[test]
    fn test_split_batches_even_division() {
        let prompts: Vec<BatchPrompt> = (0..6)
            .map(|i| BatchPrompt {
                prompt_index: i,
                prompt: format!("prompt {i}"),
                toolsets: None,
                image: None,
            })
            .collect();

        let batches = split_batches(prompts, 2);
        assert_eq!(batches.len(), 3);
        for batch in &batches {
            assert_eq!(batch.len(), 2);
        }
    }

    #[test]
    fn test_split_batches_remainder() {
        let prompts: Vec<BatchPrompt> = (0..5)
            .map(|i| BatchPrompt {
                prompt_index: i,
                prompt: format!("prompt {i}"),
                toolsets: None,
                image: None,
            })
            .collect();

        let batches = split_batches(prompts, 2);
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0].len(), 2);
        assert_eq!(batches[1].len(), 2);
        assert_eq!(batches[2].len(), 1);
    }

    #[test]
    fn test_split_batches_larger_than_input() {
        let prompts: Vec<BatchPrompt> = (0..3)
            .map(|i| BatchPrompt {
                prompt_index: i,
                prompt: format!("prompt {i}"),
                toolsets: None,
                image: None,
            })
            .collect();

        let batches = split_batches(prompts, 10);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].len(), 3);
    }

    #[test]
    fn test_split_batches_empty_input() {
        let batches = split_batches(vec![], 5);
        assert!(batches.is_empty());
    }

    // -----------------------------------------------------------------------
    // test_filter_completed
    // -----------------------------------------------------------------------

    #[test]
    fn test_filter_completed_removes_matching_prompts() {
        let prompts: Vec<BatchPrompt> = vec![
            BatchPrompt {
                prompt_index: 0,
                prompt: "already done".to_owned(),
                toolsets: None,
                image: None,
            },
            BatchPrompt {
                prompt_index: 1,
                prompt: "not done yet".to_owned(),
                toolsets: None,
                image: None,
            },
            BatchPrompt {
                prompt_index: 2,
                prompt: "also done".to_owned(),
                toolsets: None,
                image: None,
            },
        ];

        let mut completed = HashSet::new();
        completed.insert("already done".to_owned());
        completed.insert("also done".to_owned());

        let remaining = filter_completed(prompts, &completed);
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].prompt, "not done yet");
    }

    #[test]
    fn test_filter_completed_empty_completed_set() {
        let prompts: Vec<BatchPrompt> = (0..3)
            .map(|i| BatchPrompt {
                prompt_index: i,
                prompt: format!("prompt {i}"),
                toolsets: None,
                image: None,
            })
            .collect();

        let completed: HashSet<String> = HashSet::new();
        let remaining = filter_completed(prompts, &completed);
        assert_eq!(remaining.len(), 3);
    }

    #[test]
    fn test_filter_completed_all_completed() {
        let prompts: Vec<BatchPrompt> = vec![
            BatchPrompt {
                prompt_index: 0,
                prompt: "a".to_owned(),
                toolsets: None,
                image: None,
            },
            BatchPrompt {
                prompt_index: 1,
                prompt: "b".to_owned(),
                toolsets: None,
                image: None,
            },
        ];

        let mut completed = HashSet::new();
        completed.insert("a".to_owned());
        completed.insert("b".to_owned());

        let remaining = filter_completed(prompts, &completed);
        assert!(remaining.is_empty());
    }

    // -----------------------------------------------------------------------
    // test_aggregate_tool_stats
    // -----------------------------------------------------------------------

    #[test]
    fn test_aggregate_tool_stats_basic() {
        let entries = vec![
            json!({
                "tool_stats": {
                    "read_file": { "count": 4, "success": 3, "failure": 1 },
                    "write_file": { "count": 2, "success": 2, "failure": 0 }
                }
            }),
            json!({
                "tool_stats": {
                    "read_file": { "count": 2, "success": 1, "failure": 1 }
                }
            }),
        ];

        let stats = aggregate_tool_stats(&entries);

        let read = stats.get("read_file").expect("read_file present");
        assert_eq!(read.count, 6);
        assert_eq!(read.success, 4);
        assert_eq!(read.failure, 2);
        // success_rate = 4/6 ≈ 0.67
        assert!((read.success_rate - 0.67).abs() < 0.01);

        let write = stats.get("write_file").expect("write_file present");
        assert_eq!(write.count, 2);
        assert_eq!(write.success, 2);
        assert_eq!(write.failure, 0);
        assert_eq!(write.success_rate, 1.0);
        assert_eq!(write.failure_rate, 0.0);
    }

    #[test]
    fn test_aggregate_tool_stats_zero_count() {
        let entries = vec![json!({
            "tool_stats": {
                "empty_tool": { "count": 0, "success": 0, "failure": 0 }
            }
        })];

        let stats = aggregate_tool_stats(&entries);
        let tool = stats.get("empty_tool").expect("empty_tool present");
        assert_eq!(tool.success_rate, 0.0);
        assert_eq!(tool.failure_rate, 0.0);
    }

    #[test]
    fn test_aggregate_tool_stats_no_entries() {
        let stats = aggregate_tool_stats(&[]);
        assert!(stats.is_empty());
    }
}
