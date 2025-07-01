#!/usr/bin/env rust-script
//! ```cargo
//! [dependencies]
//! clap = { version = "4.0", features = ["derive"] }
//! tokio = { version = "1.0", features = ["process", "fs", "rt-multi-thread", "macros", "io-util"] }
//! serde = { version = "1.0", features = ["derive"] }
//! serde_json = "1.0"
//! futures = "0.3"
//! ```

// scripts/helpers/build-orchestrator.rs
// Orchestrates the entire build process with parallel .deb creation

use clap::Parser;
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tokio::fs;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command as TokioCommand;

#[derive(Parser, Debug)]
#[command(name = "build-orchestrator")]
#[command(about = "Orchestrate the build process for all packages")]
struct Args {
    /// Workspace path
    #[arg(long, default_value = "/workspace")]
    workspace: PathBuf,
    
    /// Debian directories path
    #[arg(long, default_value = "/workspace/debian_dirs")]
    debian_dirs: PathBuf,
    
    /// Output directory
    #[arg(long, default_value = "/workspace/output")]
    output_dir: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
struct ScanResult {
    packages: Vec<Package>,
    total: usize,
    errors: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Package {
    name: String,
    version: String,
    path: PathBuf,
    build_type: String,
    description: String,
    maintainers: Vec<String>,
    dependencies: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    let ros_distro = env::var("ROS_DISTRO").unwrap_or_else(|_| "humble".to_string());
    
    build_all_packages(&args.workspace, &args.debian_dirs, &args.output_dir, &ros_distro).await?;
    
    Ok(())
}

async fn build_all_packages(
    workspace: &Path,
    debian_dirs: &Path,
    output_dir: &Path,
    ros_distro: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Stage: Scanning
    report_stage("scanning").await?;
    
    report_log("info", "Scanning workspace for ROS packages").await?;
    let scan_result = scan_packages(&workspace.join("src")).await?;
    
    let total_packages = scan_result.packages.len();
    report_log("info", &format!("Found {} packages to build", total_packages)).await?;
    
    if total_packages == 0 {
        report_log("warning", "No packages found to build").await?;
        return Ok(());
    }
    
    // Stage: Building with colcon (handles all dependencies)
    report_stage("colcon_build").await?;
    
    report_log("info", "Building all packages with colcon").await?;
    run_colcon_build(workspace).await?;
    
    // Stage: Preparing Debian directories
    report_stage("preparing").await?;
    
    for (current, package) in scan_result.packages.iter().enumerate() {
        let current = current + 1;
        report_progress(current, total_packages, &format!("Preparing {}", package.name)).await?;
        
        let package_path = workspace.join("src").join(&package.path);
        prepare_debian_dir(&package.name, &package_path, debian_dirs, ros_distro).await?;
    }
    
    // Stage: Creating .deb packages (can be done in parallel)
    report_stage("packaging").await?;
    
    // Create output directory
    fs::create_dir_all(output_dir).await?;
    
    // Build all .deb packages in parallel
    let parallel_jobs = env::var("PARALLEL_JOBS")
        .unwrap_or_else(|_| "4".to_string())
        .parse::<usize>()
        .unwrap_or(4);
    
    // Process packages in chunks to limit parallelism
    let mut all_results = Vec::new();
    for chunk in scan_result.packages.chunks(parallel_jobs) {
        let mut handles = vec![];
        
        for package in chunk {
            let package_name = package.name.clone();
            let package_path = workspace.join("src").join(&package.path);
            let output_dir = output_dir.to_path_buf();
            
            let handle = tokio::spawn(async move {
                report_package_start(&package_name).await?;
                
                match build_single_package(&package_name, &package_path, &output_dir).await {
                    Ok(_) => {
                        report_package_complete(&package_name, true).await?;
                        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
                    }
                    Err(e) => {
                        report_package_complete(&package_name, false).await?;
                        report_log("error", &format!("Build failed for {}: {}", package_name, e)).await?;
                        Err(e)
                    }
                }
            });
            
            handles.push(handle);
        }
        
        // Wait for this chunk to complete
        let results = join_all(handles).await;
        all_results.extend(results);
    }
    
    // Check if any builds failed
    let mut failed_packages = 0;
    for result in all_results {
        if let Ok(Err(_)) = result {
            failed_packages += 1;
        }
    }
    
    if failed_packages > 0 {
        report_log("warning", &format!("{} packages failed to build", failed_packages)).await?;
    }
    
    // Stage: Repository
    report_stage("repository").await?;
    generate_repository(output_dir).await?;
    
    // Stage: Complete
    report_stage("complete").await?;
    report_log("info", "Build completed successfully").await?;
    
    Ok(())
}

async fn scan_packages(src_path: &Path) -> Result<ScanResult, Box<dyn std::error::Error>> {
    let output = Command::new("/helpers/package-scanner.rs")
        .arg(src_path)
        .arg("--format")
        .arg("json")
        .output()?;
        
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to scan packages: {}", stderr).into());
    }
    
    let stdout = String::from_utf8(output.stdout)?;
    let result: ScanResult = serde_json::from_str(&stdout)?;
    
