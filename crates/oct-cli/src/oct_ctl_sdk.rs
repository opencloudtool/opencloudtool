use reqwest::Response;
use std::collections::HashMap;

pub async fn run_container(
    name: String,
    image: String,
    external_port: String,
    internal_port: String,
    public_ip: String,
) -> Result<Response, reqwest::Error> {
    let client = reqwest::Client::new();

    let mut map = HashMap::new();
    map.insert("name", name.as_str());
    map.insert("image", image.as_str());
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

pub async fn remove_container(name: String, public_ip: String) -> Result<Response, reqwest::Error> {
    let client = reqwest::Client::new();

    let mut map = HashMap::new();
    map.insert("name", name.as_str());

    let response = client
        .post(format!(
            "http://{public_ip}:31888/remove-container",
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
