use reqwest::Response;
use std::collections::HashMap;

pub async fn run_container(
    container_name: String,
    internal_port: String,
    external_port: String,
    public_ip: String,
) -> Result<Response, reqwest::Error> {
    let client = reqwest::Client::new();

    let mut map = HashMap::new();
    map.insert("image_uri", container_name.as_str());
    map.insert("external_port", external_port.as_str());
    map.insert("internal_port", internal_port.as_str());

    let response = client
        .post(format!(
            "http://{public_ip}:31888/run-container",
            public_ip = public_ip
        ))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(serde_json::to_string(&map).unwrap())
        .send()
        .await?;

    if response.status().is_success() {
        Ok(response)
    } else {
        response.error_for_status()
    }
}