    Ok(result)
}

async fn run_colcon_build(workspace: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let parallel_jobs = env::var("PARALLEL_JOBS").unwrap_or_else(|_| "4".to_string());
    let build_type = env::var("BUILD_TYPE").unwrap_or_else(|_| "Release".to_string());
    
    let mut child = TokioCommand::new("colcon")
        .args(&[
            "build",
            "--merge-install",
            "--parallel-workers", &parallel_jobs,
            "--cmake-args", &format!("-DCMAKE_BUILD_TYPE={}", build_type),
        ])
        .current_dir(workspace)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
        
    // Stream stdout
    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        
        tokio::spawn(async move {
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = report_log("debug", &format!("colcon: {}", line)).await;
            }
        });
    }
    
    // Stream stderr
    if let Some(stderr) = child.stderr.take() {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        
        tokio::spawn(async move {
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = report_log("debug", &format!("colcon: {}", line)).await;
            }
        });
    }
    
    let status = child.wait().await?;
    
    if !status.success() {
        return Err("colcon build failed".into());
    }
    
    Ok(())
}

async fn prepare_debian_dir(
    package_name: &str,
    package_path: &Path,
    debian_dirs: &Path,
    ros_distro: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("/helpers/debian-preparer.rs")
        .args(&[
            package_name,
            &package_path.to_string_lossy(),
            &debian_dirs.to_string_lossy(),
            ros_distro,
        ])
        .output()?;
        
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to prepare debian dir for {}: {}", package_name, stderr).into());
    }
    
    Ok(())
}

async fn build_single_package(
    package_name: &str,
    package_path: &Path,
    output_dir: &Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    report_log("debug", &format!("Building package {} at {:?}", package_name, package_path)).await?;
    
    // Check if debian directory exists
    if !package_path.join("debian").exists() {
        return Err(format!("No debian directory found for package {}", package_name).into());
    }
    
    let mut child = TokioCommand::new("dpkg-buildpackage")
        .args(&["-b", "-uc", "-us"])
        .current_dir(package_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
        
    // Capture output for error reporting
    let mut stdout_buf = Vec::new();
    let mut stderr_buf = Vec::new();
    
    if let Some(stdout) = child.stdout.take() {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        while reader.read_line(&mut line).await? > 0 {
            stdout_buf.push(line.clone());
            let _ = report_log("debug", &format!("{}: {}", package_name, line.trim())).await;
            line.clear();
        }
    }
    
    if let Some(stderr) = child.stderr.take() {
        let mut reader = BufReader::new(stderr);
        let mut line = String::new();
        while reader.read_line(&mut line).await? > 0 {
            stderr_buf.push(line.clone());
            line.clear();
        }
    }
    
    let status = child.wait().await?;
    
    if !status.success() {
        // Log stderr on failure
        for line in &stderr_buf {
            let _ = report_log("error", &format!("{}: {}", package_name, line.trim())).await;
        }
        return Err(format!("dpkg-buildpackage failed for {}", package_name).into());
    }
    
    // Move generated .deb files to output
    move_deb_files(package_path.parent().unwrap(), output_dir).await?;
    
    Ok(())
}

async fn move_deb_files(source_dir: &Path, output_dir: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut entries = fs::read_dir(source_dir).await?;
    let mut moved_count = 0;
    
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if let Some(extension) = path.extension() {
            if extension == "deb" {
                let dest = output_dir.join(entry.file_name());
                fs::rename(&path, &dest).await?;
                moved_count += 1;
            }
        }
    }
    
    if moved_count == 0 {
        return Err("No .deb files generated".into());
    }
    
    Ok(())
}

async fn generate_repository(output_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    report_log("info", "Generating APT repository metadata").await?;
    
    // Call the create-repo.sh script
    let output = Command::new("/scripts/create-repo.sh")
        .arg(output_dir)
        .output()?;
        
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to create repository: {}", stderr).into());
    }
    
    Ok(())
}

// Progress reporting functions
async fn report_stage(stage: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if Path::new("/helpers/progress-reporter.rs").exists() {
        Command::new("/helpers/progress-reporter.rs")
            .args(&["stage", stage])
            .output()?;
    } else {
        eprintln!("::progress::type=stage,value={}", stage);
    }
    Ok(())
}

async fn report_progress(current: usize, total: usize, message: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if Path::new("/helpers/progress-reporter.rs").exists() {
        Command::new("/helpers/progress-reporter.rs")
            .args(&[
                "progress",
                "--current", &current.to_string(),
                "--total", &total.to_string(),
                "--message", message
            ])
            .output()?;
    } else {
        eprintln!("::progress::type=general,current={},total={},message={}", current, total, message);
    }
    Ok(())
}

async fn report_package_start(name: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if Path::new("/helpers/progress-reporter.rs").exists() {
        Command::new("/helpers/progress-reporter.rs")
            .args(&["package-start", name])
            .output()?;
    } else {
        eprintln!("::progress::type=package_start,name={}", name);
    }
    Ok(())
}

async fn report_package_complete(name: &str, success: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if Path::new("/helpers/progress-reporter.rs").exists() {
        Command::new("/helpers/progress-reporter.rs")
            .args(&[
                "package-complete",
                name,
                if success { "--success" } else { "" }
            ].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>())
            .output()?;
    } else {
        eprintln!("::progress::type=package_complete,name={},success={}", name, success);
    }
    Ok(())
}

async fn report_log(level: &str, message: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if Path::new("/helpers/progress-reporter.rs").exists() {
        Command::new("/helpers/progress-reporter.rs")
            .args(&["log", "--level", level, "--message", message])
            .output()?;
    } else {
        eprintln!("::log::level={},msg={}", level, message);
    }
    Ok(())
}