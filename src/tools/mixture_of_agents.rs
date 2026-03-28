// src/tools/mixture_of_agents.rs

use std::sync::Arc;

use async_trait::async_trait;
use mchact_channels::channel_adapter::ChannelRegistry;
use mchact_core::llm_types::ToolDefinition;
use mchact_storage::db::Database;
use serde_json::json;

use super::{Tool, ToolResult};
use crate::config::Config;

const DEFAULT_PERSPECTIVES: &[&str] = &[
    "Analyze this as a pragmatic engineer focused on simplicity and correctness.",
    "Analyze this as a skeptic looking for edge cases, failure modes, and risks.",
    "Analyze this as an architect focused on long-term maintainability and scalability.",
    "Analyze this as a security and risk analyst focused on vulnerabilities and compliance.",
    "Analyze this as a user/stakeholder advocate focused on usability and impact.",
];

const AGGREGATOR_SYSTEM_PROMPT: &str = "You have been provided with a set of responses from various perspectives to the latest user query. Your task is to synthesize these responses into a single, high-quality response. It is crucial to critically evaluate the information provided in these responses, recognizing that some of it may be biased or incorrect. Your response should not simply replicate the given answers but should offer a refined, accurate, and comprehensive reply. Ensure your response is well-structured, coherent, and adheres to the highest standards of accuracy and reliability.\n\nResponses from perspectives:";

pub struct MixtureOfAgentsTool {
    config: Config,
    db: Arc<Database>,
    channel_registry: Arc<ChannelRegistry>,
}

impl MixtureOfAgentsTool {
    pub fn new(
        config: &Config,
        db: Arc<Database>,
        channel_registry: Arc<ChannelRegistry>,
    ) -> Self {
        Self {
            config: config.clone(),
            db,
            channel_registry,
        }
    }

    fn resolve_perspectives(
        &self,
        count: usize,
        approach_hints: Option<Vec<String>>,
    ) -> Vec<String> {
        if let Some(hints) = approach_hints {
            return hints;
        }
        DEFAULT_PERSPECTIVES
            .iter()
            .take(count)
            .map(|s| s.to_string())
            .collect()
    }
}

