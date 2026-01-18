/// TODO(#147): Generate this from `oct-ctl`'s `OpenAPI` spec
use serde::{Deserialize, Serialize};

use oct_config::Config;

/// HTTP client to access `oct-ctl`'s API
pub struct Client {
    public_ip: String,
    port: u16,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApplyRequest {
    config: Config,
}

impl Client {
    const DEFAULT_PORT: u16 = 31888;

    pub fn new(public_ip: String) -> Self {
        Self {
            public_ip,
            port: Self::DEFAULT_PORT,
        }
    }

    pub fn public_ip(&self) -> &str {
        &self.public_ip
    }

    pub async fn apply(&self, config: Config) -> Result<(), Box<dyn std::error::Error>> {
        let () = self.check_host_health().await?;

        let client = reqwest::Client::new();

        let request = ApplyRequest { config };

        let response = client
            .post(format!("http://{}:{}/apply", self.public_ip, self.port))
            .header("Accept", "application/json")
            .json(&request)
            .send()
            .await?;

        match response.error_for_status() {
            Ok(_) => Ok(()),
            Err(e) => Err(Box::new(e)),
        }
    }

    pub async fn destroy(&self) -> Result<(), Box<dyn std::error::Error>> {
        let () = self.check_host_health().await?;

        let client = reqwest::Client::new();

        let response = client
            .post(format!("http://{}:{}/destroy", self.public_ip, self.port))
            .header("Accept", "application/json")
            .send()
            .await?;

        match response.error_for_status() {
            Ok(_) => Ok(()),
            Err(e) => Err(Box::new(e)),
        }
    }

    async fn check_host_health(&self) -> Result<(), Box<dyn std::error::Error>> {
        let max_tries = 24;
        let sleep_duration_s = 5;

        log::info!("Waiting for host '{}' to be ready", self.public_ip);

        let mut is_healthy = false;
        for _ in 0..max_tries {
            is_healthy = match self.health_check().await {
                Ok(()) => {
                    log::info!("Host '{}' is ready", self.public_ip);

                    true
                }
                Err(err) => {
                    log::info!("Host '{}' responded with error: {}", self.public_ip, err);

                    false
                }
            };

            if is_healthy {
                break;
            }

            log::info!("Retrying in {sleep_duration_s} sec...");

            tokio::time::sleep(std::time::Duration::from_secs(sleep_duration_s)).await;
        }

        if is_healthy {
            Ok(())
        } else {
            Err(format!(
                "Host '{}' failed to become ready after max retries",
                self.public_ip
            )
            .into())
        }
    }

    async fn health_check(&self) -> Result<(), Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();

        let response = client
            .get(format!(
                "http://{}:{}/health-check",
                self.public_ip, self.port
            ))
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?;

        match response.error_for_status() {
            Ok(_) => Ok(()),
            Err(e) => Err(Box::new(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oct_config::{Project, StateBackend};

    async fn setup_server() -> (String, u16, mockito::ServerGuard) {
        let server = mockito::Server::new_async().await;

        let addr = server.socket_address();

        (addr.ip().to_string(), addr.port(), server)
    }

    #[tokio::test]
    async fn test_apply_success() {
        // Arrange
        let (ip, port, mut server) = setup_server().await;

        let health_check_mock = server
            .mock("GET", "/health-check")
            .with_status(200)
            .create();

        let apply_mock = server
            .mock("POST", "/apply")
            .with_status(201)
            .match_header("Content-Type", "application/json")
            .match_header("Accept", "application/json")
            .create();

        let client = Client {
            public_ip: ip,
            port,
        };

        let config = Config {
            project: Project {
                name: "test".to_string(),
                state_backend: StateBackend::Local {
                    path: "state.json".to_string(),
                },
                user_state_backend: StateBackend::Local {
                    path: "user_state.json".to_string(),
                },
                services: Vec::new(),
                domain: None,
            },
        };

        // Act
        let response = client.apply(config).await;

        // Assert
        assert!(response.is_ok());

        health_check_mock.assert();
        apply_mock.assert();
    }

    #[tokio::test]
    async fn test_destroy_success() {
        // Arrange
        let (ip, port, mut server) = setup_server().await;

        let health_check_mock = server
            .mock("GET", "/health-check")
            .with_status(200)
            .create();

        let destroy_mock = server
            .mock("POST", "/destroy")
            .with_status(200)
            .match_header("Accept", "application/json")
            .create();

        let client = Client {
            public_ip: ip,
            port,
        };

        // Act
        let response = client.destroy().await;

        // Assert
        assert!(response.is_ok());

        health_check_mock.assert();
        destroy_mock.assert();
    }
}
