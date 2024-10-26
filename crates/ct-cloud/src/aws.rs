use aws_sdk_ec2::config;
use aws_sdk_ec2::types;
use aws_sdk_ec2::Client;
use base64::{engine::general_purpose, Engine as _};
use std::process::Command;
use uuid;

pub async fn create_ec2_instance() -> Result<String, Box<dyn std::error::Error>> {
    // Load AWS configuration
    let region_provider = config::Region::new("us-west-2");
    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(region_provider)
        .load()
        .await;
    let client: Client = Client::new(&config);

    // Specify instance details
    let instance_type = "t2.micro";
    let ami_id = "ami-0c65adc9a5c1b5d7c";

    // Generate ssh key pair
    let key_pair = client
        .create_key_pair()
        .key_name(uuid::Uuid::new_v4().to_string())
        .send()
        .await?;

    // User data
    let user_data = r#"#!/bin/bash
set -e

sudo apt update
sudo apt install docker.io -y
sudo systemctl start docker
"#;

    let encoded_user_data = general_purpose::STANDARD.encode(user_data);

    // Launch EC2 instance
    let response = client
        .run_instances()
        .instance_type(types::InstanceType::from(instance_type))
        .image_id(ami_id)
        .user_data(encoded_user_data)
        .key_name(key_pair.key_name().unwrap())
        .min_count(1)
        .max_count(1)
        .send()
        .await?;

    // Extract the instance ID from the response
    let instance_id = response
        .instances()
        .first()
        .and_then(|instance| instance.instance_id().map(String::from));

    // Return the instance ID or an error message
    match instance_id {
        Some(id) => {
            // Save key pair to file
            let key_pair_file_path = format!("{}.pem", id);
            std::fs::write(&key_pair_file_path, key_pair.key_material().unwrap())?;

            Command::new("chmod")
                .arg("400")
                .arg(key_pair_file_path)
                .spawn()?;

            Ok(id)
        }
        None => Err("Failed to launch EC2 instance".into()),
    }
}

pub async fn destroy_ec2_instance(instance_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Load AWS configuration
    let region_provider = config::Region::new("us-west-2");
    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(region_provider)
        .load()
        .await;
    let client = Client::new(&config);

    // Get instance details
    let instance_details = client
        .describe_instances()
        .instance_ids(instance_id)
        .send()
        .await?;

    // Extract key name from instance details
    let key_name = instance_details
        .reservations()
        .first()
        .and_then(|r| r.instances().first())
        .and_then(|i| i.key_name())
        .ok_or("Failed to get key name")?;

    // Terminate the instance
    client
        .terminate_instances()
        .instance_ids(instance_id)
        .send()
        .await?;

    // Delete the key pair
    client.delete_key_pair().key_name(key_name).send().await?;

    // Delete key pair file
    std::fs::remove_file(format!("{}.pem", instance_id))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    use std::sync::Once;

    static SETUP: Once = Once::new();

    // TODO: Move to ct-test-utils crate
    pub fn setup() {
        SETUP.call_once(|| {
            if env::var("AWS_ENDPOINT_URL").is_err() {
                env::set_var("AWS_ENDPOINT_URL", "http://localhost:4566");
            }
            if env::var("AWS_REGION").is_err() {
                env::set_var("AWS_REGION", "eu-central-1");
            }
            if env::var("AWS_ACCESS_KEY_ID").is_err() {
                env::set_var("AWS_ACCESS_KEY_ID", "test");
            }
            if env::var("AWS_SECRET_ACCESS_KEY").is_err() {
                env::set_var("AWS_SECRET_ACCESS_KEY", "test");
            }
        });
    }

    #[tokio::test]
    async fn test_create_ec2_instance() {
        setup();

        let instance_id = create_ec2_instance().await.unwrap();
        assert!(!instance_id.is_empty());
    }

    #[tokio::test]
    async fn test_destroy_ec2_instance() {
        setup();

        let instance_id = create_ec2_instance().await.unwrap();

        assert!(destroy_ec2_instance(&instance_id).await.is_ok());
    }
}
