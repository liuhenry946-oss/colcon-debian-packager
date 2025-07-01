//! ROS package handling for Colcon Debian Packager
//!
//! This crate provides functionality for discovering and parsing
//! ROS packages in a workspace.

// use colcon_deb_core::error::{Error, Result};

pub mod parser;
pub mod scanner;

pub use parser::parse_package_xml;
pub use scanner::scan_workspace;
