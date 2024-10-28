use aws_config;
use aws_sdk_ec2;
use aws_sdk_ecr;
use aws_sdk_iam;

use base64::{engine::general_purpose, Engine as _};
use std::process::Command;
use uuid;

pub async fn create_ec2_instance(
    docker_image_tag: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // Load AWS configuration
    let region_provider = aws_sdk_ec2::config::Region::new("us-west-2");
    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(region_provider)
        .load()
        .await;
    let ec2_client = aws_sdk_ec2::Client::new(&config);

    // Specify instance details
    let instance_type = "t2.micro";
    let ami_id = "ami-0c65adc9a5c1b5d7c";

    // Generate ssh key pair
    let key_name = uuid::Uuid::new_v4().to_string();
    let key_pair = ec2_client
        .create_key_pair()
        .key_name(&key_name)
        .send()
        .await?;

    // User data
    // get ecr repo uri from docker image tag
    let ecr_repo_uri = docker_image_tag.split('/').next().unwrap();

    let user_data = format!(
        r#"#!/bin/bash
set -e

sudo apt update
sudo apt install docker.io awscli -y
sudo systemctl start docker
aws ecr get-login-password --region us-west-2 | docker login --username AWS --password-stdin {ecr_repo_uri}
sudo docker run -d --name ct-app -p 80:8000 {ecr_repo_uri}/ct-app:latest
"#,
        ecr_repo_uri = ecr_repo_uri
    );

    let encoded_user_data = general_purpose::STANDARD.encode(user_data);

    // Create iam role for the instance with ECR access
    // Create IAM client
    let iam_client = aws_sdk_iam::Client::new(&config);

    // Create IAM role for EC2 instance
    let assume_role_policy = r#"{
        "Version": "2012-10-17",
        "Statement": [
            {
                "Effect": "Allow",
                "Principal": {
                    "Service": "ec2.amazonaws.com"
                },
                "Action": "sts:AssumeRole"
            }
        ]
    }"#;

    let _iam_role = iam_client
        .create_role()
        .role_name("ct-app-ecr-role")
        .assume_role_policy_document(assume_role_policy)
        .send()
        .await?;

    // Attach AmazonEC2ContainerRegistryReadOnly policy to the role
    iam_client
        .attach_role_policy()
        .role_name("ct-app-ecr-role")
        .policy_arn("arn:aws:iam::aws:policy/AmazonEC2ContainerRegistryReadOnly")
        .send()
        .await?;

    // Create an instance profile and add the role to it
    let instance_profile = iam_client
        .create_instance_profile()
        .instance_profile_name("ct-app-ecr-profile")
        .send()
        .await?;

    iam_client
        .add_role_to_instance_profile()
        .instance_profile_name("ct-app-ecr-profile")
        .role_name("ct-app-ecr-role")
        .send()
        .await?;

    // Wait for the instance profile to be ready
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    // Launch EC2 instance
    let response = ec2_client
        .run_instances()
        .instance_type(aws_sdk_ec2::types::InstanceType::from(instance_type))
        .image_id(ami_id)
        .user_data(encoded_user_data)
        .key_name(key_pair.key_name().unwrap())
        .iam_instance_profile(
            aws_sdk_ec2::types::IamInstanceProfileSpecification::builder()
                .arn(instance_profile.instance_profile().unwrap().arn.as_str())
                .build(),
        )
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
            let key_pair_file_path = format!("{}.pem", &key_name);
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
    let region_provider = aws_sdk_ec2::config::Region::new("us-west-2");
    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(region_provider)
        .load()
        .await;
    // Create EC2 client
    let ec2_client = aws_sdk_ec2::Client::new(&config);

    // Create IAM client
    let iam_client = aws_sdk_iam::Client::new(&config);

    // Get instance details
    let instance_details = ec2_client
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
    ec2_client
        .terminate_instances()
        .instance_ids(instance_id)
        .send()
        .await?;

    // Remove iam role and instance profile
    iam_client
        .remove_role_from_instance_profile()
        .instance_profile_name("ct-app-ecr-profile")
        .role_name("ct-app-ecr-role")
        .send()
        .await?;
    iam_client
        .detach_role_policy()
        .role_name("ct-app-ecr-role")
        .policy_arn("arn:aws:iam::aws:policy/AmazonEC2ContainerRegistryReadOnly")
        .send()
        .await?;
    iam_client
        .delete_role()
        .role_name("ct-app-ecr-role")
        .send()
        .await?;
    iam_client
        .delete_instance_profile()
        .instance_profile_name("ct-app-ecr-profile")
        .send()
        .await?;

    // Delete the key pair
    ec2_client
        .delete_key_pair()
        .key_name(key_name)
        .send()
        .await?;

    // Delete key pair file
    std::fs::remove_file(format!("{}.pem", &key_name))?;

    Ok(())
}

pub async fn create_ecr_repository(
    repository_name: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // Load AWS configuration
    let region_provider = aws_sdk_ecr::config::Region::new("us-west-2");
    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(region_provider)
        .load()
        .await;
    let client = aws_sdk_ecr::Client::new(&config);

    // Create ECR repository and get its URI
    let create_result = client
        .create_repository()
        .repository_name(repository_name)
        .send()
        .await?;

    let repository_uri = create_result
        .repository()
        .and_then(|repo| repo.repository_uri().map(String::from))
        .ok_or("Failed to get repository URI")?;

    Ok(repository_uri)
}

pub async fn delete_ecr_repository(
    repository_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Load AWS configuration
    let region_provider = aws_sdk_ecr::config::Region::new("us-west-2");
    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(region_provider)
        .load()
        .await;
    let client = aws_sdk_ecr::Client::new(&config);

    client
        .delete_repository()
        .force(true)
        .repository_name(repository_name)
        .send()
        .await?;

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

        let instance_id = create_ec2_instance("1234567890").await.unwrap();
        assert!(!instance_id.is_empty());
    }

    #[tokio::test]
    async fn test_destroy_ec2_instance() {
        setup();

        let instance_id = create_ec2_instance("1234567890").await.unwrap();

        assert!(destroy_ec2_instance(&instance_id).await.is_ok());
    }
}
