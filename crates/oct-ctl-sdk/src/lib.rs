/// TODO(#147): Generate this from `oct-ctl`'s `OpenAPI` spec
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// HTTP client to access `oct-ctl`'s API
pub struct Client {
    public_ip: String,
    port: u16,
}

#[derive(Debug, Serialize, Deserialize)]
struct RunContainerRequest {
    name: String,
    image: String,
    command: Option<String>,
    external_port: Option<u32>,
    internal_port: Option<u32>,
    cpus: u32,
    memory: u64,
    envs: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RemoveContainerRequest {
    name: String,
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

    pub async fn run_container(
        &self,
        name: String,
        image: String,
        command: Option<String>,
        external_port: Option<u32>,
        internal_port: Option<u32>,
        cpus: u32,
        memory: u64,
        envs: HashMap<String, String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let () = self.check_host_health().await?;

        let client = reqwest::Client::new();

        let request = RunContainerRequest {
            name,
            image,
            command,
            external_port,
            internal_port,
            cpus,
            memory,
            envs,
        };

        let response = client
            .post(format!(
                "http://{}:{}/run-container",
                self.public_ip, self.port
            ))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .body(serde_json::to_string(&request)?)
            .send()
            .await?;

        match response.error_for_status() {
            Ok(_) => Ok(()),
            Err(e) => Err(Box::new(e)),
        }
    }

    pub async fn remove_container(&self, name: String) -> Result<(), Box<dyn std::error::Error>> {
        let () = self.check_host_health().await?;

        let client = reqwest::Client::new();

        let request = RemoveContainerRequest { name };

        let response = client
            .post(format!(
                "http://{}:{}/remove-container",
                self.public_ip, self.port
            ))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .body(serde_json::to_string(&request)?)
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

    async fn setup_server() -> (String, u16, mockito::ServerGuard) {
        let server = mockito::Server::new_async().await;

        let addr = server.socket_address();

        (addr.ip().to_string(), addr.port(), server)
    }

    #[tokio::test]
    async fn test_run_container_success() {
        // Arrange
        let (ip, port, mut server) = setup_server().await;

        let health_check_mock = server
            .mock("GET", "/health-check")
            .with_status(200)
            .create();

        let run_container_mock = server
            .mock("POST", "/run-container")
            .with_status(201)
            .match_header("Content-Type", "application/json")
            .match_header("Accept", "application/json")
            .create();

        let client = Client {
            public_ip: ip,
            port,
        };

        // Act
        let response = client
            .run_container(
                "test".to_string(),
                "nginx:latest".to_string(),
                Some("echo hello".to_string()),
                Some(8080),
                Some(80),
                250,
                64,
                HashMap::new(),
            )
            .await;

        // Assert
        assert!(response.is_ok());

        health_check_mock.assert();
        run_container_mock.assert();
    }

    #[tokio::test]
    async fn test_run_container_failure() {
        // Arrange
        let (ip, port, mut server) = setup_server().await;

        let health_check_mock = server
            .mock("GET", "/health-check")
            .with_status(200)
            .create();

        let run_container_mock = server
            .mock("POST", "/run-container")
            .with_status(500)
            .match_header("Content-Type", "application/json")
            .match_header("Accept", "application/json")
            .create();

        let client = Client {
            public_ip: ip,
            port,
        };

        // Act
        let response = client
            .run_container(
                "test".to_string(),
                "nginx:latest".to_string(),
                None,
                Some(8080),
                Some(80),
                250,
                64,
                HashMap::new(),
            )
            .await;

        // Assert
        assert!(response.is_err());

        health_check_mock.assert();
        run_container_mock.assert();
    }

    #[tokio::test]
    async fn test_remove_container_success() {
        // Arrange
        let (ip, port, mut server) = setup_server().await;

        let health_check_mock = server
            .mock("GET", "/health-check")
            .with_status(200)
            .create();

        let remove_container_mock = server
            .mock("POST", "/remove-container")
            .with_status(200)
            .match_header("Content-Type", "application/json")
            .match_header("Accept", "application/json")
            .create();

        let client = Client {
            public_ip: ip,
            port,
        };

        // Act
        let response = client.remove_container("test".to_string()).await;

        // Assert
        assert!(response.is_ok());

        health_check_mock.assert();
        remove_container_mock.assert();
    }

    #[tokio::test]
    async fn test_remove_container_failure() {
        // Arrange
        let (ip, port, mut server) = setup_server().await;

        let health_check_mock = server
            .mock("GET", "/health-check")
            .with_status(200)
            .create();

        let remove_container_mock = server
            .mock("POST", "/remove-container")
            .with_status(500)
            .match_header("Content-Type", "application/json")
            .match_header("Accept", "application/json")
            .create();

        let client = Client {
            public_ip: ip,
            port,
        };

        // Act
        let response = client.remove_container("test".to_string()).await;

        // Assert
        assert!(response.is_err());

        health_check_mock.assert();
        remove_container_mock.assert();
    }
}
