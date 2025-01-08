use aws_config;
pub use aws_sdk_ec2;
use aws_sdk_ec2::operation::run_instances::RunInstancesOutput;

use base64::{engine::general_purpose, Engine as _};

use log;
#[allow(unused_imports)]
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

pub trait Resource {
    fn create(
        &mut self,
    ) -> impl std::future::Future<Output = Result<(), Box<dyn std::error::Error>>> + Send;
    fn destroy(
        &mut self,
    ) -> impl std::future::Future<Output = Result<(), Box<dyn std::error::Error>>> + Send;
}

#[derive(Debug)]
struct Ec2Impl {
    inner: aws_sdk_ec2::Client,
}

/// TODO: Add tests using static replay
#[cfg_attr(test, automock)]
impl Ec2Impl {
    fn new(inner: aws_sdk_ec2::Client) -> Self {
        Self { inner }
    }

    // Retrieve metadata about specific EC2 instance
    async fn describe_instances(
        &self,
        instance_id: String,
    ) -> Result<aws_sdk_ec2::types::Instance, Box<dyn std::error::Error>> {
        let response = self
            .inner
            .describe_instances()
            .instance_ids(instance_id)
            .send()
            .await?;

        let instance = response
            .reservations()
            .get(0)
            .ok_or("No reservations")?
            .instances()
            .get(0)
            .ok_or("No instances")?;

        Ok(instance.clone())
    }

    // TODO: Return Instance instead of response
    async fn run_instances(
        &self,
        instance_type: aws_sdk_ec2::types::InstanceType,
        ami: String,
        user_data_base64: String,
        instance_profile_name: Option<String>,
    ) -> Result<RunInstancesOutput, Box<dyn std::error::Error>> {
        log::info!("Starting EC2 instance");

        let mut request = self
            .inner
            .run_instances()
            .instance_type(instance_type.clone())
            .image_id(ami.clone())
            .user_data(user_data_base64.clone())
            .min_count(1)
            .max_count(1);

        if let Some(instance_profile_name) = instance_profile_name {
            request = request.iam_instance_profile(
                aws_sdk_ec2::types::IamInstanceProfileSpecification::builder()
                    .name(instance_profile_name.clone())
                    .build(),
            );
        }

        let response = request.send().await?;

        log::info!("Created EC2 instance");

        Ok(response)
    }

