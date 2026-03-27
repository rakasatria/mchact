#[cfg(feature = "sqlite")]
pub mod sqlite;

#[cfg(feature = "postgres")]
pub mod postgres;

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::ObservationStore;

// ---------------------------------------------------------------------------
// MemoryConfig
// ---------------------------------------------------------------------------

fn default_backend() -> String {
    "sqlite".to_string()
}

fn default_db_path() -> String {
    "~/.microclaw/memory.db".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    #[serde(default = "default_backend")]
    pub backend: String,
    #[serde(default = "default_db_path")]
    pub db_path: String,
    #[serde(default)]
    pub database_url: Option<String>,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            backend: default_backend(),
            db_path: default_db_path(),
            database_url: None,
        }
    }
}

// ---------------------------------------------------------------------------
// create_store factory
// ---------------------------------------------------------------------------

/// Create an `ObservationStore` from `MemoryConfig`.
///
/// Returns `None` if the backend fails to initialize — the agent will run
/// without observation memory in that case.
pub async fn create_store(config: &MemoryConfig) -> Option<Arc<dyn ObservationStore>> {
    match config.backend.as_str() {
        "sqlite" => create_sqlite_store(config).await,
        "postgres" | "postgresql" => create_postgres_store(config).await,
        other => {
            warn!("mchact-memory: unknown backend '{other}', memory disabled");
            None
        }
    }
}

async fn create_sqlite_store(config: &MemoryConfig) -> Option<Arc<dyn ObservationStore>> {
    #[cfg(feature = "sqlite")]
    {
        use std::path::PathBuf;

        // Expand ~ in path
        let raw = config.db_path.as_str();
        let expanded = shellexpand::tilde(raw).into_owned();
        let path = PathBuf::from(&expanded);

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                warn!("mchact-memory: failed to create db directory {}: {e}", parent.display());
                return None;
            }
        }

        match sqlite::SqliteDriver::open(&path) {
            Ok(driver) => {
                tracing::info!("mchact-memory: SQLite backend opened at {expanded}");
                Some(Arc::new(driver))
            }
            Err(e) => {
                warn!("mchact-memory: failed to open SQLite database at {expanded}: {e}");
                None
            }
        }
    }

    #[cfg(not(feature = "sqlite"))]
    {
        let _ = config;
        warn!("mchact-memory: sqlite backend requested but feature not enabled");
        None
    }
}

async fn create_postgres_store(config: &MemoryConfig) -> Option<Arc<dyn ObservationStore>> {
    #[cfg(feature = "postgres")]
    {
        let url = match config.database_url.as_deref() {
            Some(u) if !u.is_empty() => u,
            _ => {
                warn!("mchact-memory: postgres backend requires database_url to be set");
                return None;
            }
        };

        match postgres::PgDriver::connect(url).await {
            Ok(driver) => {
                tracing::info!("mchact-memory: PostgreSQL backend connected");
                Some(Arc::new(driver))
            }
            Err(e) => {
                warn!("mchact-memory: failed to connect to PostgreSQL: {e}");
                None
            }
        }
    }

    #[cfg(not(feature = "postgres"))]
    {
        let _ = config;
        warn!("mchact-memory: postgres backend requested but feature not enabled");
        None
    }
}
