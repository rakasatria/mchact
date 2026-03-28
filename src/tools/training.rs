use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::json;

use super::{schema_object, Tool, ToolResult};
use mchact_core::llm_types::ToolDefinition;

// ---------------------------------------------------------------------------
// BatchGenerateTool
// ---------------------------------------------------------------------------

pub struct BatchGenerateTool;

#[async_trait]
impl Tool for BatchGenerateTool {
    fn name(&self) -> &str {
        "batch_generate"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "batch_generate".into(),
            description:
                "Run a batch generation job: load a dataset, split into batches, and spawn \
                 worker processes to generate agent trajectories. Returns run metadata and \
                 output location."
                    .into(),
            input_schema: schema_object(
                json!({
                    "dataset": {
                        "type": "string",
                        "description": "Path to the JSONL dataset file"
                    },
                    "run_name": {
                        "type": "string",
                        "description": "Name for this training run"
                    },
                    "output_dir": {
                        "type": "string",
                        "description": "Directory where batch outputs will be written"
                    },
                    "workers": {
                        "type": "integer",
                        "description": "Number of parallel worker processes (default: 4)"
                    },
                    "batch_size": {
                        "type": "integer",
                        "description": "Number of prompts per batch (default: 10)"
                    },
                    "distribution": {
                        "type": "string",
                        "description": "Toolset distribution name (default: \"all\")"
                    },
                    "max_iterations": {
                        "type": "integer",
                        "description": "Max agent loop iterations per prompt (default: 20)"
                    },
                    "model": {
                        "type": "string",
                        "description": "Model override (optional)"
                    },
                    "max_samples": {
                        "type": "integer",
                        "description": "Maximum number of prompts to load from the dataset (optional)"
                    },
                    "resume": {
                        "type": "boolean",
                        "description": "Skip already-completed prompts (default: false)"
                    }
                }),
                &["dataset", "run_name", "output_dir"],
            ),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let dataset_str = match input.get("dataset").and_then(|v| v.as_str()) {
            Some(s) => s.to_owned(),
            None => return ToolResult::error("Missing required parameter: dataset".into()),
        };
        let run_name = match input.get("run_name").and_then(|v| v.as_str()) {
            Some(s) => s.to_owned(),
            None => return ToolResult::error("Missing required parameter: run_name".into()),
        };
        let output_dir_str = match input.get("output_dir").and_then(|v| v.as_str()) {
            Some(s) => s.to_owned(),
            None => return ToolResult::error("Missing required parameter: output_dir".into()),
        };

        let workers = input
            .get("workers")
            .and_then(|v| v.as_u64())
            .unwrap_or(4) as usize;
        let batch_size = input
            .get("batch_size")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;
        let distribution = input
            .get("distribution")
            .and_then(|v| v.as_str())
            .unwrap_or("all")
            .to_owned();
        let max_iterations = input
            .get("max_iterations")
            .and_then(|v| v.as_u64())
            .unwrap_or(20) as usize;
        let model = input
            .get("model")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned());
        let max_samples = input
            .get("max_samples")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);
        let resume = input
            .get("resume")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let dataset_path = PathBuf::from(dataset_str);
        let output_dir = PathBuf::from(output_dir_str.clone());

        let start = std::time::Instant::now();

        let result = tokio::task::spawn_blocking(move || {
            let prompts = crate::batch::load_dataset(&dataset_path, max_samples)?;
            let total_prompts = prompts.len();

            let prompts = if resume {
                let completed = crate::batch::find_completed_prompts(&output_dir);
                if !completed.is_empty() {
                    crate::batch::filter_completed(prompts, &completed)
                } else {
                    prompts
                }
            } else {
                prompts
            };

            let batches = crate::batch::split_batches(prompts, batch_size);

            crate::batch::spawn_workers(
                &batches,
                &output_dir,
                workers,
                &distribution,
                max_iterations,
                model.as_deref(),
                None,
                None,
            )?;

            Ok::<usize, String>(total_prompts)
        })
        .await;

        let duration_seconds = start.elapsed().as_secs_f64();

        match result {
            Ok(Ok(total_prompts)) => ToolResult::success(
                json!({
                    "run_name": run_name,
                    "output_dir": output_dir_str,
                    "total_prompts": total_prompts,
                    "duration_seconds": duration_seconds
                })
                .to_string(),
            ),
            Ok(Err(e)) => ToolResult::error(format!("Batch generation failed: {e}")),
            Err(e) => ToolResult::error(format!("Task panicked: {e}")),
        }
    }
}

// ---------------------------------------------------------------------------
// ExportTrajectoriesTool
// ---------------------------------------------------------------------------

pub struct ExportTrajectoriesTool;

