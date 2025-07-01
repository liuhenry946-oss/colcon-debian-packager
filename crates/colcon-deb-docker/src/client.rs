//! Docker client wrapper

use std::time::Duration;

use bollard::Docker;
use colcon_deb_core::error::{Error, Result};

/// Docker service configuration
pub struct DockerConfig {
    /// Socket path (None for default)
    pub socket_path: Option<String>,
    /// Operation timeout
    pub timeout: Duration,
    /// Maximum concurrent image pulls
    pub max_concurrent_pulls: usize,
}

impl Default for DockerConfig {
    fn default() -> Self {
        Self {
            socket_path: None,
            timeout: Duration::from_secs(300),
            max_concurrent_pulls: 2,
        }
    }
}

/// High-level Docker service wrapper
pub struct DockerService {
    client: Docker,
    #[allow(dead_code)]
    config: DockerConfig,
}

impl DockerService {
    /// Create a new Docker service
    pub async fn new(config: DockerConfig) -> Result<Self> {
        let client = match &config.socket_path {
            Some(path) => Docker::connect_with_socket(path, 120, bollard::API_DEFAULT_VERSION),
            None => Docker::connect_with_local_defaults(),
        }
        .map_err(|e| Error::DockerError { message: format!("Failed to connect to Docker: {e}") })?;

        // Test connection
        client.ping().await.map_err(|e| Error::DockerError {
            message: format!("Docker daemon not accessible: {e}"),
        })?;

        Ok(Self { client, config })
    }

    /// Get the underlying Docker client
    pub fn client(&self) -> &Docker {
        &self.client
    }
}
