#!/usr/bin/env rust-script
//! ```cargo
//! [dependencies]
//! clap = { version = "4.0", features = ["derive"] }
//! tokio = { version = "1.0", features = ["process", "fs", "rt", "macros"] }
//! serde_json = "1.0"
//! walkdir = "2.0"
//! regex = "1.0"
//! chrono = { version = "0.4", features = ["serde"] }
//! ```

//! Debian directory preparer for ROS packages
//!
//! This script manages debian directories by:
//! 1. Checking for existing custom debian directories
//! 2. Calling bloom-generate for missing directories
//! 3. Validating debian directory structure
//! 4. Saving generated directories to the collection

use clap::Parser;
use regex::Regex;
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(name = "debian-preparer")]
#[command(about = "Prepare debian directories for ROS packages")]
struct Args {
    /// Package name
    #[arg(short, long)]
    package_name: String,
    
    /// Package source path
    #[arg(short = 's', long)]
    package_path: PathBuf,
    
    /// Package version
    #[arg(short = 'v', long)]
    package_version: String,
    
    /// Debian directories collection path
    #[arg(short, long)]
    debian_dirs: PathBuf,
    
    /// ROS distribution
    #[arg(short, long, default_value = "humble")]
    ros_distro: String,
    
    /// Maintainer information
    #[arg(short, long)]
    maintainer: Option<String>,
    
    /// Whether to use bloom-generate for missing directories
    #[arg(long, default_value = "true")]
    use_bloom: bool,
    
    /// Force regeneration even if custom directory exists
    #[arg(long, default_value = "false")]
    force_regenerate: bool,
    
    /// Validate debian directory after preparation
    #[arg(long, default_value = "true")]
    validate: bool,
    
    /// JSON output for structured results
    #[arg(long, default_value = "false")]
    json_output: bool,
}

#[derive(Debug)]
struct PreparationResult {
    package_name: String,
    used_custom: bool,
    used_bloom: bool,
    validation_passed: bool,
    debian_path: PathBuf,
    warnings: Vec<String>,
    errors: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    let result = prepare_debian_directory(&args).await?;
    
    if args.json_output {
        output_json_result(&result).await?;
    } else {
        output_text_result(&result).await?;
    }
    
    // Exit with error if validation failed
    if args.validate && !result.validation_passed {
        std::process::exit(1);
    }
    
    Ok(())
}

async fn prepare_debian_directory(args: &Args) -> Result<PreparationResult, Box<dyn std::error::Error>> {
    let mut result = PreparationResult {
        package_name: args.package_name.clone(),
        used_custom: false,
        used_bloom: false,
        validation_passed: false,
        debian_path: args.package_path.join("debian"),
        warnings: Vec::new(),
        errors: Vec::new(),
    };
    
    report_progress("stage", &format!("Preparing debian directory for {}", args.package_name)).await?;
    
    let custom_debian_path = args.debian_dirs.join(&args.package_name).join("debian");
    let target_debian_path = &result.debian_path;
    
    // Remove existing target debian directory
    if target_debian_path.exists() {
        fs::remove_dir_all(target_debian_path)?;
    }
    
    // Check if custom debian directory exists and not forced to regenerate
    if custom_debian_path.exists() && !args.force_regenerate {
        report_log("info", &format!("Using custom debian directory for {}", args.package_name)).await?;
        
        copy_directory_recursive(&custom_debian_path, target_debian_path)?;
        result.used_custom = true;
        
        // Update version in changelog if needed
        update_changelog_version(target_debian_path, &args.package_name, &args.package_version, &args.ros_distro).await?;
        
    } else if args.use_bloom {
        report_log("info", &format!("Generating debian directory with bloom-generate for {}", args.package_name)).await?;
        
        generate_with_bloom(args, target_debian_path, &mut result).await?;
        result.used_bloom = true;
        
        // Save generated directory to collection for future use
        if target_debian_path.exists() {
            save_generated_directory(target_debian_path, &custom_debian_path, &mut result).await?;
        }
    } else {
        result.errors.push("No custom debian directory found and bloom-generate disabled".to_string());
        return Ok(result);
    }
    
    // Validate debian directory if requested
    if args.validate {
        result.validation_passed = validate_debian_directory(target_debian_path, &args.package_name, &mut result).await?;
    } else {
        result.validation_passed = true;
    }
    
    Ok(result)
}

