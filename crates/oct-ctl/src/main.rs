mod container;
mod executor;
mod service;

#[tokio::main]
async fn main() {
    service::run().await;
}
