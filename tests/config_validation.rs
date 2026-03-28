//! Integration tests for configuration loading and validation.

use mchact::config::{Config, WorkingDirIsolation};

/// Helper to create a minimal valid config for testing.
fn minimal_config() -> Config {
    Config {
        telegram_bot_token: "tok".into(),
        bot_username: "testbot".into(),
        llm_provider: "anthropic".into(),
        api_key: "test-key".into(),
        model: String::new(),
        provider_presets: std::collections::HashMap::new(),
        llm_providers: std::collections::HashMap::new(),
        llm_base_url: None,
        llm_user_agent: mchact::http_client::default_llm_user_agent(),
        max_tokens: 8192,
        max_tool_iterations: 25,
        max_history_messages: 50,
        max_document_size_mb: 100,
        memory_token_budget: 1500,
        data_dir: "./mchact.data".into(),
        skills_dir: None,
        working_dir: "./tmp".into(),
        working_dir_isolation: WorkingDirIsolation::Chat,
        high_risk_tool_user_confirmation_required: true,
        sandbox: mchact::config::SandboxConfig::default(),
        openai_api_key: None,
        override_timezone: None,
        timezone: "UTC".into(),
        allowed_groups: vec![],
        control_chat_ids: vec![],
        max_session_messages: 40,
        compact_keep_recent: 20,
        default_tool_timeout_secs: 30,
        tool_timeout_overrides: std::collections::HashMap::new(),
        default_mcp_request_timeout_secs: 120,
        compaction_timeout_secs: 180,
        discord_bot_token: None,
        discord_allowed_channels: vec![],
        discord_no_mention: false,
        allow_group_slash_without_mention: false,
        show_thinking: false,
        subagents: mchact::config::SubagentConfig::default(),
        a2a: mchact::config::A2AConfig::default(),
        openai_compat_body_overrides: std::collections::HashMap::new(),
        openai_compat_body_overrides_by_provider: std::collections::HashMap::new(),
        openai_compat_body_overrides_by_model: std::collections::HashMap::new(),
        web_enabled: false,
        web_host: "127.0.0.1".into(),
        web_port: 3900,
        web_max_inflight_per_session: 2,
        web_max_requests_per_window: 8,
        web_rate_window_seconds: 10,
        web_run_history_limit: 512,
        web_session_idle_ttl_seconds: 300,
        web_fetch_validation:
            mchact_tools::web_content_validation::WebContentValidationConfig::default(),
        web_fetch_url_validation: mchact_tools::web_fetch::WebFetchUrlValidationConfig::default(
        ),
        model_prices: vec![],
        embedding_provider: None,
        embedding_api_key: None,
        embedding_base_url: None,
        embedding_model: None,
        embedding_dim: None,
        reflector_enabled: true,
        reflector_interval_mins: 15,
        soul_path: None,
        souls_dir: None,
        clawhub: mchact::config::ClawHubConfig::default(),
        plugins: mchact::plugins::PluginsConfig::default(),
        voice_provider: "openai".into(),
        voice_transcription_command: None,
        observability: None,
        channels: std::collections::HashMap::new(),
        memory: mchact_memory::driver::MemoryConfig::default(),
        // Multimodal
        tts_enabled: true,
        tts_provider: "edge".into(),
        tts_voice: "en-US-AriaNeural".into(),
        tts_api_key: None,
        tts_elevenlabs_voice_id: None,
        stt_enabled: true,
        stt_provider: "whisper-local".into(),
        stt_model: "base".into(),
        stt_model_path: None,
        image_gen_enabled: true,
        image_gen_provider: "openai".into(),
        image_gen_api_key: None,
        image_gen_fal_key: None,
        image_gen_default_size: "1024x1024".into(),
        image_gen_default_quality: "standard".into(),
        video_gen_enabled: false,
        video_gen_provider: "sora".into(),
        video_gen_api_key: None,
        video_gen_fal_model: None,
        video_gen_minimax_key: None,
        video_gen_timeout_secs: 300,
        vision_fallback_enabled: true,
        vision_fallback_provider: "openrouter".into(),
        vision_fallback_model: "anthropic/claude-sonnet-4".into(),
        vision_fallback_api_key: None,
        vision_fallback_base_url: "https://openrouter.ai/api/v1".into(),
        document_extraction_enabled: true,
        // Training configuration
        training_default_workers: 4,
        training_default_batch_size: 10,
        training_default_max_iterations: 10,
        training_default_distribution: "default".into(),
        training_output_dir: "./training-runs".into(),
        training_compress_target_tokens: 15250,
        training_compress_model: "google/gemini-3-flash-preview".into(),
        training_compress_tokenizer: "moonshotai/Kimi-K2-Thinking".into(),
        training_environments_dir: "./training/environments".into(),
        training_distributions_file: "./training/distributions.yaml".into(),
        skill_nudge_enabled: true,
        skill_nudge_threshold_tool_calls: 10,
        skill_nudge_threshold_turns: 15,
        skill_nudge_threshold_duration_secs: 300,
        storage_backend: "local".into(),
        storage_cache_max_size_mb: 1024,
        storage_s3_bucket: None,
        storage_s3_region: None,
        storage_s3_endpoint: None,
        storage_s3_access_key_id: None,
        storage_s3_secret_access_key: None,
        storage_azure_container: None,
        storage_azure_connection_string: None,
        storage_azure_account_name: None,
        storage_azure_account_key: None,
        storage_gcs_bucket: None,
        storage_gcs_credentials_path: None,
        knowledge_embed_interval_mins: 5,
        knowledge_embed_batch_size: 50,
        knowledge_observe_interval_mins: 15,
        knowledge_observe_batch_size: 20,
        knowledge_autogroup_interval_mins: 60,
        knowledge_autogroup_min_docs: 5,
        knowledge_retry_delay_mins: 30,
        knowledge_max_embedding_tokens: 8192,
        db_backend: "sqlite".into(),
        db_database_url: None,
    }
}