#[async_trait]
impl Tool for MixtureOfAgentsTool {
    fn name(&self) -> &str {
        "mixture_of_agents"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "mixture_of_agents".into(),
            description: "Route a hard problem through multiple independent perspectives collaboratively. Spawns N sub-agents that each tackle the same question from a different angle, then synthesizes a consensus answer. Use sparingly for genuinely difficult problems that benefit from diverse analysis.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "user_prompt": {
                        "type": "string",
                        "description": "The complex query or problem to solve using multiple perspectives"
                    },
                    "perspectives": {
                        "type": "integer",
                        "description": "Number of independent agents (default 3, max 5)",
                        "default": 3
                    },
                    "approach_hints": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Explicit perspective labels (optional)"
                    },
                    "model_overrides": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Per-worker model or provider_preset names (optional)"
                    },
                    "wait_timeout_secs": {
                        "type": "integer",
                        "description": "Timeout in seconds (default 120, max 300)",
                        "default": 120
                    },
                    "min_successful": {
                        "type": "integer",
                        "description": "Minimum workers that must succeed (default 1)",
                        "default": 1
                    }
                },
                "required": ["user_prompt"]
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let user_prompt = match input.get("user_prompt").and_then(|v| v.as_str()) {
            Some(p) if !p.trim().is_empty() => p.to_string(),
            _ => return ToolResult::error("Missing or empty 'user_prompt' parameter".into()),
        };

        let perspective_count = input
            .get("perspectives")
            .and_then(|v| v.as_u64())
            .unwrap_or(3)
            .min(5) as usize;

        let approach_hints: Option<Vec<String>> = input
            .get("approach_hints")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            });

        let wait_timeout = input
            .get("wait_timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(120)
            .min(300);

        let min_successful = input
            .get("min_successful")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as usize;

        let perspectives = self.resolve_perspectives(perspective_count, approach_hints);

        // Build work packages — each is the same question with a different perspective
        let work_packages: Vec<serde_json::Value> = perspectives
            .iter()
            .map(|perspective| {
                json!(format!(
                    "PERSPECTIVE: {perspective}\n\nQUESTION: {user_prompt}"
                ))
            })
            .collect();

        // Use subagents_orchestrate internally
        let orchestrate_input = json!({
            "goal": format!("Answer this question from {} independent perspectives and provide thorough analysis", perspectives.len()),
            "work_packages": work_packages,
            "wait": true,
            "wait_timeout_secs": wait_timeout,
        });

        // Pass through auth context and subagent runtime metadata
        let mut full_input = orchestrate_input;
        if let Some(auth) = input.get("__auth_context") {
            full_input
                .as_object_mut()
                .unwrap()
                .insert("__auth_context".into(), auth.clone());
        }
        if let Some(runtime) = input.get("__subagent_runtime") {
            full_input
                .as_object_mut()
                .unwrap()
                .insert("__subagent_runtime".into(), runtime.clone());
        }

        // Execute orchestration
        let orchestrate_tool = crate::tools::subagents::SubagentsOrchestrateTool::new(
            &self.config,
            self.db.clone(),
            self.channel_registry.clone(),
        );
        let orch_result = orchestrate_tool.execute(full_input).await;

        if orch_result.is_error {
            return ToolResult::error(format!(
                "MoA orchestration failed: {}",
                orch_result.content
            ));
        }

        // Parse orchestration result
        let orch_data: serde_json::Value = match serde_json::from_str(&orch_result.content) {
            Ok(v) => v,
            Err(e) => {
                return ToolResult::error(format!("Failed to parse orchestration result: {e}"))
            }
        };

        // Extract worker results
        let runs = orch_data
            .get("runs")
            .and_then(|r| r.as_array())
            .cloned()
            .unwrap_or_default();

        let successful_results: Vec<(usize, String)> = runs
            .iter()
            .enumerate()
            .filter_map(|(i, run)| {
                let status = run.get("status").and_then(|s| s.as_str()).unwrap_or("");
                if status == "completed" {
                    let result = run
                        .get("result_text")
                        .and_then(|r| r.as_str())
                        .unwrap_or("[no output]")
                        .to_string();
                    Some((i, result))
                } else {
                    None
                }
            })
            .collect();

        let successful_count = successful_results.len();
        let failed_count = runs.len() - successful_count;

        if successful_count < min_successful {
            return ToolResult::error(format!(
                "Insufficient successful perspectives ({}/{}).\nNeed at least {}. {} workers failed.",
                successful_count,
                runs.len(),
                min_successful,
                failed_count
            ));
        }

        // Build aggregator prompt
        let mut aggregator_prompt = AGGREGATOR_SYSTEM_PROMPT.to_string();
        aggregator_prompt.push_str("\n\n");
        for (idx, (orig_idx, result)) in successful_results.iter().enumerate() {
            let perspective_label = perspectives
                .get(*orig_idx)
                .map(|s| s.as_str())
                .unwrap_or("Unknown");
            aggregator_prompt.push_str(&format!(
                "{}. [{}]:\n{}\n\n",
                idx + 1,
                perspective_label,
                result
            ));
        }

        let synthesis_prompt = format!(
            "{aggregator_prompt}\n\
             Synthesize into a single answer that:\n\
             - Identifies points of agreement (high confidence)\n\
             - Notes disagreements and which perspective is most convincing\n\
             - Provides a final recommended answer"
        );

        // Return the formatted multi-perspective output for the parent agent to synthesize.
        // Full synthesis would require calling the LLM directly, which is handled by
        // the parent agent loop using this structured output.
        let perspectives_used: Vec<String> = successful_results
            .iter()
            .filter_map(|(idx, _)| perspectives.get(*idx).cloned())
            .collect();

        let result_json = json!({
            "success": true,
            "response": synthesis_prompt,
            "perspectives_used": perspectives_used,
            "successful_count": successful_count,
            "failed_count": failed_count,
        });

        ToolResult::success(serde_json::to_string_pretty(&result_json).unwrap_or_default())
    }
}
