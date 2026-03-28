//! Storage and persistence domain for mchact.

pub mod db;
pub mod driver;
pub mod fts;
pub mod memory;
pub mod memory_quality;
pub mod traits;
pub mod usage;

pub use traits::DataStore;

/// Thread-safe, dynamically-dispatched DataStore.
pub type DynDataStore = dyn DataStore + Send + Sync;

/// Prelude that re-exports all storage traits.
/// Add `use mchact_storage::prelude::*;` in files that call Database methods.
pub mod prelude {
    pub use crate::traits::{
        AuthStore, AuditStore, ChatStore, DataStore, DocumentStore, KnowledgeStore,
        MediaObjectStore, MemoryDbStore, MessageStore, MetricsStore, SessionStore,
        SubagentStore, TaskStore,
    };
}