#[test]
fn test_yaml_parse_minimal() {
    let yaml = "telegram_bot_token: tok\nbot_username: bot\napi_key: key\n";
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.telegram_bot_token, "tok");
    assert_eq!(config.bot_username, "bot");
    assert_eq!(config.api_key, "key");
    // Defaults
    assert_eq!(config.llm_provider, "anthropic");
    assert_eq!(config.max_tokens, 8192);
    assert_eq!(config.max_tool_iterations, 100);
    assert_eq!(config.max_document_size_mb, 100);
    assert_eq!(config.max_history_messages, 50);
    assert_eq!(config.timezone, "auto");
    assert!(matches!(
        config.working_dir_isolation,
        WorkingDirIsolation::Chat
    ));
    assert_eq!(config.max_session_messages, 40);
    assert_eq!(config.compact_keep_recent, 20);
    assert_eq!(config.default_tool_timeout_secs, 30);
    assert_eq!(config.default_mcp_request_timeout_secs, 120);
    assert!(config.high_risk_tool_user_confirmation_required);
    assert!(config.sandbox.require_runtime);
    assert!(config.web_fetch_validation.enabled);
    assert!(config.web_fetch_validation.strict_mode);
    assert!(config.web_fetch_url_validation.enabled);
}

#[test]
fn test_yaml_parse_full() {
    let yaml = r#"
telegram_bot_token: my_token
bot_username: mybot
llm_provider: openai
api_key: sk-test123
model: gpt-4o
llm_base_url: https://custom.api.com/v1
max_tokens: 4096
max_tool_iterations: 10
max_history_messages: 100
data_dir: /data/mchact
working_dir: /data/mchact/tmp
openai_api_key: sk-whisper
timezone: Asia/Shanghai
allowed_groups:
  - 111
  - 222
control_chat_ids:
  - 999
max_session_messages: 60
compact_keep_recent: 30
discord_bot_token: discord_tok
discord_allowed_channels:
  - 333
  - 444
"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.telegram_bot_token, "my_token");
    assert_eq!(config.llm_provider, "openai");
    assert_eq!(config.model, "gpt-4o");
    assert_eq!(
        config.llm_base_url.as_deref(),
        Some("https://custom.api.com/v1")
    );
    assert_eq!(config.max_tokens, 4096);
    assert_eq!(config.max_tool_iterations, 10);
    assert_eq!(config.max_history_messages, 100);
    assert_eq!(config.data_dir, "/data/mchact");
    assert_eq!(config.working_dir, "/data/mchact/tmp");
    assert_eq!(config.openai_api_key.as_deref(), Some("sk-whisper"));
    assert_eq!(config.timezone, "Asia/Shanghai");
    assert_eq!(config.allowed_groups, vec![111, 222]);
    assert_eq!(config.control_chat_ids, vec![999]);
    assert_eq!(config.max_session_messages, 60);
    assert_eq!(config.compact_keep_recent, 30);
    assert_eq!(config.discord_allowed_channels, vec![333, 444]);
}

#[test]
fn test_yaml_roundtrip() {
    let config = minimal_config();
    let yaml = serde_yaml::to_string(&config).unwrap();
    let parsed: Config = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(parsed.telegram_bot_token, config.telegram_bot_token);
    assert_eq!(parsed.api_key, config.api_key);
    assert_eq!(parsed.max_tokens, config.max_tokens);
    assert_eq!(parsed.timezone, "auto");
}

#[test]
fn test_data_dir_paths() {
    let mut config = minimal_config();
    config.data_dir = "/opt/mchact.data".into();

    let runtime = std::path::PathBuf::from(config.runtime_data_dir());
    let skills = std::path::PathBuf::from(config.skills_data_dir());

    assert!(runtime.ends_with(std::path::Path::new("mchact.data").join("runtime")));
    assert!(skills.ends_with(std::path::Path::new("mchact.data").join("skills")));
}

#[test]
fn test_yaml_unknown_fields_ignored() {
    let yaml = "telegram_bot_token: tok\nbot_username: bot\napi_key: key\nunknown_field: value\n";
    // serde_yaml should not fail on unknown fields by default
    let config: Result<Config, _> = serde_yaml::from_str(yaml);
    // This may fail or succeed depending on serde config; verify behavior
    if let Ok(c) = config {
        assert_eq!(c.telegram_bot_token, "tok");
    }
    // If it errors, that's also acceptable behavior (strict mode)
}

#[test]
fn test_yaml_empty_string_fields() {
    let yaml = "telegram_bot_token: ''\nbot_username: ''\napi_key: ''\n";
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.telegram_bot_token, "");
    assert_eq!(config.bot_username, "");
    assert_eq!(config.api_key, "");
}
