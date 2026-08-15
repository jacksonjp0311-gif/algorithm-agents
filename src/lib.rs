pub mod agents;
pub mod archive;
pub mod archive_match;
pub mod collect;
pub mod demo;
pub mod error;
pub mod fetch;
pub mod hunt;
pub mod host;
pub mod parse;
pub mod permissions;
pub mod persist;
pub mod pipeline;
pub mod registry;
pub mod runtime;
pub mod sandbox;
pub mod schema;
pub mod smoke;
pub mod supervisor;
pub mod tools;
pub mod ui;

pub use archive::FileArchive;
pub use error::AgentError;
pub use host::{ArchiveHost, MemoryHost, PublishedAlgorithm, ReviewCandidate, SubmittedCandidate};
pub use persist::{connect_sqlite, default_data_dir};
pub use registry::Registry;
pub use runtime::AgentRuntime;
pub use supervisor::{ManualSupervisor, ScriptedSupervisor, SupervisorAdapter, attach_contract};
pub use tools::dispatch;

use std::path::Path;
use std::sync::Arc;

use sqlx::SqlitePool;

pub async fn open_runtime(
    root: impl AsRef<Path>,
    data_dir: impl AsRef<Path>,
    pool: SqlitePool,
    host: Arc<dyn ArchiveHost>,
) -> Result<AgentRuntime, AgentError> {
    AgentRuntime::open(
        root.as_ref().to_path_buf(),
        data_dir.as_ref().to_path_buf(),
        pool,
        host,
    )
    .await
}
