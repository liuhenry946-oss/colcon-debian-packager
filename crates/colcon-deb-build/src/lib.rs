//! Build orchestration for Colcon Debian Packager
//!
//! This crate coordinates the build process, managing Docker containers
//! and tracking build progress.

pub mod artifact;
pub mod context;
pub mod error;
pub mod executor;
pub mod orchestrator;

pub use artifact::{ArtifactCollector, BuildArtifact};
pub use context::{BuildContext, BuildState};
pub use error::{BuildError, Result};
pub use executor::{BuildExecutor, ExecutorConfig};
pub use orchestrator::{BuildOrchestrator, BuildOrchestratorTrait, ColconDebBuilder};
