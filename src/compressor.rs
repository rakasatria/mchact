// src/compressor.rs

use mchact_core::llm_types::{ContentBlock, Message, MessageContent};

const CHARS_PER_TOKEN: usize = 4;
const MIN_SUMMARY_TOKENS: usize = 2000;
const SUMMARY_RATIO: f64 = 0.20;
const SUMMARY_TOKENS_CEILING: usize = 12_000;
const TOOL_RESULT_PLACEHOLDER: &str =
    "[Tool output cleared -- use session_search to recall details]";

pub struct CompressorConfig {
    pub tail_token_budget: usize,
    pub protect_first_n: usize,
    pub tool_result_max_chars: usize,
    pub compaction_timeout_secs: u64,
}

impl Default for CompressorConfig {
    fn default() -> Self {
        Self {
            tail_token_budget: 20_000,
            protect_first_n: 3,
            tool_result_max_chars: 200,
            compaction_timeout_secs: 60,
        }
    }
}

pub struct ContextCompressor {
    config: CompressorConfig,
    previous_summary: Option<String>,
    compression_count: u32,
}

impl ContextCompressor {
    pub fn new(config: CompressorConfig) -> Self {
        Self {
            config,
            previous_summary: None,
            compression_count: 0,
        }
    }

    /// Estimate token count from a string.
    fn estimate_tokens(text: &str) -> usize {
        text.len() / CHARS_PER_TOKEN
    }

    /// Estimate token count for a message.
    fn message_tokens(msg: &Message) -> usize {
        Self::estimate_tokens(&message_to_text(msg))
    }

    /// Phase 1: Replace old ToolResult content with placeholder.
    fn prune_old_tool_results(&self, messages: &[Message], tail_start: usize) -> Vec<Message> {
        messages
            .iter()
            .enumerate()
            .map(|(i, msg)| {
                if i >= tail_start {
                    return msg.clone();
                }
                match &msg.content {
                    MessageContent::Blocks(blocks) => {
                        let pruned_blocks: Vec<ContentBlock> = blocks
                            .iter()
                            .map(|block| match block {
                                ContentBlock::ToolResult {
                                    tool_use_id,
                                    content,
                                    is_error,
                                } if content.len() > self.config.tool_result_max_chars => {
                                    ContentBlock::ToolResult {
                                        tool_use_id: tool_use_id.clone(),
                                        content: TOOL_RESULT_PLACEHOLDER.to_string(),
                                        is_error: *is_error,
                                    }
                                }
                                other => other.clone(),
                            })
                            .collect();
                        Message {
                            role: msg.role.clone(),
                            content: MessageContent::Blocks(pruned_blocks),
                        }
                    }
                    _ => msg.clone(),
                }
            })
            .collect()
    }

    /// Phase 3: Find tail start index by walking backward with token budget.
    fn find_tail_start(&self, messages: &[Message]) -> usize {
        let mut tokens_accumulated: usize = 0;
        for i in (0..messages.len()).rev() {
            tokens_accumulated += Self::message_tokens(&messages[i]);
            if tokens_accumulated >= self.config.tail_token_budget {
                return i + 1;
            }
        }
        0
    }

    /// Compute max summary tokens from compression zone size.
    fn summary_budget(compression_zone_chars: usize) -> usize {
        let zone_tokens = compression_zone_chars / CHARS_PER_TOKEN;
        let budget = (zone_tokens as f64 * SUMMARY_RATIO) as usize;
        budget.max(MIN_SUMMARY_TOKENS).min(SUMMARY_TOKENS_CEILING)
    }

    /// Phase 4/5: Build the summarization prompt.
    fn build_summary_prompt(&self, compression_zone_text: &str) -> String {
        if let Some(prev) = &self.previous_summary {
            format!(
                "Here is the existing conversation summary:\n{prev}\n\n\
                 Here are new conversation turns since that summary:\n{compression_zone_text}\n\n\
                 PRESERVE all existing information that is still relevant.\n\
                 ADD new progress. Move items between Done/In Progress/Blocked as needed.\n\
                 Organize into:\n\
                 - **Goal**: What the user wants to accomplish\n\
                 - **Progress**: Done / In Progress / Blocked items\n\
                 - **Key Decisions**: Important choices made\n\
                 - **Relevant Files/Commands**: Paths, commands, URLs mentioned\n\
                 - **Critical Context**: Anything needed to continue"
            )
        } else {
            format!(
                "Summarize the following conversation segment. Organize into:\n\
                 - **Goal**: What the user wants to accomplish\n\
                 - **Progress**: Done / In Progress / Blocked items\n\
                 - **Key Decisions**: Important choices made\n\
                 - **Relevant Files/Commands**: Paths, commands, URLs mentioned\n\
                 - **Critical Context**: Anything needed to continue\n\n\
                 ---\n\n{compression_zone_text}"
            )
        }
    }

