#!/usr/bin/env rust-script
//! ```cargo
//! [dependencies]
//! clap = { version = "4.0", features = ["derive"] }
//! tokio = { version = "1.0", features = ["process", "fs", "rt", "macros"] }
//! ```

// scripts/helpers/debian-preparer.rs
// Manages debian directories - uses existing configs or generates with bloom

use clap::Parser;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tokio::fs;

#[derive(Parser, Debug)]
#[command(name = "debian-preparer")]
#[command(about = "Prepare debian directories for ROS packages")]
struct Args {
    /// Package name
    package_name: String,
    
    /// Package source path
    package_path: PathBuf,
    
    /// Debian directories collection path
    debian_dirs: PathBuf,
    
    /// ROS distribution
    ros_distro: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    prepare_debian_dir(
        &args.package_name,
        &args.package_path,
        &args.debian_dirs,
        &args.ros_distro,
    ).await?;
    
    Ok(())
}

async fn prepare_debian_dir(
    package_name: &str,
    package_path: &Path,
    debian_dirs: &Path,
    ros_distro: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let debian_dir = debian_dirs.join(package_name).join("debian");
    let target_debian = package_path.join("debian");
    
    report_log("debug", &format!("Preparing debian dir for {}", package_name)).await?;
    
    if debian_dir.exists() {
        report_log("info", &format!("Using existing debian directory for {}", package_name)).await?;
        
        // Copy existing debian directory
        copy_dir_all(&debian_dir, &target_debian).await.map_err(|e| {
            format!("Failed to copy debian directory for {}: {}", package_name, e)
        })?;
    } else {
        report_log("info", &format!("Generating debian directory for {} with bloom-generate", package_name)).await?;
        
        // Generate with bloom
        let output = Command::new("bloom-generate")
            .args(&[
                "rosdebian",
                "--ros-distro", ros_distro,
                "--debian-inc", "0",
                "--os-name", "ubuntu",
                "--os-version", "jammy",
                "."
            ])
            .current_dir(package_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;
            
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            report_log("error", &format!("bloom-generate failed for {}: {}", package_name, stderr)).await?;
            return Err(format!("bloom-generate failed for {}", package_name).into());
        }
        
        // Save generated debian dir to collection
        if target_debian.exists() {
            let collection_dir = debian_dirs.join(package_name);
            fs::create_dir_all(&collection_dir).await?;
            
            match copy_dir_all(&target_debian, &collection_dir.join("debian")).await {
                Ok(_) => {
                    report_log("info", &format!("Saved generated debian directory for {}", package_name)).await?;
                }
                Err(e) => {
                    report_log("warning", &format!("Failed to save generated debian dir for {}: {}", package_name, e)).await?;
                    // Continue anyway, we have the debian dir in place
                }
            }
        } else {
            report_log("error", &format!("bloom-generate did not create debian directory for {}", package_name)).await?;
            return Err(format!("bloom-generate did not create debian directory for {}", package_name).into());
        }
    }
    
    Ok(())
}

async fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(dst).await?;
    
    let mut entries = fs::read_dir(src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let ty = entry.file_type().await?;
        if ty.is_dir() {
            Box::pin(copy_dir_all(&entry.path(), &dst.join(entry.file_name()))).await?;
        } else {
            fs::copy(entry.path(), dst.join(entry.file_name())).await?;
        }
    }
    
    Ok(())
}

async fn report_log(level: &str, message: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Use the progress-reporter if available
    if Path::new("/helpers/progress-reporter.rs").exists() {
        let output = Command::new("/helpers/progress-reporter.rs")
            .args(&["log", "--level", level, "--message", message])
            .output()?;
            
        if !output.status.success() {
            eprintln!("Failed to report log: {}", String::from_utf8_lossy(&output.stderr));
        }
    } else {
        // Fallback to direct output
        eprintln!("::log::level={},msg={}", level, message);
    }
    
    Ok(())
}