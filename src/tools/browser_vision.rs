use async_trait::async_trait;
use base64::Engine as _;
use mchact_core::llm_types::ToolDefinition;
use mchact_tools::command_runner::agent_browser_program;
use serde_json::json;
use std::path::PathBuf;
use tracing::{info, warn};

use super::{auth_context_from_input, schema_object, Tool, ToolResult};
use crate::config::Config;

pub struct BrowserVisionTool {
    vision_model: String,
    vision_api_key: Option<String>,
    vision_base_url: String,
}

impl BrowserVisionTool {
    pub fn new(config: &Config) -> Self {
        let api_key = config.vision_fallback_api_key.clone().or_else(|| {
            let env_key = std::env::var("OPENAI_API_KEY").ok();
            if env_key.is_some() {
                info!("browser_vision: using OPENAI_API_KEY from environment");
            }
            env_key
        });

        Self {
            vision_model: config.vision_fallback_model.clone(),
            vision_api_key: api_key,
            vision_base_url: config.vision_fallback_base_url.clone(),
        }
    }

    fn session_name_for_chat(chat_id: i64) -> String {
        let normalized = if chat_id < 0 {
            format!("neg{}", chat_id.unsigned_abs())
        } else {
            chat_id.to_string()
        };
        format!("mchact-chat-{normalized}")
    }

    fn tmp_screenshot_path(&self) -> PathBuf {
        let filename = format!("browser_vision_{}.png", uuid::Uuid::new_v4());
        std::env::temp_dir().join(filename)
    }
}

