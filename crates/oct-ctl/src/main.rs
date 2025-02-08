use std::collections::HashMap;
use std::fs::OpenOptions;
use std::process::Command;

use axum::{
    extract::State, http::StatusCode, response::IntoResponse, routing::get, routing::post, Json,
    Router,
};
use serde::{Deserialize, Serialize};
use tower_http::trace::{self, TraceLayer};

#[cfg(test)]
use mockall::mock;

#[derive(Serialize, Deserialize)]
struct RunContainerPayload {
    /// Name of the container
    name: String,
    /// Image to use for the container
    image: String,
    /// External container port
    external_port: Option<u32>,
    /// Internal container port
    internal_port: Option<u32>,
    /// CPU millicores
    cpus: u32,
    /// Memory in MB
    memory: u64,
    /// Environment variables
    envs: HashMap<String, String>,
}

#[derive(Serialize, Deserialize)]
struct RemoveContainerPayload {
    /// Name of the container
    name: String,
}

/// Container manager options
#[derive(Clone, Default)]
enum ContainerManager {
    #[default]
    Podman,
}

impl ContainerManager {
    fn as_str(&self) -> &'static str {
        match self {
            ContainerManager::Podman => "podman",
        }
    }
}

/// Container engine implementation
#[derive(Clone, Default)]
struct ContainerEngine {
    manager: ContainerManager,
}

#[cfg_attr(test, allow(dead_code))]
impl ContainerEngine {
    const NETWORK_NAME: &str = "oct";

    /// Runs container using `podman`
    fn run(
        &self,
        name: &str,
        image: &str,
        external_port: Option<u32>,
        internal_port: Option<u32>,
        cpus: u32,
        memory: u64,
        envs: &HashMap<String, String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let network_create_cmd = Command::new(self.manager.as_str())
            .args(["network", "create", Self::NETWORK_NAME])
            .output()?;

        log::info!(
            "Network create command output: status={:?}, stdout={:?}, stderr={:?}",
            network_create_cmd.status,
            network_create_cmd.stdout,
            network_create_cmd.stderr
        );

        let run_container_args = Self::build_run_container_args(
            name,
            image,
            external_port,
            internal_port,
            cpus,
            memory,
            envs,
        );

        let run_container_cmd = Command::new(self.manager.as_str())
            .args(&run_container_args)
            .output()?;

        log::info!(
            "Run container command output: status={:?}, stdout={:?}, stderr={:?}",
            run_container_cmd.status,
            run_container_cmd.stdout,
            run_container_cmd.stderr
        );

        Ok(())
    }

    /// Removes container
    fn remove(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let command = Command::new(self.manager.as_str())
            .args(["rm", "-f", name])
            .output();

        match command {
            Ok(_) => Ok(()),
            Err(err) => Err(Box::new(err)),
        }
    }

    fn build_run_container_args(
        name: &str,
        image: &str,
        external_port: Option<u32>,
        internal_port: Option<u32>,
        cpus: u32,
        memory: u64,
        envs: &HashMap<String, String>,
    ) -> Vec<String> {
        let cpus = f64::from(cpus) / 1000.0; // Convert millicores to cores
        let cpus_str = format!("{cpus:.2}");
        let memory_str = format!("{memory}m");

        let mut run_container_args = vec![
            "run".to_string(),
            "--restart".to_string(),
            "always".to_string(),
            "-d".to_string(),
            "--name".to_string(),
            name.to_string(),
            "--cpus".to_string(),
            cpus_str,
            "--memory".to_string(),
            memory_str,
            "--network".to_string(),
            Self::NETWORK_NAME.to_string(),
        ];

        if let (Some(external_port), Some(internal_port)) = (external_port, internal_port) {
            let port_mapping = format!("{external_port}:{internal_port}");

            run_container_args.push("-p".to_string());
            run_container_args.push(port_mapping);
        }

        for (key, value) in envs {
            let env_str = format!("{key}={value}");
            run_container_args.push("-e".to_string());
            run_container_args.push(env_str);
        }

        run_container_args.push(image.to_string());

        run_container_args
    }
}

// As long as ContainerEngine implemnts Clone, we mock it using
// mockall::mock macro, more info here:
// https://docs.rs/mockall/latest/mockall/macro.mock.html#examples
#[cfg(test)]
mock! {
    pub ContainerEngine {
        fn run(
            &self,
            name: &str,
            image: &str,
            external_port: Option<u32>,
            internal_port: Option<u32>,
            cpus: u32,
            memory: u64,
            envs: &HashMap<String, String>,
        ) -> Result<(), Box<dyn std::error::Error>>;

        fn remove(&self, name: &str) -> Result<(), Box<dyn std::error::Error>>;
    }

    impl Clone for ContainerEngine {
        fn clone(&self) -> Self;
    }
}

#[cfg(not(test))]
use ContainerEngine as ContainerEngineImpl;
#[cfg(test)]
use MockContainerEngine as ContainerEngineImpl;

/// Server config passed as a state to the endpoints.
/// It is used as a Dependency Injection container.
#[derive(Clone)]
struct ServerConfig {
    container_engine: ContainerEngineImpl,
}

