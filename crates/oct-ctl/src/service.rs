use std::collections::HashMap;
use std::fs::OpenOptions;

use axum::{
    Json, Router, extract, http::StatusCode, response::IntoResponse, routing::get, routing::post,
};
use petgraph::Graph;
use serde::{Deserialize, Serialize};
use tower_http::trace::{self, TraceLayer};

use oct_cloud::infra::graph::GraphManager;
use oct_cloud::infra::resource::{ResourceSpecType, SpecNode, VpcSpec};
use oct_cloud::infra::state::State;
use oct_orchestrator::backend;
use oct_orchestrator::config::StateBackend;

#[cfg(not(test))]
use crate::container::ContainerEngine;
#[cfg(test)]
use crate::container::mocks::MockContainerEngine as ContainerEngine;

pub(crate) async fn run() {
    let server_config = ServerConfig {
        container_engine: ContainerEngine::default(),
    };

    let app = prepare_router(server_config);

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

fn prepare_router(server_config: ServerConfig) -> Router {
    let log_file = OpenOptions::new()
        .append(true)
        .create(true)
        .open("/var/log/oct-ctl.log")
        .expect("Failed to open log file");

    // Tracing initialization code was inspired by
    // https://github.com/tower-rs/tower-http/issues/296#issuecomment-1301108593
    tracing_subscriber::fmt().with_writer(log_file).init();

    Router::new()
        .route("/apply", post(apply))
        .route("/destroy", post(destroy))
        .route("/run-container", post(run_container))
        .route("/remove-container", post(remove_container))
        .route("/health-check", get(health_check))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(trace::DefaultMakeSpan::new().level(tracing::Level::INFO))
                .on_response(trace::DefaultOnResponse::new().level(tracing::Level::INFO)),
        )
        .with_state(server_config)
}

/// Server config passed as a state to the endpoints.
/// It is used as a Dependency Injection container.
#[derive(Clone)]
struct ServerConfig {
    container_engine: ContainerEngine,
}

/// Apply endpoint definition for Axum
///
/// Temporary endpoint implementation to show the ability of `oct-ctl`
/// to deploy cloud infra resources from the Leader node
async fn apply() -> impl IntoResponse {
    let mut graph = Graph::<SpecNode, String>::new();
    let root = graph.add_node(SpecNode::Root);

    let vpc_2 = graph.add_node(SpecNode::Resource(ResourceSpecType::Vpc(VpcSpec {
        region: String::from("us-west-2"),
        cidr_block: String::from("10.1.0.0/16"),
        name: String::from("vpc-from-leader"),
    })));
    let edges = vec![(root, vpc_2, String::new())];
    graph.extend_with_edges(&edges);

    let graph_manager = GraphManager::new().await;

    let Ok(resource_graph) = graph_manager.deploy(&graph).await else {
        return (
            StatusCode::BAD_REQUEST,
            String::from("Failed to deploy graph"),
        );
    };

    let state = State::from_graph(&resource_graph);

    let state_backend = StateBackend::Local {
        path: String::from("/var/log/oct-state.json"),
    };
    let infra_state_backend = backend::get_state_backend::<State>(&state_backend);

    match infra_state_backend.save(&state).await {
        Ok(()) => (StatusCode::CREATED, "Success".to_string()),
        Err(err) => (
            StatusCode::BAD_REQUEST,
            format!("Error to save state: {err}"),
        ),
    }
}

/// Destroy endpoint definition for Axum
///
/// Temporary endpoint implementation to show the ability of `oct-ctl`
/// to destroy cloud infra resources from the Leader node reployed via `apply` endpoint
async fn destroy() -> impl IntoResponse {
    let state_backend = StateBackend::Local {
        path: String::from("/var/log/oct-state.json"),
    };
    let infra_state_backend = backend::get_state_backend::<State>(&state_backend);
    let Ok((state, _loaded)) = infra_state_backend.load().await else {
        return (
            StatusCode::BAD_REQUEST,
            String::from("Failed to load state"),
        );
    };

    let mut graph = state.to_graph();

    let graph_manager = GraphManager::new().await;
    match graph_manager.destroy(&mut graph).await {
        Ok(_resource_graph) => (StatusCode::OK, String::from("Success")),
        Err(err) => (
            StatusCode::BAD_REQUEST,
            format!("Error to save state: {err}"),
        ),
    }
}

