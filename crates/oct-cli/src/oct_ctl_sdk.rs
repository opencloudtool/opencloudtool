/// TODO: Generate this from `oct-ctl`'s OpenAPI spec
use reqwest::Response;
use std::collections::HashMap;

pub(crate) struct Client {
    public_ip: String,
}

impl Client {
    pub(crate) fn new(public_ip: String) -> Self {
        Self { public_ip }
    }

    pub(crate) async fn run_container(
        &self,
        name: String,
        image: String,
        external_port: String,
        internal_port: String,
    ) -> Result<Response, reqwest::Error> {
        let client = reqwest::Client::new();

        let map = HashMap::from([
            ("name", name.as_str()),
            ("image", image.as_str()),
            ("external_port", external_port.as_str()),
            ("internal_port", internal_port.as_str()),
        ]);

        let response = client
            .post(format!("http://{}:31888/run-container", self.public_ip))
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

    pub(crate) async fn remove_container(&self, name: String) -> Result<Response, reqwest::Error> {
        let client = reqwest::Client::new();

        let map = HashMap::from([("name", name.as_str())]);

        let response = client
            .post(format!("http://{}:31888/remove-container", self.public_ip))
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
}
