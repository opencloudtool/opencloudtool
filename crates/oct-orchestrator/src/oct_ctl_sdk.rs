/// TODO(#147): Generate this from `oct-ctl`'s `OpenAPI` spec
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// HTTP client to access `oct-ctl`'s API
pub(crate) struct Client {
    // TODO: Use reference instead
    pub(crate) public_ip: String,
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

    pub(crate) fn new(public_ip: String) -> Self {
        Self {
            public_ip,
            port: Self::DEFAULT_PORT,
        }
    }

    pub(crate) async fn run_container(
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

    pub(crate) async fn remove_container(
        &self,
        name: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
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

    pub(crate) async fn health_check(&self) -> Result<(), Box<dyn std::error::Error>> {
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

        let server_mock = server
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
        server_mock.assert();
    }

    #[tokio::test]
    async fn test_run_container_failure() {
        // Arrange
        let (ip, port, mut server) = setup_server().await;

        let server_mock = server
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
        assert!(!response.is_ok());
        server_mock.assert();
    }

    #[tokio::test]
    async fn test_remove_container_success() {
        // Arrange
        let (ip, port, mut server) = setup_server().await;

        let server_mock = server
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
        server_mock.assert();
    }

    #[tokio::test]
    async fn test_remove_container_failure() {
        // Arrange
        let (ip, port, mut server) = setup_server().await;

        let server_mock = server
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
        assert!(!response.is_ok());
        server_mock.assert();
    }
}
