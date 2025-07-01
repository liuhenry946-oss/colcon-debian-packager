//! Debian packaging for Colcon Debian Packager
//!
//! This crate manages Debian directory structures and interfaces
//! with bloom-generate for package generation.

// use colcon_deb_core::error::{Error, Result};

pub mod manager;

pub use manager::DebianManager;
