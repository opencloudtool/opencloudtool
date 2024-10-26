use aws_sdk_ec2::config;
use aws_sdk_ec2::types;
use aws_sdk_ec2::Client;

pub async fn create_ec2_instance() -> Result<String, Box<dyn std::error::Error>> {
    // Load AWS configuration
    let region_provider = config::Region::new("us-west-2");
    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(region_provider)
        .load()
        .await;
    let client = Client::new(&config);

    // Specify instance details
    let instance_type = "t2.micro";
    let ami_id = "ami-0c65adc9a5c1b5d7c";

    // Launch EC2 instance
    let response = client
        .run_instances()
        .instance_type(types::InstanceType::from(instance_type))
        .image_id(ami_id)
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
        Some(id) => Ok(id),
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

    // Terminate the instance
    client
        .terminate_instances()
        .instance_ids(instance_id)
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
