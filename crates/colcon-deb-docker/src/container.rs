//! Container management

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Container specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerSpec {
    /// Docker image to use
    pub image: String,

    /// Command to execute
    pub command: Vec<String>,

    /// Environment variables
    pub environment: HashMap<String, String>,

    /// Volume mounts
    pub volumes: Vec<VolumeMount>,

    /// Working directory
    pub working_dir: Option<String>,

    /// User to run as
    pub user: Option<String>,
}

/// Volume mount specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeMount {
    /// Host path
    pub host_path: PathBuf,

    /// Container path
    pub container_path: String,

    /// Read-only mount
    pub read_only: bool,
}
