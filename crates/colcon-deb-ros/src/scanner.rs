//! Workspace scanner

use std::path::Path;

use colcon_deb_core::{Package, Result};

/// Scan a workspace for ROS packages
pub fn scan_workspace(_workspace_path: &Path) -> Result<Vec<Package>> {
    // TODO: Implement workspace scanning
    todo!("Implement workspace scanning")
}
