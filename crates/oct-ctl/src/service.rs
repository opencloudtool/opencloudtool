use std::collections::HashMap;
use std::fs::OpenOptions;

use axum::{
    Json, Router, extract, http::StatusCode, response::IntoResponse, routing::get, routing::post,
};
use petgraph::Graph;
use serde::{Deserialize, Serialize};
use tower_http::trace::{self, TraceLayer};

use oct_cloud::infra::graph::kahn_traverse;
use oct_config::{Config, Node, StateBackend};
use oct_orchestrator::{backend, user_state};

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

#[derive(Serialize, Deserialize)]
struct ApplyPayload {
    config: Config,
}

/// Apply endpoint definition for Axum
///
/// Temporary endpoint implementation to show the ability of `oct-ctl`
/// to deploy cloud infra resources from the Leader node
async fn apply(
    extract::State(server_config): extract::State<ServerConfig>,
    Json(payload): Json<ApplyPayload>,
) -> impl IntoResponse {
    let Ok(services_graph) = payload.config.to_graph() else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            String::from("Failed to get graph from request body"),
        );
    };

    let apply_result = apply_user_services_graph(&server_config, &services_graph);

    match apply_result.await {
        Ok(()) => (StatusCode::CREATED, "Success".to_string()),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Error to save state: {err}"),
        ),
    }
}

/// Applies user services graph
async fn apply_user_services_graph(
    server_config: &ServerConfig,
    services_graph: &Graph<Node, String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let state_backend = StateBackend::Local {
        path: String::from("/var/log/oct-state.json"),
    };
    let user_state_backend = backend::get_state_backend::<user_state::UserState>(&state_backend);

    let sorted_graph = kahn_traverse(services_graph)?;

    let mut services = HashMap::new();
    for node_index in &sorted_graph {
        if let Node::Resource(service) = &services_graph[*node_index] {
            log::info!("Running service: {}", service.name);

            let run_result = server_config.container_engine.run(
                service.name.clone(),
                service.image.clone(),
                service.command.clone(),
                service.external_port,
                service.internal_port,
                service.cpus,
                service.memory,
                &service.envs,
            );

            let Ok(()) = run_result else {
                log::error!("Failed to run service: {}", service.name);

                continue;
            };

            services.insert(service.name.clone(), service.clone());
        }
    }

    let instance_state = user_state::Instance {
        cpus: 0,
        memory: 0,
        services,
    };

    let user_state = user_state::UserState {
        instances: HashMap::from([(String::from("localhost"), instance_state)]),
    };

    user_state_backend.save(&user_state).await
}

/// Destroy endpoint definition for Axum
///
/// Temporary endpoint implementation to show the ability of `oct-ctl`
/// to destroy cloud infra resources from the Leader node deployed via `apply` endpoint
async fn destroy() -> impl IntoResponse {
    let state_backend = StateBackend::Local {
        path: String::from("/var/log/oct-state.json"),
    };
    let user_state_backend = backend::get_state_backend::<user_state::UserState>(&state_backend);

    let Ok((_state, _loaded)) = user_state_backend.load().await else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            String::from("Failed to load state"),
        );
    };

    log::info!("Skipping containers removal in this version");

    (StatusCode::OK, String::from("Success"))
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
