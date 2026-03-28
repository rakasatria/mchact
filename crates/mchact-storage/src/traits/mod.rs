pub mod chat;
pub mod message;
pub mod session;
pub mod task;
pub mod memory;
pub mod auth;
pub mod audit;
pub mod metrics;
pub mod subagent;
pub mod document;
pub mod media;
pub mod knowledge;

pub use chat::ChatStore;
pub use message::MessageStore;
pub use session::SessionStore;
pub use task::TaskStore;
pub use memory::MemoryDbStore;
pub use auth::AuthStore;
pub use audit::AuditStore;
pub use metrics::MetricsStore;
pub use subagent::SubagentStore;
pub use document::DocumentStore;
pub use media::MediaObjectStore;
pub use knowledge::KnowledgeStore;

/// Combined trait for all storage operations.
pub trait DataStore:
    ChatStore
    + MessageStore
    + SessionStore
    + TaskStore
    + MemoryDbStore
    + AuthStore
    + AuditStore
    + MetricsStore
    + SubagentStore
    + DocumentStore
    + MediaObjectStore
    + KnowledgeStore
    + Send
    + Sync
{
}