async fn generate_with_bloom(args: &Args, target_debian_path: &Path, result: &mut PreparationResult) -> Result<(), Box<dyn std::error::Error>> {
    let debian_version = convert_ros_to_debian_version(&args.package_version)?;
    
    // Build bloom-generate command
    let mut cmd = Command::new("bloom-generate");
    cmd.arg("debian")
        .arg("--package-name")
        .arg(&args.package_name)
        .arg("--package-version")
        .arg(&debian_version)
        .arg("--ros-distro")
        .arg(&args.ros_distro)
        .arg("--os-name")
        .arg("ubuntu")
        .arg("--os-version")
        .arg(get_ubuntu_version(&args.ros_distro))
        .current_dir(&args.package_path);
    
    // Add maintainer if provided
    if let Some(maintainer) = &args.maintainer {
        cmd.arg("--maintainer").arg(maintainer);
    }
    
    report_log("debug", &format!("Running bloom-generate: {:?}", cmd)).await?;
    
    let output = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output()?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        result.errors.push(format!("bloom-generate failed: {}", stderr));
        return Err(format!("bloom-generate failed for {}: {}", args.package_name, stderr).into());
    }
    
    if !target_debian_path.exists() {
        result.errors.push("bloom-generate did not create debian directory".to_string());
        return Err("bloom-generate did not create debian directory".into());
    }
    
    report_log("info", &format!("Successfully generated debian directory for {}", args.package_name)).await?;
    Ok(())
}

async fn save_generated_directory(source: &Path, target: &Path, result: &mut PreparationResult) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    
    match copy_directory_recursive(source, target) {
        Ok(_) => {
            report_log("info", &format!("Saved generated debian directory for {}", result.package_name)).await?;
        }
        Err(e) => {
            let warning = format!("Failed to save generated debian directory: {}", e);
            result.warnings.push(warning.clone());
            report_log("warning", &warning).await?;
        }
    }
    
    Ok(())
}

async fn validate_debian_directory(debian_path: &Path, package_name: &str, result: &mut PreparationResult) -> Result<bool, Box<dyn std::error::Error>> {
    report_log("debug", &format!("Validating debian directory for {}", package_name)).await?;
    
    if !debian_path.exists() {
        result.errors.push("Debian directory does not exist".to_string());
        return Ok(false);
    }
    
    let required_files = ["control", "changelog", "copyright", "rules"];
    let mut missing_files = Vec::new();
    let mut validation_passed = true;
    
    // Check required files
    for &file in &required_files {
        let file_path = debian_path.join(file);
        if !file_path.exists() {
            missing_files.push(file.to_string());
            validation_passed = false;
        } else {
            // Validate specific files
            match file {
                "control" => validate_control_file(&file_path, package_name, result).await?,
                "rules" => validate_rules_file(&file_path, result).await?,
                _ => {}
            }
        }
    }
    
    if !missing_files.is_empty() {
        result.errors.push(format!("Missing required files: {}", missing_files.join(", ")));
        validation_passed = false;
    }
    
    // Check for common issues
    check_file_permissions(debian_path, result).await?;
    
    if validation_passed {
        report_log("info", &format!("Debian directory validation passed for {}", package_name)).await?;
    } else {
        report_log("error", &format!("Debian directory validation failed for {}", package_name)).await?;
    }
    
    Ok(validation_passed)
}

async fn validate_control_file(control_path: &Path, package_name: &str, result: &mut PreparationResult) -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read_to_string(control_path)?;
    
    let required_fields = ["Package", "Architecture", "Maintainer", "Description"];
    for field in &required_fields {
        if !content.contains(&format!("{}:", field)) {
            result.warnings.push(format!("Missing {} field in control file", field));
        }
    }
    
    // Check package name matches
    if let Some(pkg_line) = content.lines().find(|line| line.starts_with("Package:")) {
        let pkg_name = pkg_line.split(':').nth(1).map(|s| s.trim());
        if pkg_name != Some(package_name) {
            result.warnings.push(format!("Package name in control ({:?}) doesn't match expected ({})", pkg_name, package_name));
        }
    }
    
    Ok(())
}

async fn validate_rules_file(rules_path: &Path, result: &mut PreparationResult) -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read_to_string(rules_path)?;
    
    if !content.starts_with("#!") {
        result.warnings.push("Rules file missing shebang".to_string());
    }
    
    let required_targets = ["build", "binary", "clean"];
    for target in &required_targets {
        if !content.contains(&format!("{}:", target)) {
            result.warnings.push(format!("Rules file missing {} target", target));
        }
    }
    
    Ok(())
}

async fn check_file_permissions(debian_path: &Path, result: &mut PreparationResult) -> Result<(), Box<dyn std::error::Error>> {
    let rules_path = debian_path.join("rules");
    if rules_path.exists() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = fs::metadata(&rules_path)?;
            if metadata.permissions().mode() & 0o111 == 0 {
                result.warnings.push("Rules file is not executable".to_string());
            }
        }
    }
    
    Ok(())
}

