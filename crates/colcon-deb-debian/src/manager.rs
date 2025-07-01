//! Debian directory manager

// use std::path::Path;

// use colcon_deb_core::Result;

/// Manages Debian directories
pub struct DebianManager {
    debian_dirs: std::path::PathBuf,
}

impl DebianManager {
    /// Create a new Debian manager
    pub fn new(debian_dirs: impl Into<std::path::PathBuf>) -> Self {
        Self { debian_dirs: debian_dirs.into() }
    }

    /// Check if a package has a custom Debian directory
    pub fn has_custom_debian(&self, package_name: &str) -> bool {
        self.debian_dirs.join(package_name).join("debian").exists()
    }
}
