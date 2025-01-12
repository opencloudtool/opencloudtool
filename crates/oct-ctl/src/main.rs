use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use mockall::mock;

use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
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

// trait ContainerEngineTrait {
//     fn run(
//         &self,
//         name: &str,
//         image: &str,
//         external_port: &str,
//         internal_port: &str,
//     ) -> Result<(), Box<dyn std::error::Error>>;

//     fn remove(&self, name: &str) -> Result<(), Box<dyn std::error::Error>>;
// }

#[derive(Clone)]
enum ContainerEngineType {
    Docker,
    Podman,
}

impl ContainerEngineType {
    fn as_str(&self) -> &'static str {
        match self {
            ContainerEngineType::Docker => "docker",
            ContainerEngineType::Podman => "podman",
        }
    }
}

#[derive(Clone)]
struct ContainerEngine;

impl ContainerEngine {
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

    fn remove(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

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

#[derive(Clone)]
struct ServerConfig {
    container_engine: ContainerEngineImpl,
}

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

async fn remove(Json(payload): Json<RemoveContainerPayload>) -> impl IntoResponse {
    let command = Command::new("podman")
        .args(["rm", "-f", &payload.name.as_str()])
        .output();

    log::info!(
        "{}",
        String::from_utf8_lossy(&command.as_ref().expect("failed").stdout)
    );

    match command {
        Ok(res) => {
            log::info!("Result: {}", String::from_utf8_lossy(&res.stdout));
            (StatusCode::OK, "Success")
        }
        Err(err) => {
            log::error!("{}", err);
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

    let container_engine = ContainerEngine;

    let app = Router::new()
        .route("/run-container", post(run))
        .route("/remove-container", post(remove))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(trace::DefaultMakeSpan::new().level(tracing::Level::INFO))
                .on_response(trace::DefaultOnResponse::new().level(tracing::Level::INFO)),
        );
    // .with_state(ServerConfig { container_engine });

    let listener = tokio::net::TcpListener::bind("0.0.0.0:31888")
        .await
        .unwrap();

    tracing::info!("Listening on {}", listener.local_addr().unwrap());

    // axum::serve(listener, app).await.unwrap();
}

// TODO: add tests

#[cfg(test)]
mod tests {
    use super::*;

    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use axum::routing::Router;
    use axum_test::TestServer;
    use tower::ServiceExt; // for `call`, `oneshot`, and `ready`

    #[tokio::test]
    async fn test_run_container() {
        println!("Running test_run_container");
        let mut container_engine_mock = MockContainerEngine::new();
        container_engine_mock
            .expect_clone()
            .returning(|| MockContainerEngine::new());

        println!("Created mock");

        let server_config = ServerConfig {
            container_engine: container_engine_mock,
        };
        println!("Created server config");

        let app = Router::new()
            .route("/run-container", post(run))
            .with_state(server_config);
        println!("Created app");

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/run-container")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // let server = TestServer::new(app).unwrap();

        // // Get the request.
        // let response = server
        //     .post("/run-container")
        //     .json(&serde_json::json!({
        //         "name": "test",
        //         "image": "nginx:latest",
        //         "external_port": "8080",
        //         "internal_port": "80",
        //     }))
        //     .await;

        // // Assertions.
        // response.assert_status(StatusCode::CREATED);
    }
}
