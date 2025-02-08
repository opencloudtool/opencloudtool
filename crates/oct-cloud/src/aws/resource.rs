use base64::{engine::general_purpose, Engine as _};

use crate::aws::client::{Ec2, IAM};
use crate::aws::types::InstanceType;
use crate::resource::Resource;

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

    pub instance_type: InstanceType,
    pub name: String,
    pub user_data: String,
    pub user_data_base64: String,
    pub vpc: VPC,

    // TODO: Make it required
    pub instance_profile: Option<InstanceProfile>,
}

impl Ec2Instance {
    const USER_DATA: &str = r#"#!/bin/bash
    set -e

    sudo apt update
    sudo apt -y install podman
    sudo systemctl start podman

    # aws ecr get-login-password --region us-west-2 | podman login --username AWS --password-stdin {ecr_repo_uri}

    curl \
        --output /home/ubuntu/oct-ctl \
        -L \
        https://github.com/opencloudtool/opencloudtool/releases/download/tip/oct-ctl \
        && sudo chmod +x /home/ubuntu/oct-ctl \
        && /home/ubuntu/oct-ctl &
    "#;

    pub async fn new(
        id: Option<String>,
        public_ip: Option<String>,
        public_dns: Option<String>,
        region: String,
        ami: String,
        instance_type: InstanceType,
        name: String,
        vpc: VPC,
        instance_profile: Option<InstanceProfile>,
    ) -> Self {
        let user_data_base64 = general_purpose::STANDARD.encode(Self::USER_DATA);

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
            user_data: Self::USER_DATA.to_string(),
            user_data_base64,
            vpc,
            instance_profile: Some(instance_profile),
        }
    }
}

impl Resource for Ec2Instance {
    async fn create(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        const MAX_ATTEMPTS: usize = 10;
        const SLEEP_DURATION: std::time::Duration = std::time::Duration::from_secs(5);

        // Create VPC
        self.vpc.create().await?;

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
                self.instance_profile.as_ref().map(|p| p.name.clone()),
            )
            .await?;

        // Extract instance id, public ip and dns
        let instance = response
            .instances()
            .first()
            .ok_or("No instances returned")?;

        self.id.clone_from(&instance.instance_id);

        // Poll for metadata
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

        // Destroy VPC
        self.vpc.destroy().await?;

        match &mut self.instance_profile {
            Some(instance_profile) => instance_profile.destroy().await,
            None => Ok(()),
        }
    }
}

#[derive(Debug)]
pub struct VPC {
    client: Ec2,

    // Know after creation
    pub id: Option<String>,

    pub region: String,
    pub cidr_block: String,
    pub name: String,

    pub subnet: Subnet,
}

impl VPC {
    const CIDR_BLOCK: &str = "10.0.0.0/16";

    pub async fn new(id: Option<String>, region: String, name: String, subnet: Subnet) -> Self {
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

        Self {
            client: Ec2::new(ec2_client),
            id,
            region,
            cidr_block: Self::CIDR_BLOCK.to_string(),
            name,
            subnet,
        }
    }
}

impl Resource for VPC {
    async fn create(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let vpc_id = self
            .client
            .create_vpc(self.cidr_block.clone(), self.name.clone())
            .await?;

        self.id = Some(vpc_id.clone());

        self.subnet.vpc_id = Some(vpc_id);

        self.subnet.create().await?;

        Ok(())
    }

