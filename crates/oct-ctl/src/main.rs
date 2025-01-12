use axum::{http::StatusCode, response::IntoResponse, routing::post, Json, Router};
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

async fn run(Json(payload): Json<RunContainerPayload>) -> impl IntoResponse {
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
            (StatusCode::CREATED, "Success")
        }
        Err(err) => {
            log::error!("{}", err);
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

    let app = Router::new()
        .route("/run-container", post(run))
        .route("/remove-container", post(remove))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(trace::DefaultMakeSpan::new().level(tracing::Level::INFO))
                .on_response(trace::DefaultOnResponse::new().level(tracing::Level::INFO)),
        );

    let listener = tokio::net::TcpListener::bind("0.0.0.0:31888")
        .await
        .unwrap();

    tracing::info!("Listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}

// TODO: add tests
