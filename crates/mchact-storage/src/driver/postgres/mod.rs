use deadpool_postgres::{Config as PoolConfig, Pool, Runtime};
use mchact_core::error::MchactError;
use tokio_postgres::NoTls;

use crate::traits::DataStore;

pub mod auth;
pub mod audit;
pub mod chat;
pub mod document;
pub mod knowledge;
pub mod media;
pub mod memory_db;
pub mod message;
pub mod metrics;
pub mod session;
pub mod subagent;
pub mod task;

const SCHEMA_SQL: &str = include_str!("../../schema/postgres.sql");

pub struct PgDriver {
    pub(super) pool: Pool,
}

impl PgDriver {
    pub async fn connect(database_url: &str) -> Result<Self, MchactError> {
        let mut cfg = PoolConfig::new();
        cfg.url = Some(database_url.to_string());
        let pool = cfg
            .create_pool(Some(Runtime::Tokio1), NoTls)
            .map_err(|e| MchactError::Config(format!("pool creation failed: {e}")))?;

        // Run schema initialization
        let client = pool
            .get()
            .await
            .map_err(|e| MchactError::Config(format!("connect failed: {e}")))?;
        for stmt in SCHEMA_SQL.split(';') {
            let trimmed = stmt.trim();
            if trimmed.is_empty() || trimmed.starts_with("--") {
                continue;
            }
            client
                .execute(trimmed, &[])
                .await
                .map_err(|e| MchactError::Config(format!("schema init failed: {e}")))?;
        }

        Ok(Self { pool })
    }
}

impl DataStore for PgDriver {}
