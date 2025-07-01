//! Docker integration for Colcon Debian Packager
//!
//! This crate provides Docker container management for building
//! Debian packages in isolated environments.

// use colcon_deb_core::error::{Error, Result};

pub mod client;
pub mod container;
pub mod image;

pub use client::DockerService;
pub use container::{ContainerSpec, VolumeMount};
pub use image::ImageManager;