    /// Post-phase: Remove orphaned ToolUse/ToolResult blocks.
    fn sanitize_tool_pairs(messages: &[Message]) -> Vec<Message> {
        // Collect all tool_use IDs and tool_result IDs
        let mut tool_use_ids = std::collections::HashSet::new();
        let mut tool_result_ids = std::collections::HashSet::new();

        for msg in messages {
            if let MessageContent::Blocks(blocks) = &msg.content {
                for block in blocks {
                    match block {
                        ContentBlock::ToolUse { id, .. } => {
                            tool_use_ids.insert(id.clone());
                        }
                        ContentBlock::ToolResult { tool_use_id, .. } => {
                            tool_result_ids.insert(tool_use_id.clone());
                        }
                        _ => {}
                    }
                }
            }
        }

        messages
            .iter()
            .map(|msg| match &msg.content {
                MessageContent::Blocks(blocks) => {
                    let filtered: Vec<ContentBlock> = blocks
                        .iter()
                        .filter(|block| match block {
                            ContentBlock::ToolUse { id, .. } => tool_result_ids.contains(id),
                            ContentBlock::ToolResult { tool_use_id, .. } => {
                                tool_use_ids.contains(tool_use_id)
                            }
                            _ => true,
                        })
                        .cloned()
                        .collect();
                    if filtered.is_empty() {
                        Message {
                            role: msg.role.clone(),
                            content: MessageContent::Text("[context cleared]".into()),
                        }
                    } else {
                        Message {
                            role: msg.role.clone(),
                            content: MessageContent::Blocks(filtered),
                        }
                    }
                }
                _ => msg.clone(),
            })
            .collect()
    }

    /// Main entry point. Returns compressed messages.
    /// `summarize_fn` is an async closure that calls the LLM to summarize.
    pub async fn compress<F, Fut>(
        &mut self,
        messages: &[Message],
        summarize_fn: F,
    ) -> Vec<Message>
    where
        F: FnOnce(String, String) -> Fut,
        Fut: std::future::Future<Output = Result<String, String>>,
    {
        let total = messages.len();
        let head_end = self.config.protect_first_n.min(total);

        // Phase 3: Find tail boundary
        let tail_start = self.find_tail_start(messages).max(head_end);

        if tail_start <= head_end {
            // Nothing to compress
            return messages.to_vec();
        }

        // Phase 1: Prune old tool results
        let pruned = self.prune_old_tool_results(messages, tail_start);

        // Phase 2: Protected head
        let head = &pruned[..head_end];
        let compression_zone = &pruned[head_end..tail_start];
        let tail = &pruned[tail_start..];

        // Build compression zone text
        let mut zone_text = String::new();
        for msg in compression_zone {
            zone_text.push_str(&format!("[{}]: {}\n\n", msg.role, message_to_text(msg)));
        }

        if zone_text.is_empty() {
            return messages.to_vec();
        }

        // Truncate if very long
        let max_chars = Self::summary_budget(zone_text.len()) * CHARS_PER_TOKEN * 5;
        if zone_text.len() > max_chars {
            zone_text.truncate(max_chars);
            zone_text.push_str("\n... (truncated)");
        }

        // Phase 4/5: Summarize
        let prompt = self.build_summary_prompt(&zone_text);
        let system = "You are a helpful summarizer. Preserve concrete details like file paths, commands, error messages, and decisions.".to_string();

        let summary = match summarize_fn(system, prompt).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Compaction summarization failed: {e}, falling back to truncation");
                return tail.to_vec();
            }
        };

        // Store for iterative updates
        self.previous_summary = Some(summary.clone());
        self.compression_count += 1;

        // Build compacted message list
        let mut compacted = Vec::new();
        compacted.extend_from_slice(head);
        compacted.push(Message {
            role: "user".into(),
            content: MessageContent::Text(format!("[Conversation Summary]\n{summary}")),
        });
        compacted.push(Message {
            role: "assistant".into(),
            content: MessageContent::Text(
                "Understood, I have the conversation context. How can I help?".into(),
            ),
        });
        compacted.extend_from_slice(tail);

        // Post-phase: Sanitize tool pairs
        let sanitized = Self::sanitize_tool_pairs(&compacted);

        // Fix role alternation -- merge consecutive same-role messages
        let mut result: Vec<Message> = Vec::new();
        for msg in sanitized {
            if let Some(last) = result.last_mut() {
                if last.role == msg.role {
                    let existing = message_to_text(last);
                    let new_text = message_to_text(&msg);
                    last.content = MessageContent::Text(format!("{existing}\n{new_text}"));
                    continue;
                }
            }
            result.push(msg);
        }

        // Ensure last message is from user
        if let Some(last) = result.last() {
            if last.role == "assistant" {
                result.pop();
            }
        }

        result
    }
}

/// Extract text from a message (replicates agent_engine helper).
pub fn message_to_text(msg: &Message) -> String {
    match &msg.content {
        MessageContent::Text(t) => t.clone(),
        MessageContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                ContentBlock::ToolResult { content, .. } => Some(content.as_str()),
                ContentBlock::ToolUse { name, .. } => {
                    Some(name.as_str()) // minimal representation
                }
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}
