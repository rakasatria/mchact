use std::path::{Path, PathBuf};
use std::process::Command;

use crate::batch;
use crate::export;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Result
// ---------------------------------------------------------------------------

pub struct PipelineResult {
    pub trajectories: PathBuf,
    pub exported: Option<PathBuf>,
    pub compressed: Option<PathBuf>,
    pub statistics: PathBuf,
}

// ---------------------------------------------------------------------------
// Pipeline
// ---------------------------------------------------------------------------

static ALL_TOOLS: &[&str] = &[
    "bash",
    "browser",
    "read_file",
    "write_file",
    "edit_file",
    "glob",
    "grep",
    "web_fetch",
    "web_search",
    "send_message",
    "image_generate",
    "text_to_speech",
    "video_generate",
    "read_document",
    "browser_vision",
    "mixture_of_agents",
];

/// Run the full training pipeline: batch → export → compress.
///
/// Steps:
/// 1. Create output directory.
/// 2. Load dataset, optionally filter completed prompts (resume), split into batches.
/// 3. Spawn workers via `batch::spawn_workers`.
/// 4. Combine batch outputs into `trajectories.jsonl` via `batch::combine_batches`.
/// 5. If `format != "openai"`: export via `export::export_file`.
/// 6. If `compress`: invoke `python3 training/compress.py` on the final JSONL.
/// 7. Return paths to all produced files.
pub fn run_pipeline(config: &PipelineConfig) -> Result<PipelineResult, String> {
    // --- Step 1: create output directory ---
    std::fs::create_dir_all(&config.output_dir).map_err(|e| {
        format!(
            "failed to create output dir '{}': {e}",
            config.output_dir.display()
        )
    })?;

    // --- Step 2: load dataset ---
    let mut prompts = batch::load_dataset(&config.dataset, None)?;

    if config.resume {
        let completed = batch::find_completed_prompts(&config.output_dir);
        if !completed.is_empty() {
            println!("Resuming: {} prompts already completed", completed.len());
            prompts = batch::filter_completed(prompts, &completed);
        }
    }

    let batches = batch::split_batches(prompts, config.batch_size);
    println!(
        "Run '{}': {} prompt(s) across {} batch(es) (batch_size={}, workers={})",
        config.run_name,
        batches.iter().map(|b| b.len()).sum::<usize>(),
        batches.len(),
        config.batch_size,
        config.workers,
    );

    // --- Step 3: spawn workers ---
    let output_paths = batch::spawn_workers(
        &batches,
        &config.output_dir,
        config.workers,
        &config.distribution,
        config.max_iterations,
        config.model.as_deref(),
        config.config_path.as_deref(),
        None,
    )?;
    println!(
        "All workers complete: {} batch file(s) written",
        output_paths.len()
    );

    // --- Step 4: combine batches ---
    let all_tools: Vec<String> = ALL_TOOLS.iter().map(|s| s.to_string()).collect();
    let combine_result = batch::combine_batches(&config.output_dir, &all_tools)?;
    println!(
        "Combined: total={} valid={} filtered={}",
        combine_result.total, combine_result.valid, combine_result.filtered
    );

    let trajectories_path = config.output_dir.join("trajectories.jsonl");
    let statistics_path = config.output_dir.join("statistics.json");

    // Write a minimal statistics JSON so callers always have the path.
    let stats_json = serde_json::json!({
        "run_name": config.run_name,
        "total": combine_result.total,
        "valid": combine_result.valid,
        "filtered": combine_result.filtered,
    });
    std::fs::write(
        &statistics_path,
        serde_json::to_string_pretty(&stats_json)
            .map_err(|e| format!("failed to serialize statistics: {e}"))?,
    )
    .map_err(|e| format!("failed to write statistics: {e}"))?;

    // --- Step 5: optional export (format conversion) ---
    let exported_path: Option<PathBuf> = if config.format != "openai" {
        let out_path =
            export::default_output_path(&trajectories_path, &config.format);
        export::export_file(
            &trajectories_path,
            &out_path,
            &config.format,
            &config.parser,
            false,
            None,
        )
        .map_err(|e| format!("export failed: {e}"))?;
        println!("Exported to: {}", out_path.display());
        Some(out_path)
    } else {
        None
    };

    // --- Step 6: optional compression ---
    let compressed_path: Option<PathBuf> = if config.compress {
        let compress_input = exported_path
            .as_deref()
            .unwrap_or(&trajectories_path);
        let compress_output = derive_compressed_path(compress_input);

        run_compress_script(compress_input, &compress_output, config.target_tokens)?;
        println!("Compressed to: {}", compress_output.display());
        Some(compress_output)
    } else {
        None
    };

    Ok(PipelineResult {
        trajectories: trajectories_path,
        exported: exported_path,
        compressed: compressed_path,
        statistics: statistics_path,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Derive a `_compressed` output path from an input JSONL path.
fn derive_compressed_path(input: &Path) -> PathBuf {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("trajectories");
    let parent = input.parent().unwrap_or_else(|| Path::new("."));
    parent.join(format!("{stem}_compressed.jsonl"))
}

/// Invoke `python3 training/compress.py <input> --target-tokens <N> --output <output>`.
fn run_compress_script(input: &Path, output: &Path, target_tokens: usize) -> Result<(), String> {
    // Verify python3 is available.
    let python_check = Command::new("python3").arg("--version").output();
    match python_check {
        Ok(out) if out.status.success() => {}
        Ok(_) => {
            return Err(
                "python3 is not functioning correctly. \
                 Please install Python 3 to use the compress step."
                    .to_string(),
            )
        }
        Err(_) => {
            return Err(
                "python3 not found. \
                 Please install Python 3 (https://python.org) to use the compress step."
                    .to_string(),
            )
        }
    }

    let status = Command::new("python3")
        .arg("training/compress.py")
        .arg(input)
        .arg("--target-tokens")
        .arg(target_tokens.to_string())
        .arg("--output")
        .arg(output)
        .status()
        .map_err(|e| format!("failed to spawn compress.py: {e}"))?;

    if !status.success() {
        return Err(format!(
            "compress.py exited with non-zero status: {:?}",
            status.code()
        ));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_compressed_path_standard() {
        let input = Path::new("/data/trajectories.jsonl");
        let out = derive_compressed_path(input);
        assert_eq!(out, PathBuf::from("/data/trajectories_compressed.jsonl"));
    }

    #[test]
    fn test_derive_compressed_path_sharegpt() {
        let input = Path::new("/runs/abc/trajectories_sharegpt.jsonl");
        let out = derive_compressed_path(input);
        assert_eq!(
            out,
            PathBuf::from("/runs/abc/trajectories_sharegpt_compressed.jsonl")
        );
    }

    #[test]
    fn test_all_tools_list_non_empty() {
        assert!(!ALL_TOOLS.is_empty());
        assert!(ALL_TOOLS.contains(&"bash"));
        assert!(ALL_TOOLS.contains(&"mixture_of_agents"));
    }
}
