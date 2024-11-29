use actix_web::{get, App, HttpServer, Responder};
use std::process::Command;

#[get("/")]
async fn index() -> impl Responder {
    let command = Command::new("docker")
        .arg("run")
        .arg("-d")
        .arg("-p")
        .arg("80:80")
        .arg("nginx")
        .output();

    match command {
        Ok(_) => "Success",
        Err(_) => "Error",
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("Starting server at http://0.0.0.0:31888");

    HttpServer::new(|| App::new().service(index))
        .bind(("0.0.0.0", 31888))?
        .run()
        .await
}