#[async_trait]
impl Tool for ExportTrajectoriesTool {
    fn name(&self) -> &str {
        "export_trajectories"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "export_trajectories".into(),
            description:
                "Convert a JSONL trajectories file to a target format (e.g. sharegpt). \
                 Returns the output path and export statistics."
                    .into(),
            input_schema: schema_object(
                json!({
                    "input": {
                        "type": "string",
                        "description": "Path to the input JSONL trajectories file"
                    },
                    "output": {
                        "type": "string",
                        "description": "Path for the output file (optional; derived from input if omitted)"
                    },
                    "format": {
                        "type": "string",
                        "description": "Output format: \"sharegpt\" or \"openai\" (default: \"sharegpt\")"
                    },
                    "parser": {
                        "type": "string",
                        "description": "Tool-call parser name (default: \"hermes\")"
                    },
                    "filter_completed": {
                        "type": "boolean",
                        "description": "Skip entries where completed == false (default: true)"
                    },
                    "filter_min_tools": {
                        "type": "integer",
                        "description": "Skip entries with fewer than this many api_calls (optional)"
                    }
                }),
                &["input"],
            ),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let input_str = match input.get("input").and_then(|v| v.as_str()) {
            Some(s) => s.to_owned(),
            None => return ToolResult::error("Missing required parameter: input".into()),
        };

        let format = input
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("sharegpt")
            .to_owned();
        let parser = input
            .get("parser")
            .and_then(|v| v.as_str())
            .unwrap_or("hermes")
            .to_owned();
        let filter_completed = input
            .get("filter_completed")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let filter_min_tools = input
            .get("filter_min_tools")
            .and_then(|v| v.as_u64());

        let input_path = PathBuf::from(input_str);
        let output_path = match input.get("output").and_then(|v| v.as_str()) {
            Some(s) => PathBuf::from(s),
            None => crate::export::default_output_path(&input_path, &format),
        };
        let output_str = output_path.to_string_lossy().to_string();

        let result = tokio::task::spawn_blocking(move || {
            crate::export::export_file(
                &input_path,
                &output_path,
                &format,
                &parser,
                filter_completed,
                filter_min_tools,
            )
            .map_err(|e| e.to_string())
        })
        .await;

        match result {
            Ok(Ok(stats)) => ToolResult::success(
                json!({
                    "output": output_str,
                    "entries_exported": stats.exported,
                    "entries_filtered": stats.filtered,
                    "entries_total": stats.total
                })
                .to_string(),
            ),
            Ok(Err(e)) => ToolResult::error(format!("Export failed: {e}")),
            Err(e) => ToolResult::error(format!("Task panicked: {e}")),
        }
    }
}

// ---------------------------------------------------------------------------
// CompressTrajectoriesTool
// ---------------------------------------------------------------------------

pub struct CompressTrajectoriesTool;

