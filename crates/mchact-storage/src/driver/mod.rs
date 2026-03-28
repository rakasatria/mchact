use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::warn;

#[cfg(feature = "postgres")]
pub mod postgres;

fn default_backend() -> String {
    "sqlite".into()
}

fn default_db_path() -> String {
    "mchact.data".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageDriverConfig {
    #[serde(default = "default_backend")]
    pub backend: String,
    #[serde(default = "default_db_path")]
    pub db_path: String,
    #[serde(default)]
    pub database_url: Option<String>,
}

impl Default for StorageDriverConfig {
    fn default() -> Self {
        Self {
            backend: default_backend(),
            db_path: default_db_path(),
            database_url: None,
        }
    }
}

/// Create a `DataStore` from `StorageDriverConfig`.
///
/// Returns `None` if the backend fails to initialize — the bot will run
/// without persistent storage in that case, logging a warning.
pub async fn create_data_store(config: &StorageDriverConfig) -> Option<Arc<crate::DynDataStore>> {
    match config.backend.as_str() {
        "sqlite" => create_sqlite_store(config),
        "postgres" | "postgresql" => create_postgres_store(config).await,
        other => {
            warn!("mchact-storage: unknown backend '{other}', storage disabled");
            None
        }
    }
}

fn create_sqlite_store(config: &StorageDriverConfig) -> Option<Arc<crate::DynDataStore>> {
    use crate::db::Database;
    match Database::new(&config.db_path) {
        Ok(db) => Some(Arc::new(db)),
        Err(e) => {
            warn!("mchact-storage: sqlite init failed: {e}");
            None
        }
    }
}

#[cfg(feature = "postgres")]
async fn create_postgres_store(config: &StorageDriverConfig) -> Option<Arc<crate::DynDataStore>> {
    let url = config.database_url.as_deref().unwrap_or("");
    if url.is_empty() {
        warn!("mchact-storage: postgres backend requires database_url");
        return None;
    }
    match postgres::PgDriver::connect(url).await {
        Ok(driver) => Some(Arc::new(driver)),
        Err(e) => {
            warn!("mchact-storage: postgres init failed: {e}");
            None
        }
    }
}

#[cfg(not(feature = "postgres"))]
async fn create_postgres_store(_config: &StorageDriverConfig) -> Option<Arc<crate::DynDataStore>> {
    warn!("mchact-storage: postgres feature not compiled in");
    None
}