#[async_trait]
impl Tool for BrowserVisionTool {
    fn name(&self) -> &str {
        "browser_vision"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "browser_vision".into(),
            description: "Capture a screenshot of the current browser session and analyze it using a vision model. Returns a text analysis of what's visible in the screenshot.".into(),
            input_schema: schema_object(
                json!({
                    "query": {
                        "type": "string",
                        "description": "What to look for or analyze in the screenshot (e.g. 'what buttons are visible', 'is there an error message', 'describe the current page state')"
                    },
                    "selector": {
                        "type": "string",
                        "description": "Optional CSS selector to capture a specific element instead of the full page"
                    }
                }),
                &["query"],
            ),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let query = match input.get("query").and_then(|v| v.as_str()) {
            Some(q) if !q.trim().is_empty() => q.to_string(),
            _ => return ToolResult::error("Missing required parameter: query".to_string()),
        };

        let selector = input
            .get("selector")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .map(String::from);

        let auth = auth_context_from_input(&input);
        let chat_id = match auth.as_ref() {
            Some(a) => a.caller_chat_id,
            None => {
                return ToolResult::error(
                    "No auth context available. Cannot determine browser session.".to_string(),
                )
            }
        };

        let session_name = Self::session_name_for_chat(chat_id);
        let tmp_path = self.tmp_screenshot_path();

        // Build agent-browser screenshot command args
        let mut args = vec![
            "--session".to_string(),
            session_name.clone(),
            "screenshot".to_string(),
            tmp_path.to_string_lossy().to_string(),
        ];
        if let Some(ref sel) = selector {
            args.push("--selector".to_string());
            args.push(sel.clone());
        }

        let program = agent_browser_program();
        info!(
            session = %session_name,
            tmp_path = %tmp_path.display(),
            "browser_vision: capturing screenshot via '{}'",
            program
        );

        // Run agent-browser to capture screenshot
        let output = tokio::process::Command::new(&program)
            .args(&args)
            .output()
            .await;

        let screenshot_bytes = match output {
            Ok(out) if out.status.success() => {
                // Read the screenshot file
                match tokio::fs::read(&tmp_path).await {
                    Ok(bytes) if !bytes.is_empty() => bytes,
                    Ok(_) => {
                        // Empty file — session likely not running
                        let _ = tokio::fs::remove_file(&tmp_path).await;
                        return ToolResult::error(
                            "Screenshot file is empty. Make sure the browser session is active. \
                             Use the `browser` tool with `open <url>` to start a session first."
                                .to_string(),
                        );
                    }
                    Err(e) => {
                        warn!("browser_vision: failed to read screenshot file: {e}");
                        return ToolResult::error(
                            "Browser session does not appear to be running. \
                             Use the `browser` tool with `open <url>` to start a session first."
                                .to_string(),
                        );
                    }
                }
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                let code = out.status.code().unwrap_or(-1);
                warn!(
                    "browser_vision: agent-browser exited with code {code}: {stderr}"
                );
                return ToolResult::error(format!(
                    "Failed to capture screenshot (exit code {code}). \
                     Make sure the browser session is running. \
                     Use the `browser` tool with `open <url>` to start a session first.\n\
                     Details: {stderr}"
                ));
            }
            Err(e) => {
                return ToolResult::error(format!(
                    "Failed to launch agent-browser for screenshot: {e}"
                ))
                .with_error_type("spawn_error");
            }
        };

        // Clean up temp file
        let _ = tokio::fs::remove_file(&tmp_path).await;

        // Base64 encode the PNG
        let b64_image = base64::engine::general_purpose::STANDARD.encode(&screenshot_bytes);

        // Resolve API key
        let api_key = match &self.vision_api_key {
            Some(k) if !k.trim().is_empty() => k.clone(),
            _ => {
                return ToolResult::error(
                    "No vision API key configured. Set vision_fallback_api_key in config \
                     or OPENAI_API_KEY environment variable."
                        .to_string(),
                )
            }
        };

        // Call vision API (OpenAI-compatible chat completions)
        let url = format!("{}/chat/completions", self.vision_base_url.trim_end_matches('/'));

        let request_body = json!({
            "model": self.vision_model,
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": query
                        },
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": format!("data:image/png;base64,{b64_image}")
                            }
                        }
                    ]
                }
            ],
            "max_tokens": 1024
        });

        info!(
            model = %self.vision_model,
            url = %url,
            "browser_vision: sending screenshot to vision API"
        );

        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await;

        let resp = match response {
            Ok(r) => r,
            Err(e) => {
                return ToolResult::error(format!("Vision API request failed: {e}"))
                    .with_error_type("network_error");
            }
        };

        let status = resp.status();
        let body_text = resp.text().await.unwrap_or_default();

        if !status.is_success() {
            return ToolResult::error(format!(
                "Vision API returned HTTP {status}: {body_text}"
            ));
        }

        // Parse response
        let parsed: serde_json::Value = match serde_json::from_str(&body_text) {
            Ok(v) => v,
            Err(e) => {
                return ToolResult::error(format!(
                    "Failed to parse vision API response: {e}\nRaw: {body_text}"
                ))
            }
        };

        let analysis = parsed
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .map(String::from)
            .unwrap_or_else(|| {
                format!("Vision API returned unexpected response format: {body_text}")
            });

        ToolResult::success(analysis)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn make_config() -> Config {
        let mut cfg = Config::test_defaults();
        cfg.vision_fallback_model = "gpt-4o".to_string();
        cfg.vision_fallback_base_url = "https://api.openai.com/v1".to_string();
        cfg.vision_fallback_api_key = Some("test-key".to_string());
        cfg.data_dir = "/tmp/test-data".to_string();
        cfg
    }

    #[test]
    fn test_browser_vision_definition() {
        let cfg = make_config();
        let tool = BrowserVisionTool::new(&cfg);
        assert_eq!(tool.name(), "browser_vision");

        let def = tool.definition();
        assert_eq!(def.name, "browser_vision");

        // Required params: query
        let required = def.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "query"));

        // Optional: selector (not in required)
        assert!(!required.iter().any(|v| v == "selector"));

        // Both properties exist
        assert!(def.input_schema["properties"]["query"].is_object());
        assert!(def.input_schema["properties"]["selector"].is_object());
    }

    #[test]
    fn test_session_name_for_positive_chat_id() {
        assert_eq!(
            BrowserVisionTool::session_name_for_chat(12345),
            "mchact-chat-12345"
        );
    }

    #[test]
    fn test_session_name_for_negative_chat_id() {
        assert_eq!(
            BrowserVisionTool::session_name_for_chat(-100987),
            "mchact-chat-neg100987"
        );
    }

    #[tokio::test]
    async fn test_missing_query_returns_error() {
        let cfg = make_config();
        let tool = BrowserVisionTool::new(&cfg);
        let result = tool.execute(serde_json::json!({})).await;
        assert!(result.is_error);
        assert!(result.content.contains("Missing required parameter: query"));
    }

    #[tokio::test]
    async fn test_empty_query_returns_error() {
        let cfg = make_config();
        let tool = BrowserVisionTool::new(&cfg);
        let result = tool.execute(serde_json::json!({"query": "  "})).await;
        assert!(result.is_error);
        assert!(result.content.contains("Missing required parameter: query"));
    }
}