    async fn destroy(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.subnet.destroy().await?;

        match self.id.clone() {
            Some(vpc_id) => {
                self.client.delete_vpc(vpc_id.clone()).await?;
            }
            None => {
                log::warn!("VPC not found");
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct Subnet {
    client: Ec2,

    // Know after creation
    pub id: Option<String>,

    pub region: String,
    pub cidr_block: String,

    // VPC id will be passed after vpc creation
    pub vpc_id: Option<String>,
    pub name: String,
}

impl Subnet {
    pub async fn new(
        id: Option<String>,
        region: String,
        cidr_block: String,
        vpc_id: Option<String>,
        name: String,
    ) -> Self {
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

        Self {
            client: Ec2::new(ec2_client),
            id,
            region,
            cidr_block,
            vpc_id,
            name,
        }
    }
}

impl Resource for Subnet {
    async fn create(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let subnet_id = self
            .client
            .create_subnet(
                self.vpc_id.clone().expect("vpc_id not set"),
                self.cidr_block.clone(),
                self.name.clone(),
            )
            .await?;

        // Extract subnet id
        self.id = Some(subnet_id);

        Ok(())
    }

    async fn destroy(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        match self.id.clone() {
            Some(subnet_id) => {
                self.client.delete_subnet(subnet_id.clone()).await?;
                self.id = None;
            }
            None => {
                log::warn!("Subnet not found");
            }
        }

        Ok(())
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

    use aws_sdk_ec2::operation::run_instances::RunInstancesOutput;
    use mockall::predicate::eq;

    #[tokio::test]
    async fn test_create_ec2_instance() {
        // Arrange
        let mut ec2_impl_vpc_mock = Ec2::default();
        ec2_impl_vpc_mock
            .expect_create_vpc()
            .with(eq("10.0.0.0/16".to_string()), eq("test".to_string()))
            .return_once(|_, _| Ok("vpc-12345".to_string()));

        let mut ec2_impl_subnet_mock = Ec2::default();
        ec2_impl_subnet_mock
            .expect_create_subnet()
            .with(
                eq("vpc-12345".to_string()),
                eq("10.0.0.0/24".to_string()),
                eq("test".to_string()),
            )
            .return_once(|_, _, _| Ok("subnet-12345".to_string()));

        let mut ec2_impl_mock = Ec2::default();
        ec2_impl_mock
            .expect_run_instances()
            .with(
                eq(InstanceType::T2_MICRO),
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
            instance_type: InstanceType::T2_MICRO,
            name: "test".to_string(),
            user_data: "test".to_string(),
            user_data_base64: "test".to_string(),
            vpc: VPC {
                client: ec2_impl_vpc_mock,
                id: None,
                region: "us-west-2".to_string(),
                cidr_block: "10.0.0.0/16".to_string(),
                name: "test".to_string(),
                subnet: Subnet {
                    client: ec2_impl_subnet_mock,
                    id: None,
                    region: "us-west-2".to_string(),
                    cidr_block: "10.0.0.0/24".to_string(),
                    vpc_id: None,
                    name: "test".to_string(),
                },
            },
            instance_profile: None,
        };

        // Act
        instance.create().await.unwrap();

        // Assert
        assert!(instance.id == Some("id".to_string()));
        assert!(instance.public_ip == Some("1.1.1.1".to_string()));
        assert!(instance.public_dns == Some("example.com".to_string()));
        assert!(instance.region == "us-west-2");
        assert!(instance.ami == "ami-830c94e3");
        assert!(instance.instance_type == InstanceType::T2_MICRO);
        assert!(instance.name == "test");
        assert!(instance.user_data == "test");
    }

    #[tokio::test]
    async fn test_create_ec2_instance_no_instance() {
        // Arrange
        let mut ec2_impl_vpc_mock = Ec2::default();
        ec2_impl_vpc_mock
            .expect_create_vpc()
            .with(eq("10.0.0.0/16".to_string()), eq("test".to_string()))
            .return_once(|_, _| Ok("vpc-12345".to_string()));

        let mut ec2_impl_subnet_mock = Ec2::default();
        ec2_impl_subnet_mock
            .expect_create_subnet()
            .with(
                eq("vpc-12345".to_string()),
                eq("10.0.0.0/24".to_string()),
                eq("test".to_string()),
            )
            .return_once(|_, _, _| Ok("subnet-12345".to_string()));

        let mut ec2_impl_mock = Ec2::default();
        ec2_impl_mock
            .expect_run_instances()
            .with(
                eq(InstanceType::T2_MICRO),
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
            instance_type: InstanceType::T2_MICRO,
            name: "test".to_string(),
            user_data: "test".to_string(),
            user_data_base64: "test".to_string(),
            vpc: VPC {
                client: ec2_impl_vpc_mock,
                id: None,
                region: "us-west-2".to_string(),
                cidr_block: "10.0.0.0/16".to_string(),
                name: "test".to_string(),
                subnet: Subnet {
                    client: ec2_impl_subnet_mock,
                    id: None,
                    region: "us-west-2".to_string(),
                    cidr_block: "10.0.0.0/24".to_string(),
                    vpc_id: None,
                    name: "test".to_string(),
                },
            },
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
        let mut ec2_impl_vpc_mock = Ec2::default();
        ec2_impl_vpc_mock
            .expect_delete_vpc()
            .with(eq("vpc-12345".to_string()))
            .return_once(|_| Ok(()));

        let mut ec2_impl_subnet_mock = Ec2::default();
        ec2_impl_subnet_mock
            .expect_create_subnet()
            .with(
                eq("vpc-12345".to_string()),
                eq("10.0.0.0/24".to_string()),
                eq("test".to_string()),
            )
            .return_once(|_, _, _| Ok("subnet-12345".to_string()));
        let mut ec2_impl_mock = Ec2::default();
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
            instance_type: InstanceType::T2_MICRO,
            name: "test".to_string(),
            user_data: "test".to_string(),
            user_data_base64: "test".to_string(),
            vpc: VPC {
                client: ec2_impl_vpc_mock,
                id: Some("vpc-12345".to_string()),
                region: "us-west-2".to_string(),
                cidr_block: "10.0.0.0/16".to_string(),
                name: "test".to_string(),
                subnet: Subnet {
                    client: ec2_impl_subnet_mock,
                    id: None,
                    region: "us-west-2".to_string(),
                    cidr_block: "10.0.0.0/24".to_string(),
                    vpc_id: None,
                    name: "test".to_string(),
                },
            },
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
        let mut ec2_impl_mock = Ec2::default();
        ec2_impl_mock
            .expect_terminate_instance()
            .with(eq("id".to_string()))
            .return_once(|_| Ok(()));

        let mut ec2_impl_subnet_mock = Ec2::default();
        ec2_impl_subnet_mock
            .expect_create_subnet()
            .with(
                eq("vpc-12345".to_string()),
                eq("10.0.0.0/24".to_string()),
                eq("test".to_string()),
            )
            .return_once(|_, _, _| Ok("subnet-12345".to_string()));

        let mut instance = Ec2Instance {
            client: ec2_impl_mock,
            id: None,
            public_ip: Some("1.1.1.1".to_string()),
            public_dns: Some("example.com".to_string()),
            region: "us-west-2".to_string(),
            ami: "ami-830c94e3".to_string(),
            instance_type: InstanceType::T2_MICRO,
            name: "test".to_string(),
            user_data: "test".to_string(),
            user_data_base64: "test".to_string(),
            vpc: VPC {
                client: Ec2::default(),
                id: None,
                region: "us-west-2".to_string(),
                cidr_block: "10.0.0.0/16".to_string(),
                name: "test".to_string(),
                subnet: Subnet {
                    client: ec2_impl_subnet_mock,
                    id: None,
                    region: "us-west-2".to_string(),
                    cidr_block: "10.0.0.0/24".to_string(),
                    vpc_id: None,
                    name: "test".to_string(),
                },
            },
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

    #[tokio::test]
    async fn test_create_instance_profile() {
        // Arrange
        let mut iam_impl_mock = IAM::default();
        iam_impl_mock
            .expect_create_instance_profile()
            .with(eq("test".to_string()), eq(vec![]))
            .return_once(|_, _| Ok(()));

        let mut instance_profile = InstanceProfile {
            client: iam_impl_mock,
            name: "test".to_string(),
            region: "us-west-2".to_string(),
            instance_roles: vec![],
        };

        // Act
        let create_result = instance_profile.create().await;

        // Assert
        assert!(create_result.is_ok());
    }

    #[tokio::test]
    async fn test_create_instance_profile_error() {
        // Arrange
        let mut iam_impl_mock = IAM::default();
        iam_impl_mock
            .expect_create_instance_profile()
            .with(eq("test".to_string()), eq(vec![]))
            .return_once(|_, _| Err("Error".into()));

        let mut instance_profile = InstanceProfile {
            client: iam_impl_mock,
            name: "test".to_string(),
            region: "us-west-2".to_string(),
            instance_roles: vec![],
        };

        // Act
        let create_result = instance_profile.create().await;

        // Assert
        assert!(create_result.is_err());
    }

    #[tokio::test]
    async fn test_destroy_instance_profile() {
        // Arrange
        let mut iam_impl_mock = IAM::default();
        iam_impl_mock
            .expect_delete_instance_profile()
            .with(eq("test".to_string()), eq(vec![]))
            .return_once(|_, _| Ok(()));

        let mut instance_profile = InstanceProfile {
            client: iam_impl_mock,
            name: "test".to_string(),
            region: "us-west-2".to_string(),
            instance_roles: vec![],
        };

        // Act
        let destroy_result = instance_profile.destroy().await;

        // Assert
        assert!(destroy_result.is_ok());
    }

    #[tokio::test]
    async fn test_destroy_instance_profile_error() {
        // Arrange
        let mut iam_impl_mock = IAM::default();
        iam_impl_mock
            .expect_delete_instance_profile()
            .with(eq("test".to_string()), eq(vec![]))
            .return_once(|_, _| Err("Error".into()));

        let mut instance_profile = InstanceProfile {
            client: iam_impl_mock,
            name: "test".to_string(),
            region: "us-west-2".to_string(),
            instance_roles: vec![],
        };

        // Act
        let destroy_result = instance_profile.destroy().await;

        // Assert
        assert!(destroy_result.is_err());
    }

    #[tokio::test]
    async fn test_create_instance_iam_role() {
        // Arrange
        let mut iam_impl_mock = IAM::default();
        iam_impl_mock
            .expect_create_instance_iam_role()
            .with(eq("test".to_string()), eq("".to_string()), eq(vec![]))
            .return_once(|_, _, _| Ok(()));

        let mut instance_role = InstanceRole {
            client: iam_impl_mock,
            name: "test".to_string(),
            region: "us-west-2".to_string(),
            assume_role_policy: "".to_string(),
            policy_arns: vec![],
        };

        // Act
        let create_result = instance_role.create().await;

        // Assert
        assert!(create_result.is_ok());
    }

    #[tokio::test]
    async fn test_create_instance_iam_role_error() {
        // Arrange
        let mut iam_impl_mock = IAM::default();
        iam_impl_mock
            .expect_create_instance_iam_role()
            .with(eq("test".to_string()), eq("".to_string()), eq(vec![]))
            .return_once(|_, _, _| Err("Error".into()));

        let mut instance_role = InstanceRole {
            client: iam_impl_mock,
            name: "test".to_string(),
            region: "us-west-2".to_string(),
            assume_role_policy: "".to_string(),
            policy_arns: vec![],
        };

        // Act
        let create_result = instance_role.create().await;

        // Assert
        assert!(create_result.is_err());
    }

    #[tokio::test]
    async fn test_destroy_instance_iam_role() {
        // Arrange
        let mut iam_impl_mock = IAM::default();
        iam_impl_mock
            .expect_delete_instance_iam_role()
            .with(eq("test".to_string()), eq(vec![]))
            .return_once(|_, _| Ok(()));

        let mut instance_role = InstanceRole {
            client: iam_impl_mock,
            name: "test".to_string(),
            region: "us-west-2".to_string(),
            assume_role_policy: "".to_string(),
            policy_arns: vec![],
        };

        // Act
        let destroy_result = instance_role.destroy().await;

        // Assert
        assert!(destroy_result.is_ok());
    }

    #[tokio::test]
    async fn test_destroy_instance_iam_role_error() {
        // Arrange
        let mut iam_impl_mock = IAM::default();
        iam_impl_mock
            .expect_delete_instance_iam_role()
            .with(eq("test".to_string()), eq(vec![]))
            .return_once(|_, _| Err("Error".into()));

        let mut instance_role = InstanceRole {
            client: iam_impl_mock,
            name: "test".to_string(),
            region: "us-west-2".to_string(),
            assume_role_policy: "".to_string(),
            policy_arns: vec![],
        };

        // Act
        let destroy_result = instance_role.destroy().await;

        // Assert
        assert!(destroy_result.is_err());
    }

    #[tokio::test]
    async fn test_create_vpc() {
        // Arrange
        let mut ec2_impl_mock = Ec2::default();

        ec2_impl_mock
            .expect_create_vpc()
            .with(eq("10.0.0.0/16".to_string()), eq("test".to_string()))
            .return_once(|_, _| Ok("vpc-12345".to_string()));

        let mut ec2_impl_subnet_mock = Ec2::default();
        ec2_impl_subnet_mock
            .expect_create_subnet()
            .with(
                eq("vpc-12345".to_string()),
                eq("10.0.0.0/24".to_string()),
                eq("test".to_string()),
            )
            .return_once(|_, _, _| Ok("subnet-12345".to_string()));

        let mut vpc = VPC {
            client: ec2_impl_mock,
            id: None,
            region: "us-west-2".to_string(),
            cidr_block: "10.0.0.0/16".to_string(),
            name: "test".to_string(),
            subnet: Subnet {
                client: ec2_impl_subnet_mock,
                id: None,
                region: "us-west-2".to_string(),
                cidr_block: "10.0.0.0/24".to_string(),
                vpc_id: None,
                name: "test".to_string(),
            },
        };

        // Act
        let create_result = vpc.create().await;

        // Assert
        assert!(create_result.is_ok());
        assert!(vpc.id == Some("vpc-12345".to_string()));
    }

    #[tokio::test]
    async fn test_create_vpc_error() {
        // Arrange
        let mut ec2_impl_mock = Ec2::default();
        ec2_impl_mock
            .expect_create_vpc()
            .with(eq("10.0.0.0/16".to_string()), eq("test".to_string()))
            .return_once(|_, _| Err("Error".into()));

        let mut ec2_impl_subnet_mock = Ec2::default();
        ec2_impl_subnet_mock
            .expect_create_subnet()
            .with(
                eq("vpc-12345".to_string()),
                eq("10.0.0.0/24".to_string()),
                eq("test".to_string()),
            )
            .return_once(|_, _, _| Ok("subnet-12345".to_string()));

        let mut vpc = VPC {
            client: ec2_impl_mock,
            id: None,
            region: "us-west-2".to_string(),
            cidr_block: "10.0.0.0/16".to_string(),
            name: "test".to_string(),
            subnet: Subnet {
                client: ec2_impl_subnet_mock,
                id: None,
                region: "us-west-2".to_string(),
                cidr_block: "10.0.0.0/24".to_string(),
                vpc_id: None,
                name: "test".to_string(),
            },
        };

        // Act
        let create_result = vpc.create().await;

        // Assert
        assert!(create_result.is_err());
    }

    #[tokio::test]
    async fn test_destroy_vpc() {
        // Arrange
        let mut ec2_impl_mock = Ec2::default();
        ec2_impl_mock
            .expect_delete_vpc()
            .with(eq("vpc-12345".to_string()))
            .return_once(|_| Ok(()));

        let mut ec2_impl_subnet_mock = Ec2::default();
        ec2_impl_subnet_mock
            .expect_create_subnet()
            .with(
                eq("vpc-12345".to_string()),
                eq("10.0.0.0/24".to_string()),
                eq("test".to_string()),
            )
            .return_once(|_, _, _| Ok("subnet-12345".to_string()));

        let mut vpc = VPC {
            client: ec2_impl_mock,
            id: Some("vpc-12345".to_string()),
            region: "us-west-2".to_string(),
            cidr_block: "10.0.0.0/16".to_string(),
            name: "test".to_string(),
            subnet: Subnet {
                client: ec2_impl_subnet_mock,
                id: None,
                region: "us-west-2".to_string(),
                cidr_block: "10.0.0.0/24".to_string(),
                vpc_id: None,
                name: "test".to_string(),
            },
        };

        // Act
        let destroy_result = vpc.destroy().await;

        // Assert
        assert!(destroy_result.is_ok());
    }

    #[tokio::test]
    async fn test_destroy_vpc_error() {
        // Arrange
        let mut ec2_impl_mock = Ec2::default();
        ec2_impl_mock
            .expect_delete_vpc()
            .with(eq("vpc-12345".to_string()))
            .return_once(|_| Err("Error".into()));

        let mut ec2_impl_subnet_mock = Ec2::default();
        ec2_impl_subnet_mock
            .expect_create_subnet()
            .with(
                eq("vpc-12345".to_string()),
                eq("10.0.0.0/24".to_string()),
                eq("test".to_string()),
            )
            .return_once(|_, _, _| Ok("subnet-12345".to_string()));

        let mut vpc = VPC {
            client: ec2_impl_mock,
            id: Some("vpc-12345".to_string()),
            region: "us-west-2".to_string(),
            cidr_block: "10.0.0.0/16".to_string(),
            name: "test".to_string(),
            subnet: Subnet {
                client: ec2_impl_subnet_mock,
                id: None,
                region: "us-west-2".to_string(),
                cidr_block: "10.0.0.0/24".to_string(),
                vpc_id: None,
                name: "test".to_string(),
            },
        };

        // Act
        let destroy_result = vpc.destroy().await;

        // Assert
        assert!(destroy_result.is_err());
    }

    #[tokio::test]
    async fn test_create_subnet() {
        // Arrange
        let mut ec2_impl_mock = Ec2::default();
        ec2_impl_mock
            .expect_create_subnet()
            .with(
                eq("vpc-12345".to_string()),
                eq("10.0.0.0/24".to_string()),
                eq("test".to_string()),
            )
            .return_once(|_, _, _| Ok("subnet-12345".to_string()));

        let mut subnet = Subnet {
            client: ec2_impl_mock,
            id: None,
            region: "us-west-2".to_string(),
            cidr_block: "10.0.0.0/24".to_string(),
            vpc_id: Some("vpc-12345".to_string()),
            name: "test".to_string(),
        };

        // Act
        let create_result = subnet.create().await;

        // Assert
        assert!(create_result.is_ok());
        assert!(subnet.id == Some("subnet-12345".to_string()));
    }

    #[tokio::test]
    async fn test_create_subnet_error() {
        // Arrange
        let mut ec2_impl_mock = Ec2::default();
        ec2_impl_mock
            .expect_create_subnet()
            .with(
                eq("vpc-12345".to_string()),
                eq("10.0.0.0/24".to_string()),
                eq("test".to_string()),
            )
            .return_once(|_, _, _| Err("Error".into()));

        let mut subnet = Subnet {
            client: ec2_impl_mock,
            id: None,
            region: "us-west-2".to_string(),
            cidr_block: "10.0.0.0/24".to_string(),
            vpc_id: Some("vpc-12345".to_string()),
            name: "test".to_string(),
        };

        // Act
        let create_result = subnet.create().await;

        // Assert
        assert!(create_result.is_err());
    }

    #[tokio::test]
    async fn test_destroy_subnet() {
        // Arrange
        let mut ec2_impl_mock = Ec2::default();
        ec2_impl_mock
            .expect_delete_subnet()
            .with(eq("subnet-12345".to_string()))
            .return_once(|_| Ok(()));

        let mut subnet = Subnet {
            client: ec2_impl_mock,
            id: Some("subnet-12345".to_string()),
            region: "us-west-2".to_string(),
            cidr_block: "10.0.0.0/24".to_string(),
            vpc_id: Some("vpc-12345".to_string()),
            name: "test".to_string(),
        };

        // Act
        let destroy_result = subnet.destroy().await;

        // Assert
        assert!(destroy_result.is_ok());
    }

    #[tokio::test]
    async fn test_destroy_subnet_error() {
        // Arrange
        let mut ec2_impl_mock = Ec2::default();
        ec2_impl_mock
            .expect_delete_subnet()
            .with(eq("subnet-12345".to_string()))
            .return_once(|_| Err("Error".into()));

        let mut subnet = Subnet {
            client: ec2_impl_mock,
            id: Some("subnet-12345".to_string()),
            region: "us-west-2".to_string(),
            cidr_block: "10.0.0.0/24".to_string(),
            vpc_id: Some("vpc-12345".to_string()),
            name: "test".to_string(),
        };

        // Act
        let destroy_result = subnet.destroy().await;

        // Assert
        assert!(destroy_result.is_err());
    }
}
