use actix_web::{get, App, HttpServer, Responder};

#[get("/")]
async fn index() -> impl Responder {
    "Hello, World!"
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("Starting server at http://127.0.0.1:31888");
    
    HttpServer::new(|| App::new().service(index))
        .bind(("127.0.0.1", 31888))?
        .run()
        .await
}