#[derive(Serialize, Deserialize)]
struct RunContainerPayload {
    /// Name of the container
    name: String,
    /// Image to use for the container
    image: String,
    /// Command to run in the container
    command: Option<String>,
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

/// Run container endpoint definition for Axum
async fn run_container(
    extract::State(server_config): extract::State<ServerConfig>,
    Json(payload): Json<RunContainerPayload>,
) -> impl IntoResponse {
    let run_result = server_config.container_engine.run(
        payload.name.clone(),
        payload.image,
        payload.command,
        payload.external_port,
        payload.internal_port,
        payload.cpus,
        payload.memory,
        &payload.envs,
    );

    match run_result {
        Ok(()) => {
            log::info!("Created container: {}", payload.name);
            (StatusCode::CREATED, "Success".to_string())
        }
        Err(err) => {
            log::error!("Failed to create container: {err}");
            (StatusCode::BAD_REQUEST, format!("Error: {err}"))
        }
    }
}

#[derive(Serialize, Deserialize)]
struct RemoveContainerPayload {
    /// Name of the container
    name: String,
}

/// Remove container endpoint definition for Axum
async fn remove_container(
    extract::State(server_config): extract::State<ServerConfig>,
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

    fn get_container_engine_mock(is_ok: bool) -> ContainerEngine {
        let mut container_engine_mock = ContainerEngine::default();
        container_engine_mock
            .expect_run()
            .returning(
                move |_, _, _, _, _, _, _, _| {
                    if is_ok { Ok(()) } else { Err("error".into()) }
                },
            );

        container_engine_mock
            .expect_remove()
            .returning(move |_| if is_ok { Ok(()) } else { Err("error".into()) });

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
            .route("/run-container", post(run_container))
            .with_state(server_config);

        let response = app
            .oneshot(
                Request::post("/run-container")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_string_pretty(&RunContainerPayload {
                            name: "test".to_string(),
                            image: "nginx:latest".to_string(),
                            command: Some("echo hello".to_string()),
                            external_port: Some(8080),
                            internal_port: Some(80),
                            cpus: 250,
                            memory: 64,
                            envs: HashMap::new(),
                        })
                        .expect("Failed to dump JSON"),
                    ))
                    .expect("Failed to prepare body"),
            )
            .await
            .expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn test_run_container_failure() {
        let server_config = ServerConfig {
            container_engine: get_container_engine_mock(false),
        };

        let app = Router::new()
            .route("/run-container", post(run_container))
            .with_state(server_config);

        let response = app
            .oneshot(
                Request::post("/run-container")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_string_pretty(&RunContainerPayload {
                            name: "test".to_string(),
                            image: "nginx:latest".to_string(),
                            command: Some("echo hello".to_string()),
                            external_port: Some(8080),
                            internal_port: Some(80),
                            cpus: 250,
                            memory: 64,
                            envs: HashMap::new(),
                        })
                        .expect("Failed to dump JSON"),
                    ))
                    .expect("Failed to prepare body"),
            )
            .await
            .expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_remove_container_success() {
        let server_config = ServerConfig {
            container_engine: get_container_engine_mock(true),
        };

        let app = Router::new()
            .route("/remove-container", post(remove_container))
            .with_state(server_config);

        let response = app
            .oneshot(
                Request::post("/remove-container")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_string_pretty(&RemoveContainerPayload {
                            name: "test".to_string(),
                        })
                        .expect("Failed to dump JSON"),
                    ))
                    .expect("Failed to prepare body"),
            )
            .await
            .expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_remove_container_failure() {
        let server_config = ServerConfig {
            container_engine: get_container_engine_mock(false),
        };

        let app = Router::new()
            .route("/remove-container", post(remove_container))
            .with_state(server_config);

        let response = app
            .oneshot(
                Request::post("/remove-container")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_string_pretty(&RemoveContainerPayload {
                            name: "test".to_string(),
                        })
                        .expect("Failed to dump JSON"),
                    ))
                    .expect("Failed to prepare body"),
            )
            .await
            .expect("Failed to get response");

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
            .oneshot(
                Request::get("/health-check")
                    .body(Body::empty())
                    .expect("Failed to prepare body"),
            )
            .await
            .expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::OK);
    }
}