async fn update_changelog_version(debian_path: &Path, package_name: &str, version: &str, ros_distro: &str) -> Result<(), Box<dyn std::error::Error>> {
    let changelog_path = debian_path.join("changelog");
    if !changelog_path.exists() {
        return Ok(());
    }
    
    let content = fs::read_to_string(&changelog_path)?;
    let debian_version = convert_ros_to_debian_version(version)?;
    
    // Check if version needs updating
    if content.contains(&debian_version) {
        return Ok(());
    }
    
    // Create new changelog entry
    let date = chrono::Utc::now().format("%a, %d %b %Y %H:%M:%S +0000");
    let new_entry = format!(
        "{} ({}) {}; urgency=medium\n\n  * Automated build for ROS {}\n\n -- Automated Build <noreply@ros.org>  {}\n\n{}",
        package_name, debian_version, ros_distro, ros_distro, date, content
    );
    
    fs::write(&changelog_path, new_entry)?;
    Ok(())
}

fn convert_ros_to_debian_version(ros_version: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Convert ROS version to Debian format
    let version_regex = Regex::new(r"^(\d+)\.(\d+)\.(\d+)(?:\.(\d+))?(?:[-.]?(alpha|beta|rc|dev)(\d+)?)?$")?;
    
    let captures = version_regex.captures(ros_version)
        .ok_or_else(|| format!("Invalid version format: {}", ros_version))?;
    
    let major = captures.get(1).unwrap().as_str();
    let minor = captures.get(2).unwrap().as_str();
    let patch = captures.get(3).unwrap().as_str();
    let build = captures.get(4).map(|m| m.as_str());
    let prerelease_type = captures.get(5).map(|m| m.as_str());
    let prerelease_num = captures.get(6).map(|m| m.as_str());
    
    let mut debian_version = if let Some(build) = build {
        format!("{}.{}.{}.{}", major, minor, patch, build)
    } else {
        format!("{}.{}.{}", major, minor, patch)
    };
    
    // Handle pre-release versions
    if let Some(pre_type) = prerelease_type {
        let pre_suffix = match pre_type {
            "alpha" => "~alpha",
            "beta" => "~beta",
            "rc" => "~rc", 
            "dev" => "~dev",
            _ => return Err(format!("Unknown pre-release type: {}", pre_type).into()),
        };
        
        if let Some(pre_num) = prerelease_num {
            debian_version.push_str(&format!("{}{}", pre_suffix, pre_num));
        } else {
            debian_version.push_str(pre_suffix);
        }
    }
    
    // Add Debian revision
    debian_version.push_str("-1");
    
    Ok(debian_version)
}

fn get_ubuntu_version(ros_distro: &str) -> &'static str {
    match ros_distro {
        "humble" | "iron" => "jammy",
        "jazzy" => "noble",
        "rolling" => "noble",
        _ => "jammy", // Default fallback
    }
}

fn copy_directory_recursive(source: &Path, target: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(target)?;
    
    for entry in WalkDir::new(source).into_iter().filter_map(|e| e.ok()) {
        let source_path = entry.path();
        let relative_path = source_path.strip_prefix(source)?;
        let target_path = target.join(relative_path);
        
        if source_path.is_dir() {
            fs::create_dir_all(&target_path)?;
        } else {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(source_path, &target_path)?;
        }
    }
    
    Ok(())
}

async fn output_json_result(result: &PreparationResult) -> Result<(), Box<dyn std::error::Error>> {
    let json_result = json!({
        "package_name": result.package_name,
        "used_custom": result.used_custom,
        "used_bloom": result.used_bloom,
        "validation_passed": result.validation_passed,
        "debian_path": result.debian_path,
        "warnings": result.warnings,
        "errors": result.errors,
    });
    
    println!("{}", serde_json::to_string_pretty(&json_result)?);
    Ok(())
}

async fn output_text_result(result: &PreparationResult) -> Result<(), Box<dyn std::error::Error>> {
    println!("Debian preparation result for {}", result.package_name);
    println!("  Used custom directory: {}", result.used_custom);
    println!("  Used bloom-generate: {}", result.used_bloom);
    println!("  Validation passed: {}", result.validation_passed);
    println!("  Debian path: {}", result.debian_path.display());
    
    if !result.warnings.is_empty() {
        println!("  Warnings:");
        for warning in &result.warnings {
            println!("    - {}", warning);
        }
    }
    
    if !result.errors.is_empty() {
        println!("  Errors:");
        for error in &result.errors {
            println!("    - {}", error);
        }
    }
    
    Ok(())
}

async fn report_progress(event_type: &str, message: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("::progress::type={},msg={}", event_type, message);
    Ok(())
}

async fn report_log(level: &str, message: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("::log::level={},msg={}", level, message);
    Ok(())
}