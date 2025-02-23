mod container;
mod service;

#[tokio::main]
async fn main() {
    service::run().await;
}
