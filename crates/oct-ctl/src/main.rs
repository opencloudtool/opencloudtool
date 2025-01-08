use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use mockall::mock;

use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use actix_web::{middleware::Logger, post, web, App, HttpServer, Responder};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::process::Command;
use tower_http::trace::{self, TraceLayer};

#[derive(Serialize, Deserialize)]
struct RunContainerPayload {
    name: String,
    image: String,
    external_port: String,
    internal_port: String,
}

#[derive(Serialize, Deserialize)]
struct RemoveContainerPayload {
    name: String,
}

/// Container engine implementation
#[derive(Clone, Default)]
struct ContainerEngine;

impl ContainerEngine {
    /// Runs container using `podman`
    fn run(
        &self,
        name: &str,
        image: &str,
        external_port: &str,
        internal_port: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let command = Command::new("podman")
            .args([
                "run",
                "-d",
                "--name",
                name,
                "-p",
                format!(
                    "{external_port}:{internal_port}",
                    external_port = &external_port,
                    internal_port = &internal_port
                )
                .as_str(),
                image,
            ])
            .output();

        match command {
            Ok(_) => Ok(()),
            Err(err) => Err(Box::new(err)),
        }
    }

    /// Removes container
    fn remove(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let command = Command::new("podman").args(["rm", "-f", name]).output();

        match command {
            Ok(_) => Ok(()),
            Err(err) => Err(Box::new(err)),
    image_uri: String,
    internal_port: String,
    external_port: String,
}

#[post("/run-container")]
async fn run(payload: web::Json<RunContainerPayload>) -> impl Responder {
    let command = Command::new("podman")
        .args([
            "run",
            "-d",
            "-p",
            format!(
                "{external_port}:{internal_port}",
                external_port = &payload.external_port,
                internal_port = &payload.internal_port
            )
            .as_str(),
            &payload.image_uri.as_str(),
        ])
        .output();

    log::info!(
        "{}",
        String::from_utf8_lossy(&command.as_ref().expect("failed").stdout)
    );

    match command {
        Ok(res) => {
            log::info!("Result: {}", String::from_utf8_lossy(&res.stdout));
            "Success"
        }
        Err(err) => {
            log::error!("{}", err);
            "Error"
        }
    }
}

// As long as ContainerEngine implemnts Clone, we mock it using
// mockall::mock macro, more info here:
// https://docs.rs/mockall/latest/mockall/macro.mock.html#examples
mock! {
    pub ContainerEngine {
        fn run(
            &self,
            name: &str,
            image: &str,
            external_port: &str,
            internal_port: &str,
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
        &payload.name.as_str(),
        &payload.image.as_str(),
        &payload.external_port,
        &payload.internal_port,
    );

    match run_result {
        Ok(_) => {
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
    let command = server_config
        .container_engine
        .remove(&payload.name.as_str());

    match command {
        Ok(_) => {
            log::info!("Removed container: {}", &payload.name);
            (StatusCode::OK, "Success")
        }
        Err(err) => {
            log::error!("Failed to remove container: {err}");
            (StatusCode::BAD_REQUEST, "Error")
        }
    }
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

    let container_engine = ContainerEngineImpl::default();
    let server_config = ServerConfig { container_engine };

    let app = Router::new()
        .route("/run-container", post(run))
        .route("/remove-container", post(remove))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(trace::DefaultMakeSpan::new().level(tracing::Level::INFO))
                .on_response(trace::DefaultOnResponse::new().level(tracing::Level::INFO)),
        )
        .with_state(server_config);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:31888")
        .await
        .unwrap();

    tracing::info!("Listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}

// TODO: Use parametrization and fixtures from
//     https://github.com/la10736/rstest
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
                move |_, _, _, _| {
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
                        serde_json::json!({
                            "name": "test",
                            "image": "nginx:latest",
                            "external_port": "8080",
                            "internal_port": "80",
                        })
                        .to_string(),
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
                        serde_json::json!({
                            "name": "test",
                            "image": "nginx:latest",
                            "external_port": "8080",
                            "internal_port": "80",
                        })
                        .to_string(),
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
                        serde_json::json!({
                            "name": "test",
                        })
                        .to_string(),
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
                        serde_json::json!({
                            "name": "test",
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let target = Box::new(File::create("/var/log/oct-ctl.log").expect("Can't create file"));

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .target(env_logger::Target::Pipe(target))
        .init();

    log::info!("Starting server at http://0.0.0.0:31888");

    HttpServer::new(|| {
        let logger = Logger::default();
        App::new().wrap(logger).service(run)
    })
    .bind(("0.0.0.0", 31888))?
    .run()
    .await
}

// TODO: add tests
