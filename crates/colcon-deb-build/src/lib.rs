//! Build orchestration for Colcon Debian Packager
//!
//! This crate coordinates the build process, managing Docker containers
//! and tracking build progress.

// use colcon_deb_core::error::{Error, Result};

pub mod orchestrator;

pub use orchestrator::BuildOrchestrator;
