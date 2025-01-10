use actix_web::{middleware::Logger, post, web, App, HttpServer, Responder};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::process::Command;

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

#[post("/remove-container")]
async fn remove(payload: web::Json<RemoveContainerPayload>) -> impl Responder {
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
            "Success"
        }
        Err(err) => {
            log::error!("{}", err);
            "Error"
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
