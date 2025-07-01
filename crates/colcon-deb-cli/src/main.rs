//! Main CLI entry point for Colcon Debian Packager

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use color_eyre::eyre::Result;
use tracing_subscriber::EnvFilter;

/// Colcon Debian Packager - Build .deb packages for ROS workspaces
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    /// Increase logging verbosity
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Decrease logging verbosity
    #[arg(short, long, action = clap::ArgAction::Count)]
    quiet: u8,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Build Debian packages from a ROS workspace
    Build {
        /// Path to configuration file
        #[arg(short, long, value_name = "FILE")]
        config: PathBuf,

        /// Override output directory
        #[arg(short, long, value_name = "DIR")]
        output: Option<PathBuf>,

        /// Number of parallel jobs
        #[arg(short = 'j', long, value_name = "N")]
        jobs: Option<usize>,
    },

    /// Validate configuration file
    Validate {
        /// Path to configuration file
        #[arg(short, long, value_name = "FILE")]
        config: PathBuf,
    },

    /// Clean build artifacts
    Clean {
        /// Path to configuration file
        #[arg(short, long, value_name = "FILE")]
        config: Option<PathBuf>,

        /// Clean all artifacts (including cache)
        #[arg(long)]
        all: bool,
    },

    /// Initialize a new configuration file
    Init {
        /// Output path for configuration file
        #[arg(short, long, value_name = "FILE", default_value = "colcon-deb.yaml")]
        output: PathBuf,

        /// Force overwrite existing file
        #[arg(short, long)]
        force: bool,
    },
}

fn main() -> Result<()> {
    // Install color-eyre for better error reports
    color_eyre::install()?;

    // Parse command line arguments
    let cli = Cli::parse();

    // Set up logging
    setup_logging(cli.verbose, cli.quiet)?;

    // Handle commands
    match cli.command {
        Commands::Build { config, output: _, jobs: _ } => {
            tracing::info!("Building Debian packages from {:?}", config);
            // TODO: Implement build command
            eprintln!("Build command not yet implemented");
        }

        Commands::Validate { config } => {
            tracing::info!("Validating configuration file {:?}", config);
            // TODO: Implement validate command
            eprintln!("Validate command not yet implemented");
        }

        Commands::Clean { config: _, all: _ } => {
            tracing::info!("Cleaning build artifacts");
            // TODO: Implement clean command
            eprintln!("Clean command not yet implemented");
        }

        Commands::Init { output, force: _ } => {
            tracing::info!("Initializing configuration file at {:?}", output);
            // TODO: Implement init command
            eprintln!("Init command not yet implemented");
        }
    }

    Ok(())
}

fn setup_logging(verbose: u8, quiet: u8) -> Result<()> {
    let log_level = match (verbose, quiet) {
        (0, 0) => "info",
        (1, 0) => "debug",
        (2, 0) => "trace",
        (v, 0) if v > 2 => "trace",
        (0, 1) => "warn",
        (0, 2) => "error",
        (0, q) if q > 2 => "off",
        _ => "info", // If both are set, default to info
    };

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    Ok(())
}
