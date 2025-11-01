/// TODO(#147): Generate this from `oct-ctl`'s `OpenAPI` spec
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// HTTP client to access `oct-ctl`'s API
pub struct Client {
    // TODO: Use reference instead
    pub public_ip: String,
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

    /// Creates a `Client` configured for the given public IP and the default API port.
    ///
    /// The returned `Client` has its `public_ip` set to the provided value and its `port` set to `Client::DEFAULT_PORT`.
    ///
    /// # Examples
    ///
    /// ```
    /// let client = Client::new("203.0.113.5".to_string());
    /// assert_eq!(client.public_ip, "203.0.113.5");
    /// ```
    pub fn new(public_ip: String) -> Self {
        Self {
            public_ip,
            port: Self::DEFAULT_PORT,
        }
    }

    /// Requests the oct-ctl API to create and start a container using the specified configuration.
    ///
    /// The function sends a JSON-encoded POST to the client's `/run-container` endpoint and returns
    /// success when the API responds with a successful HTTP status.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the API responds with a successful HTTP status, `Err` containing the underlying error otherwise.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn doc() -> Result<(), Box<dyn std::error::Error>> {
    /// use std::collections::HashMap;
    /// let client = crate::Client::new("127.0.0.1".to_string());
    /// client.run_container(
    ///     "my-container".to_string(),
    ///     "alpine:latest".to_string(),
    ///     Some("sleep 60".to_string()),
    ///     None,
    ///     None,
    ///     1,
    ///     64,
    ///     HashMap::new(),
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
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

    /// Requests the oct-ctl API to remove a container identified by `name`.
    ///
    /// Sends a JSON POST to the API's `/remove-container` endpoint and returns an error if the API responds with a non-success status.
    ///
    /// # Examples
    ///
    /// ```
    /// # use oct_ctl_sdk::Client;
    /// # tokio_test::block_on(async {
    /// let client = Client::new("127.0.0.1".into());
    /// // Remove a container named "my-container"
    /// client.remove_container("my-container".to_string()).await.unwrap();
    /// # });
    /// ```
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, `Err` with a boxed error when the request fails or the server returns a non-success status.
    pub async fn remove_container(&self, name: String) -> Result<(), Box<dyn std::error::Error>> {
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

    /// Performs a health check against the configured oct-ctl server.
    ///
    /// This sends an HTTP GET to `/health-check` on the client's `public_ip` and port,
    /// and returns an error if the request fails, times out (5 seconds), or the server
    /// responds with a non-success status code.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::new("127.0.0.1".to_string());
    /// client.health_check().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn health_check(&self) -> Result<(), Box<dyn std::error::Error>> {
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
        assert!(response.is_err());
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
        assert!(response.is_err());
        server_mock.assert();
    }
}