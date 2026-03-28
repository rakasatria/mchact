pub mod a2a;
pub mod batch;
pub mod batch_worker;
pub mod acp;
pub mod acp_subagent;
pub mod agent_engine;
pub mod channels;
pub mod compressor;
pub mod distributions;
pub mod export;
pub mod parsers;
pub mod train_pipeline;
pub mod chat_commands;
pub mod clawhub;
pub mod codex_auth;
pub mod config;
pub mod doctor;
pub mod embedding;
pub mod gateway;
pub mod hooks;
pub mod http_client;
pub mod knowledge;
pub mod knowledge_scheduler;
pub mod llm;
pub mod mcp;
pub mod media_manager;
pub mod memory_backend;
pub mod memory_service;
pub mod plugins;
pub(crate) mod run_control;
pub mod runtime;
pub mod scheduler;
pub mod setup;
pub mod setup_def;
pub mod skills;
pub mod tools;
pub mod rl;
pub mod web;

pub use channels::discord;
pub use channels::telegram;
pub use mchact_app::builtin_skills;
pub use mchact_app::logging;
pub use mchact_app::transcribe;
pub use mchact_channels::channel;
pub use mchact_channels::channel_adapter;
pub use mchact_core::error;
pub use mchact_core::llm_types;
pub use mchact_core::text;
pub use mchact_storage::db;
pub use mchact_storage::memory;
pub use mchact_storage::memory_quality;
pub use mchact_tools::sandbox;

#[cfg(test)]
pub mod test_support {
    use std::io::ErrorKind;
    use std::net::TcpListener;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    pub fn env_lock() -> MutexGuard<'static, ()> {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    pub fn bind_test_tcp_listener() -> Option<TcpListener> {
        match TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => Some(listener),
            Err(err) if err.kind() == ErrorKind::PermissionDenied => {
                eprintln!("skipping test: loopback bind not permitted in this environment");
                None
            }
            Err(err) => panic!("failed to bind test TCP listener: {err}"),
        }
    }

    pub async fn bind_test_tokio_listener() -> Option<tokio::net::TcpListener> {
        match tokio::net::TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => Some(listener),
            Err(err) if err.kind() == ErrorKind::PermissionDenied => {
                eprintln!("skipping test: loopback bind not permitted in this environment");
                None
            }
            Err(err) => panic!("failed to bind test tokio TCP listener: {err}"),
        }
    }
}
