/// TODO: Generate this from `oct-ctl`'s `OpenAPI` spec
use std::collections::HashMap;

/// HTTP client to access `oct-ctl`'s API
pub(crate) struct Client {
    public_ip: String,
    port: u16,
}

impl Client {
    const DEFAULT_PORT: u16 = 31888;

    pub(crate) fn new(public_ip: String, port: Option<u16>) -> Self {
        Self {
            public_ip,
            port: port.unwrap_or(Self::DEFAULT_PORT),
        }
    }

    pub(crate) async fn run_container(
        &self,
        name: String,
        image: String,
        external_port: String,
        internal_port: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();

        let map = HashMap::from([
            ("name", name.as_str()),
            ("image", image.as_str()),
            ("external_port", external_port.as_str()),
            ("internal_port", internal_port.as_str()),
        ]);

        let response = client
            .post(format!(
                "http://{}:{}/run-container",
                self.public_ip, self.port
            ))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .body(serde_json::to_string(&map)?)
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

        let map = HashMap::from([("name", name.as_str())]);

        let response = client
            .post(format!(
                "http://{}:{}/remove-container",
                self.public_ip, self.port
            ))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .body(serde_json::to_string(&map)?)
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

        let (ip, port) = if let std::net::SocketAddr::V4(addr) = server.socket_address() {
            (addr.ip().to_string(), addr.port())
        } else {
            panic!("Server address is not IPv4")
        };

        (ip, port, server)
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

        let client = Client::new(ip, Some(port));

        // Act
        let response = client
            .run_container(
                "test".to_string(),
                "nginx:latest".to_string(),
                "8080".to_string(),
                "80".to_string(),
            )
            .await;

        // Assert
        assert_eq!(response.is_ok(), true);
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

        let client = Client::new(ip, Some(port));

        // Act
        let response = client
            .run_container(
                "test".to_string(),
                "nginx:latest".to_string(),
                "8080".to_string(),
                "80".to_string(),
            )
            .await;

        // Assert
        assert_eq!(response.is_ok(), false);
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

        let client = Client::new(ip, Some(port));

        // Act
        let response = client.remove_container("test".to_string()).await;

        // Assert
        assert_eq!(response.is_ok(), true);
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

        let client = Client::new(ip, Some(port));

        // Act
        let response = client.remove_container("test".to_string()).await;

        // Assert
        assert_eq!(response.is_ok(), false);
        server_mock.assert();
    }
}
