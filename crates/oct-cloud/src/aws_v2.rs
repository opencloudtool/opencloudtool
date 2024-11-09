use aws_config;
use aws_sdk_ec2;
use aws_sdk_ec2::operation::run_instances::RunInstancesOutput;

use base64::{engine::general_purpose, Engine as _};

use mockall::automock;

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
    async fn destroy(&mut self) -> Result<(), Box<dyn std::error::Error>>;
}

struct Ec2Impl {
    inner: aws_sdk_ec2::Client,
}

/// TODO: Add tests using static replay
#[cfg_attr(test, automock)]
impl Ec2Impl {
    pub fn new(inner: aws_sdk_ec2::Client) -> Self {
        Self { inner }
    }

    pub async fn run_instances(
        &self,
        instance_type: aws_sdk_ec2::types::InstanceType,
        ami: String,
        user_data_base64: String,
    ) -> Result<RunInstancesOutput, Box<dyn std::error::Error>> {
        let response = self
            .inner
            .run_instances()
            .instance_type(instance_type.clone())
            .image_id(ami.clone())
            .user_data(user_data_base64.clone())
            .min_count(1)
            .max_count(1)
            .send()
            .await?;

        Ok(response)
    }

    pub async fn terminate_instance(
        &self,
        instance_id: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.inner
            .terminate_instances()
            .instance_ids(instance_id)
            .send()
            .await?;

        Ok(())
    }
}

#[cfg(not(test))]
use Ec2Impl as Ec2;
#[cfg(test)]
use MockEc2Impl as Ec2;

struct Ec2Instance {
    client: Ec2,

    // Known after creation
    id: Option<String>,

    arn: Option<String>,

    public_ip: Option<String>,
    public_dns: Option<String>,

    // Known before creation
    region: String,

    ami: String,

    instance_type: aws_sdk_ec2::types::InstanceType,

    name: String,
    user_data: String,
    user_data_base64: String,
}

impl Ec2Instance {
    async fn new(
        region: String,
        ami: String,
        instance_type: aws_sdk_ec2::types::InstanceType,
        name: String,
        user_data: String,
    ) -> Self {
        let user_data_base64 = general_purpose::STANDARD.encode(&user_data);

        // Load AWS configuration
        let region_provider = aws_sdk_ec2::config::Region::new(region.clone());
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(region_provider)
            .load()
            .await;
        let ec2_client = aws_sdk_ec2::Client::new(&config);

        Self {
            client: Ec2::new(ec2_client),
            id: None,
            arn: None,
            public_ip: None,
            public_dns: None,
            region,
            ami,
            instance_type,
            name,
            user_data,
            user_data_base64,
        }
    }
}

impl Resource for Ec2Instance {
    async fn create(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Launch EC2 instance
        let response = self
            .client
            .run_instances(
                self.instance_type.clone(),
                self.ami.clone(),
                self.user_data_base64.clone(),
            )
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

    async fn destroy(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.client
            .terminate_instance(self.id.clone().ok_or("No instance id")?)
            .await?;

        self.id = None;
        self.public_ip = None;
        self.public_dns = None;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::eq;

    #[tokio::test]
    async fn test_create_ec2_instance() {
        // Arrange
        let mut ec2_impl_mock = MockEc2Impl::default();
        ec2_impl_mock
            .expect_run_instances()
            .with(
                eq(aws_sdk_ec2::types::InstanceType::T2Micro),
                eq("ami-830c94e3".to_string()),
                eq("test".to_string()),
            )
            .return_once(|_, _, _| {
                Ok(RunInstancesOutput::builder()
                    .instances(
                        aws_sdk_ec2::types::Instance::builder()
                            .instance_id("id")
                            .public_ip_address("1.1.1.1")
                            .public_dns_name("example.com")
                            .build(),
                    )
                    .build())
            });

        let mut instance = Ec2Instance {
            client: ec2_impl_mock,
            id: None,
            arn: None,
            public_ip: None,
            public_dns: None,
            region: "us-west-2".to_string(),
            ami: "ami-830c94e3".to_string(),
            instance_type: aws_sdk_ec2::types::InstanceType::T2Micro,
            name: "test".to_string(),
            user_data: "test".to_string(),
            user_data_base64: "test".to_string(),
        };

        // Act
        instance.create().await.unwrap();

        // Assert
        assert!(instance.id == Some("id".to_string()));
        assert!(instance.public_ip == Some("1.1.1.1".to_string()));
        assert!(instance.public_dns == Some("example.com".to_string()));

        // assert!(instance.region == "us-west-2");
        // assert!(instance.ami == "ami-830c94e3");
        // assert!(instance.arn == "arn:aws:ec2:us-west-2:595634067310:instance/i-0e2939f5d64eba517");
        // assert!(instance.instance_type == aws_sdk_ec2::types::InstanceType::T2Micro);
        // assert!(instance.key_name == "test");
        // assert!(instance.name == "test");
        // assert!(instance.user_data == "test");
    }

    #[tokio::test]
    async fn test_destroy_ec2_instance() {
        // Arrange
        let mut ec2_impl_mock = MockEc2Impl::default();
        ec2_impl_mock
            .expect_terminate_instance()
            .with(eq("id".to_string()))
            .return_once(|_| Ok(()));

        let mut instance = Ec2Instance {
            client: ec2_impl_mock,
            id: Some("id".to_string()),
            arn: None,
            public_ip: None,
            public_dns: None,
            region: "us-west-2".to_string(),
            ami: "ami-830c94e3".to_string(),
            instance_type: aws_sdk_ec2::types::InstanceType::T2Micro,
            name: "test".to_string(),
            user_data: "test".to_string(),
            user_data_base64: "test".to_string(),
        };

        // Act
        instance.destroy().await.unwrap();

        // Assert
        assert!(instance.id == None);
        assert!(instance.public_ip == None);
        assert!(instance.public_dns == None);
    }
}
