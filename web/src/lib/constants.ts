import type { A2APeerDraft, UiTheme } from "./types";

export const DOCS_BASE = "https://mchact.ai/docs";

export const PROVIDER_SUGGESTIONS = [
  "openai",
  "openai-codex",
  "ollama",
  "openrouter",
  "anthropic",
  "google",
  "aliyun-bailian",
  "alibaba",
  "deepseek",
  "moonshot",
  "mistral",
  "azure",
  "bedrock",
  "zhipu",
  "minimax",
  "cohere",
  "tencent",
  "xai",
  "nvidia",
  "huggingface",
  "together",
  "custom",
];

export const MODEL_OPTIONS: Record<string, string[]> = {
  anthropic: [
    "claude-sonnet-4-5-20250929",
    "claude-opus-4-1-20250805",
    "claude-3-7-sonnet-latest",
  ],
  openai: ["gpt-5.2"],
  "openai-codex": ["gpt-5.3-codex"],
  ollama: ["llama3.2", "qwen2.5", "deepseek-r1"],
  openrouter: [
    "openai/gpt-5",
    "anthropic/claude-sonnet-4-5",
    "google/gemini-2.5-pro",
  ],
  deepseek: ["deepseek-chat", "deepseek-reasoner"],
  google: ["gemini-2.5-pro", "gemini-2.5-flash"],
  "aliyun-bailian": ["qwen3.5-plus", "qwen3-max", "qwen-plus-latest"],
  nvidia: ["meta/llama-3.3-70b-instruct", "meta/llama-3.1-70b-instruct"],
};

export const DEFAULT_CONFIG_VALUES = {
  llm_provider: "anthropic",
  working_dir_isolation: "chat",
  high_risk_tool_user_confirmation_required: true,
  max_tokens: 8192,
  max_tool_iterations: 100,
  max_document_size_mb: 100,
  memory_token_budget: 1500,
  show_thinking: false,
  web_enabled: true,
  web_host: "127.0.0.1",
  web_port: 10961,
  reflector_enabled: true,
  reflector_interval_mins: 15,
  embedding_provider: "",
  embedding_api_key: "",
  embedding_base_url: "",
  embedding_model: "",
  embedding_dim: "",
  a2a_enabled: false,
  a2a_public_base_url: "",
  a2a_agent_name: "",
  a2a_agent_description: "",
  a2a_shared_tokens: "",
  a2a_peers: [] as A2APeerDraft[],
  souls_dir: "",
};

export const UI_THEME_OPTIONS: { key: UiTheme; label: string; color: string }[] = [
  { key: "green", label: "Green", color: "#34d399" },
  { key: "blue", label: "Blue", color: "#60a5fa" },
  { key: "slate", label: "Slate", color: "#94a3b8" },
  { key: "amber", label: "Amber", color: "#fbbf24" },
  { key: "violet", label: "Violet", color: "#a78bfa" },
  { key: "rose", label: "Rose", color: "#fb7185" },
  { key: "cyan", label: "Cyan", color: "#22d3ee" },
  { key: "teal", label: "Teal", color: "#2dd4bf" },
  { key: "orange", label: "Orange", color: "#fb923c" },
  { key: "indigo", label: "Indigo", color: "#818cf8" },
];

export const RADIX_ACCENT_BY_THEME: Record<UiTheme, string> = {
  green: "green",
  blue: "blue",
  slate: "gray",
  amber: "amber",
  violet: "violet",
  rose: "ruby",
  cyan: "cyan",
  teal: "teal",
  orange: "orange",
  indigo: "indigo",
};

export const BOT_SLOT_MAX = 10;
export const MAIN_PROFILE_VALUE = "__main__";
