//! Error types for the build orchestration module

use thiserror::Error;

/// Build-specific error types
#[derive(Error, Debug)]
pub enum BuildError {
    /// Environment preparation failed
    #[error("Failed to prepare build environment: {reason}")]
    EnvironmentPreparation { reason: String },

    /// Container creation failed
    #[error("Failed to create container: {reason}")]
    ContainerCreation { reason: String },

    /// Build execution failed
    #[error("Build execution failed for package {package}: {reason}")]
    BuildExecution { package: String, reason: String },

    /// Artifact collection failed
    #[error("Failed to collect artifacts: {reason}")]
    ArtifactCollection { reason: String },

    /// Build timeout
    #[error("Build timed out after {duration_secs} seconds")]
    BuildTimeout { duration_secs: u64 },

    /// Missing build dependency
    #[error("Missing build dependency: {dependency}")]
    MissingDependency { dependency: String },

    /// Invalid build configuration
    #[error("Invalid build configuration: {reason}")]
    InvalidConfiguration { reason: String },

    /// Progress monitoring error
    #[error("Progress monitoring error: {reason}")]
    ProgressMonitoring { reason: String },

    /// Core library error
    #[error("Core error: {0}")]
    Core(#[from] colcon_deb_core::error::Error),

    /// Docker error
    #[error("Docker error: {0}")]
    Docker(#[from] colcon_deb_docker::error::DockerError),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Generic error with context
    #[error("{context}: {source}")]
    WithContext {
        context: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

/// Result type alias for build operations
pub type Result<T> = std::result::Result<T, BuildError>;

impl BuildError {
    /// Add context to an error
    pub fn context<E>(context: impl Into<String>, source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::WithContext { context: context.into(), source: Box::new(source) }
    }

    /// Create a configuration error
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    /// Create an environment preparation error
    pub fn environment(reason: impl Into<String>) -> Self {
        Self::EnvironmentPreparation { reason: reason.into() }
    }

    /// Create a build execution error
    pub fn build_failed(package: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::BuildExecution { package: package.into(), reason: reason.into() }
    }
}
