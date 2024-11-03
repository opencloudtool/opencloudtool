use aws_config;
use aws_sdk_ec2;

use base64::{engine::general_purpose, Engine as _};

/// Now we deploy only one EC2 instance where the services from
/// the config.
/// In state we store only the information about the instance and
/// related resources (IAM role, ECR repository, etc.).
///
/// User flow:
/// - Check state of the resource (by resource name from dynamic config)
/// - Create if not exists
/// - Update if exists
trait Resource {
    async fn create(&mut self) -> Result<(), Box<dyn std::error::Error>>;
    async fn destroy(&self) -> Result<(), Box<dyn std::error::Error>>;
}

struct Ec2Instance {
    // Known after creation
    id: Option<String>,

    public_ip: Option<String>,
    public_dns: Option<String>,

    // Known before creation
    region: String,

    ami: String,
    arn: String,

    instance_type: aws_sdk_ec2::types::InstanceType,

    key_name: String,

    name: String,
    user_data: String,
    user_data_base64: String,
}

impl Ec2Instance {
    fn new(
        region: String,
        ami: String,
        arn: String,
        instance_type: aws_sdk_ec2::types::InstanceType,
        key_name: String,
        name: String,
        user_data: String,
    ) -> Self {
        let user_data_base64 = general_purpose::STANDARD.encode(&user_data);

        Self {
            id: None,
            public_ip: None,
            public_dns: None,
            region,
            ami,
            arn,
            instance_type,
            key_name,
            name,
            user_data,
            user_data_base64,
        }
    }
}

impl Resource for Ec2Instance {
    async fn create(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Load AWS configuration
        let region_provider = aws_sdk_ec2::config::Region::new(self.region.clone());
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(region_provider)
            .load()
            .await;
        let ec2_client = aws_sdk_ec2::Client::new(&config);

        // Launch EC2 instance
        let response = ec2_client
            .run_instances()
            .instance_type(self.instance_type.clone())
            .image_id(self.ami.clone())
            .user_data(self.user_data_base64.clone())
            .min_count(1)
            .max_count(1)
            .send()
            .await?;

        // Extract instance id, public ip and dns
        let instance = response
            .instances()
            .first()
            .ok_or("No instances returned")?;

        self.id = instance.instance_id.clone();
        self.public_ip = instance.public_ip_address.clone();
        self.public_dns = instance.public_dns_name.clone();

        Ok(())
    }

    async fn destroy(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
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

        let mut instance = Ec2Instance::new(
            "us-west-2".to_string(),
            "ami-830c94e3".to_string(),
            "arn:aws:ec2:us-west-2:595634067310:instance/i-0e2939f5d64eba517".to_string(),
            aws_sdk_ec2::types::InstanceType::T2Micro,
            "test".to_string(),
            "test".to_string(),
            "test".to_string(),
        );

        instance.create().await.unwrap();

        assert!(instance.id.is_some());
        assert!(instance.public_ip.is_some());
        assert!(instance.public_dns.is_some());

        assert!(instance.region == "us-west-2");
        assert!(instance.ami == "ami-830c94e3");
        assert!(instance.arn == "arn:aws:ec2:us-west-2:595634067310:instance/i-0e2939f5d64eba517");
        assert!(instance.instance_type == aws_sdk_ec2::types::InstanceType::T2Micro);
        assert!(instance.key_name == "test");
        assert!(instance.name == "test");
        assert!(instance.user_data == "test");
    }
}
