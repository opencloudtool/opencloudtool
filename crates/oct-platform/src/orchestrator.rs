use async_trait::async_trait;

#[async_trait]
pub trait Orchestrator: Send + Sync {
    async fn genesis(
        &self,
        config: &oct_config::Config,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    async fn apply(
        &self,
        config: &oct_config::Config,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    async fn destroy(
        &self,
        config: &oct_config::Config,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

pub struct RealOrchestrator;

#[async_trait]
impl Orchestrator for RealOrchestrator {
    async fn genesis(
        &self,
        config: &oct_config::Config,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        oct_orchestrator::OrchestratorWithGraph
            .genesis(config)
            .await
    }

    async fn apply(
        &self,
        config: &oct_config::Config,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        oct_orchestrator::OrchestratorWithGraph.apply(config).await
    }

    async fn destroy(
        &self,
        config: &oct_config::Config,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        oct_orchestrator::OrchestratorWithGraph
            .destroy(config)
            .await
    }
}

pub struct MockOrchestrator {
    genesis: Result<(), String>,
    apply: Result<(), String>,
    destroy: Result<(), String>,
}

impl Default for MockOrchestrator {
    fn default() -> Self {
        Self {
            genesis: Ok(()),
            apply: Ok(()),
            destroy: Ok(()),
        }
    }
}

#[async_trait]
impl Orchestrator for MockOrchestrator {
    async fn genesis(
        &self,
        _config: &oct_config::Config,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.genesis.clone().map_err(std::convert::Into::into)
    }
    async fn apply(
        &self,
        _config: &oct_config::Config,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.apply.clone().map_err(std::convert::Into::into)
    }
    async fn destroy(
        &self,
        _config: &oct_config::Config,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.destroy.clone().map_err(std::convert::Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oct_config::{Config, Project, StateBackend};

    fn create_test_config() -> Config {
        Config {
            project: Project {
                name: "test".to_string(),
                state_backend: StateBackend::Local {
                    path: "state.json".to_string(),
                },
                user_state_backend: StateBackend::Local {
                    path: "user_state.json".to_string(),
                },
                services: vec![],
                domain: None,
            },
        }
    }

    #[tokio::test]
    async fn test_mock_orchestrator_success() {
        // Arrange
        let orchestrator = MockOrchestrator::default();
        let config = create_test_config();

        // Act
        let genesis_res = orchestrator.genesis(&config).await;
        let apply_res = orchestrator.apply(&config).await;
        let destroy_res = orchestrator.destroy(&config).await;

        // Assert
        assert!(genesis_res.is_ok());
        assert!(apply_res.is_ok());
        assert!(destroy_res.is_ok());
    }

    #[tokio::test]
    async fn test_mock_orchestrator_failure() {
        // Arrange
        let orchestrator = MockOrchestrator {
            genesis: Err("genesis failed".to_string()),
            apply: Ok(()),
            destroy: Ok(()),
        };
        let config = create_test_config();

        // Act
        let res = orchestrator.genesis(&config).await;

        // Assert
        assert!(res.is_err());
        assert_eq!(
            res.expect_err("Should be error").to_string(),
            "genesis failed"
        );
    }
}