/// Run container endpoint definition for Axum
async fn run(
    State(server_config): State<ServerConfig>,
    Json(payload): Json<RunContainerPayload>,
) -> impl IntoResponse {
    let run_result = server_config.container_engine.run(
        payload.name.as_str(),
        payload.image.as_str(),
        payload.external_port,
        payload.internal_port,
        payload.cpus,
        payload.memory,
        &payload.envs,
    );

    match run_result {
        Ok(()) => {
            log::info!("Created container: {}", &payload.name);
            (StatusCode::CREATED, "Success")
        }
        Err(err) => {
            log::error!("Failed to create container: {err}");
            (StatusCode::BAD_REQUEST, "Error")
        }
    }
}

/// Remove container endpoint definition for Axum
async fn remove(
    State(server_config): State<ServerConfig>,
    Json(payload): Json<RemoveContainerPayload>,
) -> impl IntoResponse {
    let command = server_config.container_engine.remove(payload.name.as_str());

    match command {
        Ok(()) => {
            log::info!("Removed container: {}", &payload.name);
            (StatusCode::OK, "Success")
        }
        Err(err) => {
            log::error!("Failed to remove container: {err}");
            (StatusCode::BAD_REQUEST, "Error")
        }
    }
}

/// Health endpoint definition for Axum
async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "Success")
}

#[tokio::main]
async fn main() {
    let log_file = OpenOptions::new()
        .append(true)
        .create(true)
        .open("/var/log/oct-ctl.log")
        .expect("Failed to open log file");

    // Tracing initialization code was inspired by
    // https://github.com/tower-rs/tower-http/issues/296#issuecomment-1301108593
    tracing_subscriber::fmt().with_writer(log_file).init();

    let server_config = ServerConfig {
        container_engine: ContainerEngineImpl::default(),
    };

    let app = Router::new()
        .route("/run-container", post(run))
        .route("/remove-container", post(remove))
        .route("/health-check", get(health_check))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(trace::DefaultMakeSpan::new().level(tracing::Level::INFO))
                .on_response(trace::DefaultOnResponse::new().level(tracing::Level::INFO)),
        )
        .with_state(server_config);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:31888")
        .await
        .expect("Failed to bind listener to 0.0.0.0:31888");

    tracing::info!(
        "Listening on {}",
        listener
            .local_addr()
            .expect("Failed to get listener address")
    );

    axum::serve(listener, app)
        .await
        .expect("Failed to start server");
}

// TODO: Use parametrization and fixtures from
//     https://github.com/la10736/rstest
// TODO: Add integration tests
#[cfg(test)]
mod tests {
    use super::*;

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::Router;
    use tower::ServiceExt;

    fn get_container_engine_mock(is_ok: bool) -> ContainerEngineImpl {
        let mut container_engine_mock = ContainerEngineImpl::default();
        container_engine_mock
            .expect_run()
            .returning(
                move |_, _, _, _, _, _, _| {
                    if is_ok {
                        Ok(())
                    } else {
                        Err("error".into())
                    }
                },
            );

        container_engine_mock.expect_remove().returning(move |_| {
            if is_ok {
                Ok(())
            } else {
                Err("error".into())
            }
        });

        container_engine_mock
            .expect_clone()
            .returning(move || get_container_engine_mock(is_ok));

        container_engine_mock
    }

    #[tokio::test]
    async fn test_run_container_success() {
        let server_config = ServerConfig {
            container_engine: get_container_engine_mock(true),
        };

        let app = Router::new()
            .route("/run-container", post(run))
            .with_state(server_config);

        let response = app
            .oneshot(
                Request::post("/run-container")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_string_pretty(&RunContainerPayload {
                            name: "test".to_string(),
                            image: "nginx:latest".to_string(),
                            external_port: Some(8080),
                            internal_port: Some(80),
                            cpus: 250,
                            memory: 64,
                            envs: HashMap::new(),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn test_run_container_failure() {
        let server_config = ServerConfig {
            container_engine: get_container_engine_mock(false),
        };

        let app = Router::new()
            .route("/run-container", post(run))
            .with_state(server_config);

        let response = app
            .oneshot(
                Request::post("/run-container")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_string_pretty(&RunContainerPayload {
                            name: "test".to_string(),
                            image: "nginx:latest".to_string(),
                            external_port: Some(8080),
                            internal_port: Some(80),
                            cpus: 250,
                            memory: 64,
                            envs: HashMap::new(),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_remove_container_success() {
        let server_config = ServerConfig {
            container_engine: get_container_engine_mock(true),
        };

        let app = Router::new()
            .route("/remove-container", post(remove))
            .with_state(server_config);

        let response = app
            .oneshot(
                Request::post("/remove-container")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_string_pretty(&RemoveContainerPayload {
                            name: "test".to_string(),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_remove_container_failure() {
        let server_config = ServerConfig {
            container_engine: get_container_engine_mock(false),
        };

        let app = Router::new()
            .route("/remove-container", post(remove))
            .with_state(server_config);

        let response = app
            .oneshot(
                Request::post("/remove-container")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_string_pretty(&RemoveContainerPayload {
                            name: "test".to_string(),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_health_check() {
        let server_config = ServerConfig {
            container_engine: get_container_engine_mock(true),
        };
        let app = Router::new()
            .route("/health-check", get(health_check))
            .with_state(server_config);

        let response = app
            .oneshot(Request::get("/health-check").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
