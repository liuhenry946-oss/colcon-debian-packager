//! Build orchestration logic

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use colcon_deb_config::Config;
use colcon_deb_docker::{DockerService, DockerServiceTrait};
use tracing::{debug, info, warn};

use crate::{
    artifact::ArtifactCollector,
    context::{BuildContext, BuildState},
    error::{BuildError, Result},
    executor::{BuildExecutor, ExecutorConfig},
};

/// Trait for build orchestration
#[async_trait]
pub trait BuildOrchestratorTrait {
    /// Prepare the build environment
    async fn prepare_environment(&mut self) -> Result<()>;

    /// Run the build process
    async fn run_build(&mut self) -> Result<()>;

    /// Collect build artifacts
    async fn collect_artifacts(&mut self) -> Result<Vec<PathBuf>>;

    /// Get the current build context
    fn context(&self) -> &BuildContext;

    /// Get mutable build context
    fn context_mut(&mut self) -> &mut BuildContext;
}

/// Main build orchestrator implementation
pub struct ColconDebBuilder {
    /// Build configuration
    config: Config,
    /// Build context
    context: BuildContext,
    /// Docker service (wrapped in Arc for sharing)
    docker: Arc<DockerService>,
    /// Build executor
    executor: Option<BuildExecutor>,
    /// Artifact collector
    artifact_collector: ArtifactCollector,
}

impl ColconDebBuilder {
    /// Create a new build orchestrator
    pub async fn new(config: Config) -> Result<Self> {
        let docker = DockerService::new(Default::default())
            .await
            .map_err(BuildError::Docker)?;

        let context = BuildContext::new(config.clone());
        let artifact_collector = ArtifactCollector::new(config.output_dir.clone());

        Ok(Self {
            config,
            context,
            docker: Arc::new(docker),
            executor: None,
            artifact_collector,
        })
    }

    /// Initialize the Docker service
    async fn init_docker(&mut self) -> Result<()> {
        info!("Initializing Docker service");

        // Verify connection by checking if we can list containers
        self.docker
            .list_containers(false)
            .await
            .map_err(BuildError::Docker)?;

        debug!("Docker service initialized successfully");
        Ok(())
    }

    /// Create build executor
    fn create_executor(&mut self) -> Result<()> {
        let executor_config = ExecutorConfig {
            container_image: self.config.docker_image(),
            workspace_path: self.config.colcon_repo.clone(),
            output_dir: self.config.output_dir.clone(),
            parallel_jobs: self.config.parallel_jobs,
            timeout_seconds: None,
        };

        self.executor = Some(BuildExecutor::new(executor_config, Arc::clone(&self.docker)));

        Ok(())
    }

    /// Clean up resources
    async fn cleanup(&mut self) -> Result<()> {
        if let Some(executor) = &mut self.executor {
            executor.cleanup().await?;
        }
        Ok(())
    }
}

#[async_trait]
impl BuildOrchestratorTrait for ColconDebBuilder {
    async fn prepare_environment(&mut self) -> Result<()> {
        info!("Preparing build environment");

        // Update build state
        self.context.set_state(BuildState::Preparing);

        // Initialize Docker
        self.init_docker().await?;

        // Create build executor
        self.create_executor()?;

        // Validate workspace
        if !self.config.colcon_repo.exists() {
            return Err(BuildError::environment(format!(
                "Workspace path does not exist: {}",
                self.config.colcon_repo.display()
            )));
        }

        // Create output directory
        std::fs::create_dir_all(&self.config.output_dir).map_err(|e| {
            BuildError::environment(format!("Failed to create output directory: {e}"))
        })?;

        info!("Build environment prepared successfully");
        self.context.set_state(BuildState::Ready);

        Ok(())
    }

    async fn run_build(&mut self) -> Result<()> {
        info!("Starting build process");

        // Update build state
        self.context.set_state(BuildState::Building);

        // Get executor
        let executor = self
            .executor
            .as_mut()
            .ok_or_else(|| BuildError::InvalidConfiguration {
                reason: "Build executor not initialized".to_string(),
            })?;

        // Run build
        match executor.execute_build(&mut self.context).await {
            Ok(()) => {
                info!("Build completed successfully");
                self.context.set_state(BuildState::Completed);
                Ok(())
            }
            Err(e) => {
                warn!("Build failed: {}", e);
                self.context.set_state(BuildState::Failed);
                Err(e)
            }
        }
    }

    async fn collect_artifacts(&mut self) -> Result<Vec<PathBuf>> {
        info!("Collecting build artifacts");

        // Update build state
        self.context.set_state(BuildState::CollectingArtifacts);

        // Get executor
        let executor = self
            .executor
            .as_ref()
            .ok_or_else(|| BuildError::InvalidConfiguration {
                reason: "Build executor not initialized".to_string(),
            })?;

        // Collect artifacts from container
        let artifacts = self
            .artifact_collector
            .collect_from_executor(executor, &self.context)
            .await?;

        info!("Collected {} artifacts", artifacts.len());

        // Clean up
        self.cleanup().await?;

        Ok(artifacts)
    }

    fn context(&self) -> &BuildContext {
        &self.context
    }

    fn context_mut(&mut self) -> &mut BuildContext {
        &mut self.context
    }
}

/// Legacy build orchestrator for compatibility  
pub struct BuildOrchestrator {
    inner: ColconDebBuilder,
}

impl BuildOrchestrator {
    /// Create a new build orchestrator
    pub async fn new(config: Config) -> Self {
        Self {
            inner: ColconDebBuilder::new(config)
                .await
                .expect("Failed to create builder"),
        }
    }

    /// Run the build process
    pub async fn build(&mut self) -> std::result::Result<(), colcon_deb_core::error::Error> {
        // Prepare environment
        self.inner.prepare_environment().await.map_err(|e| {
            colcon_deb_core::error::Error::BuildFailed {
                package: "workspace".to_string(),
                reason: e.to_string(),
            }
        })?;

        // Run build
        self.inner
            .run_build()
            .await
            .map_err(|e| colcon_deb_core::error::Error::BuildFailed {
                package: "workspace".to_string(),
                reason: e.to_string(),
            })?;

        // Collect artifacts
        self.inner.collect_artifacts().await.map_err(|e| {
            colcon_deb_core::error::Error::BuildFailed {
                package: "workspace".to_string(),
                reason: e.to_string(),
            }
        })?;

        Ok(())
    }
}
