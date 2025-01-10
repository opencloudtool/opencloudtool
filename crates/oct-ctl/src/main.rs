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
    internal_port: String,
    external_port: String,
}

#[derive(Serialize, Deserialize)]
struct RemoveContainerPayload {
    name: String,
}

#[post("/run-container")]
async fn run(payload: web::Json<RunContainerPayload>) -> impl Responder {
    let command = Command::new("podman")
        .args([
            "run",
            "-d",
            "--name",
            &payload.name.as_str(),
            "-p",
            format!(
                "{external_port}:{internal_port}",
                external_port = &payload.external_port,
                internal_port = &payload.internal_port
            )
            .as_str(),
            &payload.image.as_str(),
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

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let target = Box::new(File::create("/var/log/oct-ctl.log").expect("Can't create file"));

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .target(env_logger::Target::Pipe(target))
        .init();

    log::info!("Starting server at http://0.0.0.0:31888");

    HttpServer::new(|| {
        let logger = Logger::default();
        App::new().wrap(logger).service(run).service(remove)
    })
    .bind(("0.0.0.0", 31888))?
    .run()
    .await
}

// TODO: add tests