    async fn terminate_instance(
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

#[derive(Debug)]
struct IAMImpl {
    inner: aws_sdk_iam::Client,
}

/// TODO: Add tests using static replay
#[cfg_attr(test, automock)]
impl IAMImpl {
    fn new(inner: aws_sdk_iam::Client) -> Self {
        Self { inner }
    }

    async fn create_instance_iam_role(
        &self,
        name: String,
        assume_role_policy: String,
        policy_arns: Vec<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Create IAM role for EC2 instance
        log::info!("Creating IAM role for EC2 instance");

        self.inner
            .create_role()
            .role_name(name.clone())
            .assume_role_policy_document(assume_role_policy)
            .send()
            .await?;

        log::info!("Created IAM role for EC2 instance");

        for policy_arn in &policy_arns {
            log::info!("Attaching '{policy_arn}' policy to the role");

            self.inner
                .attach_role_policy()
                .role_name(name.clone())
                .policy_arn(policy_arn)
                .send()
                .await?;

            log::info!("Attached '{policy_arn}' policy to the role");
        }

        Ok(())
    }

    async fn delete_instance_iam_role(
        &self,
        name: String,
        policy_arns: Vec<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for policy_arn in &policy_arns {
            log::info!("Detaching '{policy_arn}' IAM role from EC2 instance");

            self.inner
                .detach_role_policy()
                .role_name(name.clone())
                .policy_arn(policy_arn)
                .send()
                .await?;

            log::info!("Detached '{policy_arn}' IAM role from EC2 instance");
        }

        log::info!("Deleting IAM role for EC2 instance");

        self.inner
            .delete_role()
            .role_name(name.clone())
            .send()
            .await?;

        log::info!("Deleted IAM role for EC2 instance");

        Ok(())
    }

    async fn create_instance_profile(
        &self,
        name: String,
        role_names: Vec<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Creating IAM instance profile for EC2 instance");

        self.inner
            .create_instance_profile()
            .instance_profile_name(name.clone())
            .send()
            .await?;

        log::info!("Created IAM instance profile for EC2 instance");

        for role_name in role_names {
            log::info!("Adding '{role_name}' IAM role to instance profile");

            self.inner
                .add_role_to_instance_profile()
                .instance_profile_name(name.clone())
                .role_name(role_name.clone())
                .send()
                .await?;

            log::info!("Added '{role_name}' IAM role to instance profile");
        }

        log::info!("Waiting for instance profile to be ready");
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;

        Ok(())
    }

    async fn delete_instance_profile(
        &self,
        name: String,
        role_names: Vec<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for role_name in role_names {
            log::info!("Removing {role_name} IAM role from instance profile");

            self.inner
                .remove_role_from_instance_profile()
                .instance_profile_name(name.clone())
                .role_name(role_name.clone())
                .send()
                .await?;

            log::info!("Removed {role_name} IAM role from instance profile");
        }

        log::info!("Deleting IAM instance profile");

        self.inner
            .delete_instance_profile()
            .instance_profile_name(name.clone())
            .send()
            .await?;

        log::info!("Deleted IAM instance profile");

        Ok(())
    }
}

#[cfg(not(test))]
use IAMImpl as IAM;
#[cfg(test)]
use MockIAMImpl as IAM;

#[derive(Debug)]
pub struct Ec2Instance {
    client: Ec2,

    // Known after creation
    pub id: Option<String>,

    pub public_ip: Option<String>,
    pub public_dns: Option<String>,

    // Known before creation
    pub region: String,

    pub ami: String,

    pub instance_type: aws_sdk_ec2::types::InstanceType,
    pub name: String,
    pub user_data: String,
    pub user_data_base64: String,
    instance_type: aws_sdk_ec2::types::InstanceType,
    name: String,
    user_data: String,
    user_data_base64: String,

    // TODO: Make it required
    pub instance_profile: Option<InstanceProfile>,
}

impl Ec2Instance {
    pub async fn new(
        id: Option<String>,
        public_ip: Option<String>,
        public_dns: Option<String>,
        region: String,
        ami: String,
        instance_type: aws_sdk_ec2::types::InstanceType,
        name: String,
        instance_profile: Option<InstanceProfile>,
    ) -> Self {
        let user_data = r#"#!/bin/bash
    set -e

    sudo apt update
    sudo apt -y install podman
    sudo systemctl start podman

    # aws ecr get-login-password --region us-west-2 | podman login --username AWS --password-stdin {ecr_repo_uri}

    curl \
        --output /home/ubuntu/oct-ctl \
        -L \
        https://github.com/21inchLingcod/opencloudtool/releases/download/v0.0.5/oct-ctl \
        && sudo chmod +x /home/ubuntu/oct-ctl \
        && /home/ubuntu/oct-ctl &
    "#;

        let user_data_base64 = general_purpose::STANDARD.encode(&user_data);

        // Load AWS configuration
        let region_provider = aws_sdk_ec2::config::Region::new(region.clone());
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .credentials_provider(
                aws_config::profile::ProfileFileCredentialsProvider::builder()
                    .profile_name("default")
                    .build(),
            )
            .region(region_provider)
            .load()
            .await;

        let ec2_client = aws_sdk_ec2::Client::new(&config);

        let instance_profile = match instance_profile {
            Some(profile) => profile,
            None => {
                let instance_role = InstanceRole::new(region.clone()).await;
                InstanceProfile::new(region.clone(), vec![instance_role]).await
            }
        };

        Self {
            client: Ec2::new(ec2_client),
            id,
            public_ip,
            public_dns,
            region,
            ami,
            instance_type,
            name,
            user_data: user_data.to_string(),
            user_data_base64,
            instance_profile: Some(instance_profile),
        }
    }
}

impl Resource for Ec2Instance {
    async fn create(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Create IAM role for EC2 instance
        match &mut self.instance_profile {
            Some(instance_profile) => instance_profile.create().await,
            None => Ok(()),
        }?;

        // Launch EC2 instance
        let response = self
            .client
            .run_instances(
                self.instance_type.clone(),
                self.ami.clone(),
                self.user_data_base64.clone(),
                match &self.instance_profile {
                    Some(instance_profile) => Some(instance_profile.name.clone()),
                    None => None,
                },
            )
            .await?;

        // Extract instance id, public ip and dns
        let instance = response
            .instances()
            .first()
            .ok_or("No instances returned")?;

        self.id = instance.instance_id.clone();

        // Poll for metadata
        const MAX_ATTEMPTS: usize = 10;
        const SLEEP_DURATION: std::time::Duration = std::time::Duration::from_secs(5);

        let instance_id = self.id.as_ref().ok_or("No instance id")?;

        for _ in 0..MAX_ATTEMPTS {
            log::info!("Waiting for EC2 instance metadata to be available...");

            if let Ok(instance) = self.client.describe_instances(instance_id.clone()).await {
                // Update metadata fields
                if let Some(public_ip) = instance.public_ip_address() {
                    self.public_ip = Some(public_ip.to_string());

                    log::info!("Metadata retrieved: public_ip={}", public_ip);
                }
                if let Some(public_dns) = instance.public_dns_name() {
                    self.public_dns = Some(public_dns.to_string());

                    log::info!("Metadata retrieved: public_dns={}", public_dns);
                }

                // Break if all metadata is available
                if self.public_ip.is_some() && self.public_dns.is_some() {
                    break;
                }
            }

            tokio::time::sleep(SLEEP_DURATION).await;
        }

        if self.public_ip.is_none() || self.public_dns.is_none() {
            return Err("Failed to retrieve instance metadata after retries".into());
        }

        Ok(())
    }

    async fn destroy(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.client
            .terminate_instance(self.id.clone().ok_or("No instance id")?)
            .await?;

        self.id = None;
        self.public_ip = None;
        self.public_dns = None;

        match &mut self.instance_profile {
            Some(instance_profile) => instance_profile.destroy().await,
            None => Ok(()),
        }
    }
}

#[derive(Debug)]
pub struct InstanceProfile {
    client: IAM,

    pub name: String,

    pub region: String,

    pub instance_roles: Vec<InstanceRole>,
}

impl InstanceProfile {
    const NAME: &str = "ct-app-ecr-role";

    pub async fn new(region: String, instance_roles: Vec<InstanceRole>) -> Self {
        // Load AWS configuration
        let region_provider = aws_sdk_ec2::config::Region::new(region.clone());
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .credentials_provider(
                aws_config::profile::ProfileFileCredentialsProvider::builder()
                    .profile_name("default")
                    .build(),
            )
            .region(region_provider)
            .load()
            .await;

        let iam_client = aws_sdk_iam::Client::new(&config);

        Self {
            client: IAM::new(iam_client),
            name: Self::NAME.to_string(),
            region,
            instance_roles,
        }
    }
}

impl Resource for InstanceProfile {
    async fn create(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        for role in &mut self.instance_roles {
            role.create().await?;
        }

        self.client
            .create_instance_profile(
                self.name.clone(),
                self.instance_roles.iter().map(|r| r.name.clone()).collect(),
            )
            .await
    }

    async fn destroy(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.client
            .delete_instance_profile(
                self.name.clone(),
                self.instance_roles.iter().map(|r| r.name.clone()).collect(),
            )
            .await?;

        for role in &mut self.instance_roles {
            role.destroy().await?;
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct InstanceRole {
    client: IAM,

    pub name: String,

    pub region: String,

    pub assume_role_policy: String,

    pub policy_arns: Vec<String>,
}

impl InstanceRole {
    const NAME: &str = "ct-app-ecr-role";
    const POLICY_ARN: &str = "arn:aws:iam::aws:policy/AmazonEC2ContainerRegistryReadOnly";
    const ASSUME_ROLE_POLICY: &str = r#"{
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

    pub async fn new(region: String) -> Self {
        // Load AWS configuration
        let region_provider = aws_sdk_ec2::config::Region::new(region.clone());
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .credentials_provider(
                aws_config::profile::ProfileFileCredentialsProvider::builder()
                    .profile_name("default")
                    .build(),
            )
            .region(region_provider)
            .load()
            .await;

        let iam_client = aws_sdk_iam::Client::new(&config);

        Self {
            client: IAM::new(iam_client),
            name: Self::NAME.to_string(),
            region,
            assume_role_policy: Self::ASSUME_ROLE_POLICY.to_string(),
            policy_arns: vec![Self::POLICY_ARN.to_string()],
        }
    }
}

impl Resource for InstanceRole {
    async fn create(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.client
            .create_instance_iam_role(
                self.name.clone(),
                self.assume_role_policy.clone(),
                self.policy_arns.clone(),
            )
            .await
    }

    async fn destroy(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.client
            .delete_instance_iam_role(self.name.clone(), self.policy_arns.clone())
            .await
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
                eq(None),
            )
            .return_once(|_, _, _, _| {
                Ok(RunInstancesOutput::builder()
                    .instances(
                        aws_sdk_ec2::types::Instance::builder()
                            .instance_id("id")
                            .public_ip_address(String::new())
                            .public_dns_name(String::new())
                            .build(),
                    )
                    .build())
            });

        ec2_impl_mock.expect_describe_instances().returning(|_| {
            Ok(aws_sdk_ec2::types::Instance::builder()
                .instance_id("id")
                .public_ip_address("1.1.1.1")
                .public_dns_name("example.com")
                .build())
        });

        let mut instance = Ec2Instance {
            client: ec2_impl_mock,
            id: None,
            public_ip: None,
            public_dns: None,
            region: "us-west-2".to_string(),
            ami: "ami-830c94e3".to_string(),
            instance_type: aws_sdk_ec2::types::InstanceType::T2Micro,
            name: "test".to_string(),
            user_data: "test".to_string(),
            user_data_base64: "test".to_string(),
            instance_profile: None,
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
    async fn test_create_ec2_instance_no_instance() {
        // Arrange
        let mut ec2_impl_mock = MockEc2Impl::default();
        ec2_impl_mock
            .expect_run_instances()
            .with(
                eq(aws_sdk_ec2::types::InstanceType::T2Micro),
                eq("ami-830c94e3".to_string()),
                eq("test".to_string()),
                eq(None),
            )
            .return_once(|_, _, _, _| Ok(RunInstancesOutput::builder().build()));

        let mut instance = Ec2Instance {
            client: ec2_impl_mock,
            id: None,
            public_ip: None,
            public_dns: None,
            region: "us-west-2".to_string(),
            ami: "ami-830c94e3".to_string(),
            instance_type: aws_sdk_ec2::types::InstanceType::T2Micro,
            name: "test".to_string(),
            user_data: "test".to_string(),
            user_data_base64: "test".to_string(),
            instance_profile: None,
        };

        // Act
        let creation_result = instance.create().await;

        // Assert
        assert!(creation_result.is_err());

        assert!(instance.id == None);
        assert!(instance.public_ip == None);
        assert!(instance.public_dns == None);
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
            public_ip: Some("1.1.1.1".to_string()),
            public_dns: Some("example.com".to_string()),
            region: "us-west-2".to_string(),
            ami: "ami-830c94e3".to_string(),
            instance_type: aws_sdk_ec2::types::InstanceType::T2Micro,
            name: "test".to_string(),
            user_data: "test".to_string(),
            user_data_base64: "test".to_string(),
            instance_profile: None,
        };

        // Act
        instance.destroy().await.unwrap();

        // Assert
        assert!(instance.id == None);
        assert!(instance.public_ip == None);
        assert!(instance.public_dns == None);
    }

    #[tokio::test]
    async fn test_destroy_ec2_instance_no_instance_id() {
        // Arrange
        let mut ec2_impl_mock = MockEc2Impl::default();
        ec2_impl_mock
            .expect_terminate_instance()
            .with(eq("id".to_string()))
            .return_once(|_| Ok(()));

        let mut instance = Ec2Instance {
            client: ec2_impl_mock,
            id: None,
            public_ip: Some("1.1.1.1".to_string()),
            public_dns: Some("example.com".to_string()),
            region: "us-west-2".to_string(),
            ami: "ami-830c94e3".to_string(),
            instance_type: aws_sdk_ec2::types::InstanceType::T2Micro,
            name: "test".to_string(),
            user_data: "test".to_string(),
            user_data_base64: "test".to_string(),
            instance_profile: None,
        };

        // Act
        let destroy_result = instance.destroy().await;

        // Assert
        assert!(destroy_result.is_err());

        assert!(instance.id == None);
        assert!(instance.public_ip == Some("1.1.1.1".to_string()));
        assert!(instance.public_dns == Some("example.com".to_string()));
    }
}