#[async_trait]
impl Tool for CompressTrajectoriesTool {
    fn name(&self) -> &str {
        "compress_trajectories"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "compress_trajectories".into(),
            description:
                "Compress a JSONL trajectories file by running the Python compression script \
                 (training/compress.py). Reduces token count to fit training budgets."
                    .into(),
            input_schema: schema_object(
                json!({
                    "input": {
                        "type": "string",
                        "description": "Path to the input JSONL file to compress"
                    },
                    "output": {
                        "type": "string",
                        "description": "Path for the compressed output file (optional; derived from input if omitted)"
                    },
                    "target_tokens": {
                        "type": "integer",
                        "description": "Target token count per entry (default: 4096)"
                    }
                }),
                &["input"],
            ),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let input_str = match input.get("input").and_then(|v| v.as_str()) {
            Some(s) => s.to_owned(),
            None => return ToolResult::error("Missing required parameter: input".into()),
        };

        let target_tokens = input
            .get("target_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(4096) as usize;

        let input_path = PathBuf::from(&input_str);
        let output_path = match input.get("output").and_then(|v| v.as_str()) {
            Some(s) => PathBuf::from(s),
            None => derive_compressed_path(&input_path),
        };
        let output_str = output_path.to_string_lossy().to_string();

        let mut cmd = tokio::process::Command::new("python3");
        cmd.arg("training/compress.py")
            .arg("--input")
            .arg(&input_str)
            .arg("--output")
            .arg(&output_str)
            .arg("--target-tokens")
            .arg(target_tokens.to_string());

        let output_result = cmd.output().await;

        match output_result {
            Ok(output) => {
                if output.status.success() {
                    ToolResult::success(
                        json!({
                            "output": output_str,
                            "target_tokens": target_tokens
                        })
                        .to_string(),
                    )
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    ToolResult::error(format!(
                        "compress.py exited with status {}: {stderr}",
                        output.status
                    ))
                }
            }
            Err(e) => ToolResult::error(format!("Failed to spawn python3: {e}")),
        }
    }
}

fn derive_compressed_path(input: &std::path::Path) -> PathBuf {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("trajectories");
    let parent = input.parent().unwrap_or_else(|| std::path::Path::new("."));
    parent.join(format!("{stem}_compressed.jsonl"))
}

// ---------------------------------------------------------------------------
// TrainPipelineTool
// ---------------------------------------------------------------------------

pub struct TrainPipelineTool;

#[async_trait]
impl Tool for TrainPipelineTool {
    fn name(&self) -> &str {
        "train_pipeline"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "train_pipeline".into(),
            description:
                "Run the full training pipeline end-to-end: generate trajectories, export to \
                 training format, and optionally compress. Returns the run name and paths to \
                 all produced output files."
                    .into(),
            input_schema: schema_object(
                json!({
                    "dataset": {
                        "type": "string",
                        "description": "Path to the JSONL dataset file"
                    },
                    "run_name": {
                        "type": "string",
                        "description": "Name for this training run"
                    },
                    "output_dir": {
                        "type": "string",
                        "description": "Directory where all pipeline outputs will be written"
                    },
                    "workers": {
                        "type": "integer",
                        "description": "Number of parallel worker processes (default: 4)"
                    },
                    "batch_size": {
                        "type": "integer",
                        "description": "Number of prompts per batch (default: 10)"
                    },
                    "distribution": {
                        "type": "string",
                        "description": "Toolset distribution name (default: \"all\")"
                    },
                    "max_iterations": {
                        "type": "integer",
                        "description": "Max agent loop iterations per prompt (default: 20)"
                    },
                    "model": {
                        "type": "string",
                        "description": "Model override (optional)"
                    },
                    "format": {
                        "type": "string",
                        "description": "Output format: \"sharegpt\" or \"openai\" (default: \"sharegpt\")"
                    },
                    "parser": {
                        "type": "string",
                        "description": "Tool-call parser name (default: \"hermes\")"
                    },
                    "compress": {
                        "type": "boolean",
                        "description": "Whether to run the compression step (default: false)"
                    },
                    "target_tokens": {
                        "type": "integer",
                        "description": "Target token count for compression (default: 4096)"
                    },
                    "resume": {
                        "type": "boolean",
                        "description": "Skip already-completed prompts (default: false)"
                    }
                }),
                &["dataset", "run_name", "output_dir"],
            ),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let dataset_str = match input.get("dataset").and_then(|v| v.as_str()) {
            Some(s) => s.to_owned(),
            None => return ToolResult::error("Missing required parameter: dataset".into()),
        };
        let run_name = match input.get("run_name").and_then(|v| v.as_str()) {
            Some(s) => s.to_owned(),
            None => return ToolResult::error("Missing required parameter: run_name".into()),
        };
        let output_dir_str = match input.get("output_dir").and_then(|v| v.as_str()) {
            Some(s) => s.to_owned(),
            None => return ToolResult::error("Missing required parameter: output_dir".into()),
        };

        let config = crate::train_pipeline::PipelineConfig {
            dataset: PathBuf::from(dataset_str),
            workers: input
                .get("workers")
                .and_then(|v| v.as_u64())
                .unwrap_or(4) as usize,
            batch_size: input
                .get("batch_size")
                .and_then(|v| v.as_u64())
                .unwrap_or(10) as usize,
            distribution: input
                .get("distribution")
                .and_then(|v| v.as_str())
                .unwrap_or("all")
                .to_owned(),
            max_iterations: input
                .get("max_iterations")
                .and_then(|v| v.as_u64())
                .unwrap_or(20) as usize,
            model: input
                .get("model")
                .and_then(|v| v.as_str())
                .map(|s| s.to_owned()),
            format: input
                .get("format")
                .and_then(|v| v.as_str())
                .unwrap_or("sharegpt")
                .to_owned(),
            parser: input
                .get("parser")
                .and_then(|v| v.as_str())
                .unwrap_or("hermes")
                .to_owned(),
            compress: input
                .get("compress")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            target_tokens: input
                .get("target_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(4096) as usize,
            run_name: run_name.clone(),
            output_dir: PathBuf::from(output_dir_str.clone()),
            resume: input
                .get("resume")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            config_path: None,
        };

        let result = tokio::task::spawn_blocking(move || {
            crate::train_pipeline::run_pipeline(&config)
        })
        .await;

        match result {
            Ok(Ok(pipeline_result)) => {
                let mut outputs = json!({
                    "run_name": run_name,
                    "output_dir": output_dir_str,
                    "trajectories": pipeline_result.trajectories.to_string_lossy().to_string(),
                    "statistics": pipeline_result.statistics.to_string_lossy().to_string(),
                });

                if let Some(exported) = pipeline_result.exported {
                    outputs["exported"] =
                        json!(exported.to_string_lossy().to_string());
                }
                if let Some(compressed) = pipeline_result.compressed {
                    outputs["compressed"] =
                        json!(compressed.to_string_lossy().to_string());
                }

                ToolResult::success(outputs.to_string())
            }
            Ok(Err(e)) => ToolResult::error(format!("Pipeline failed: {e}")),
            Err(e) => ToolResult::error(format!("Task panicked: {e}")),
        }
    }
}
