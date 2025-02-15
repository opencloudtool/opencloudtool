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

    pub instance_profile_name: String,
    pub subnet_id: String,
    pub security_group_id: String,
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
        instance_profile_name: String,
        subnet_id: String,
        security_group_id: String,
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
            instance_profile_name,
            subnet_id,
            security_group_id,
        }
    }
}

impl Resource for Ec2Instance {
    async fn create(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        const MAX_ATTEMPTS: usize = 10;
        const SLEEP_DURATION: std::time::Duration = std::time::Duration::from_secs(5);

        // Launch EC2 instance
        let response = self
            .client
            .run_instances(
                self.instance_type.clone(),
                self.ami.clone(),
                self.user_data_base64.clone(),
                self.instance_profile_name.clone(),
                self.subnet_id.clone(),
                self.security_group_id.clone(),
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

        Ok(())
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

    // Not all VPCs will have an Internet Gateway
    pub internet_gateway: Option<InternetGateway>,

    pub route_table: RouteTable,

    pub security_group: SecurityGroup,
}

impl VPC {
    pub async fn new(
        id: Option<String>,
        region: String,
        cidr_block: String,
        name: String,
        subnet: Subnet,

        internet_gateway: Option<InternetGateway>,

        route_table: RouteTable,
        security_group: SecurityGroup,
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
            name,
            subnet,
            internet_gateway,
            route_table,
            security_group,
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

        // Create Subnet
        self.subnet.vpc_id = Some(vpc_id.clone());
        self.subnet.create().await?;

        // Create Route Table
        // FYI, there is a default route table created for a VPC
        self.route_table.vpc_id = Some(vpc_id.clone());
        self.route_table.subnet_id = Some(self.subnet.id.clone().expect("subnet_id not set"));
        self.route_table.create().await?;

        // Create Security Group
        self.security_group.vpc_id = Some(vpc_id.clone());
        self.security_group.create().await?;

        // Create Internet Gateway
        match &mut self.internet_gateway {
            Some(internet_gateway) => {
                internet_gateway.vpc_id = Some(vpc_id.clone());
                internet_gateway.route_table_id =
                    Some(self.route_table.id.clone().expect("route_table_id not set"));
                internet_gateway.subnet_id =
                    Some(self.subnet.id.clone().expect("subnet_id not set"));
                internet_gateway.create().await?;
            }
            None => log::info!("No Internet Gateway created, using a private VPC."),
        }

        Ok(())
    }

    async fn destroy(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Delete Route Table
        self.route_table.destroy().await?;

        // Wait for route table to be deleted
        log::info!("Waiting for Public IPs to be deleted");
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;

        // Delete Internet Gateway
        match &mut self.internet_gateway {
            Some(internet_gateway) => internet_gateway.destroy().await?,
            None => log::info!("No Internet Gateway was created, skipping deletion."),
        }

        // Delete security group
        self.security_group.destroy().await?;

        // Delete Subnet
        self.subnet.destroy().await?;

        // Delete VPC
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
pub struct InternetGateway {
    client: Ec2,

    pub id: Option<String>,

    pub vpc_id: Option<String>,
    pub route_table_id: Option<String>,
    pub subnet_id: Option<String>,

    pub region: String,
}

impl InternetGateway {
    pub async fn new(
        id: Option<String>,
        vpc_id: Option<String>,
        route_table_id: Option<String>,
        subnet_id: Option<String>,
        region: String,
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
            vpc_id,
            route_table_id,
            subnet_id,
            region,
        }
    }
}

impl Resource for InternetGateway {
    async fn create(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let internet_gateway_id = self
            .client
            .create_internet_gateway(self.vpc_id.clone().expect("vpc_id not set"))
            .await?;

        self.id = Some(internet_gateway_id.clone());

        // Add public route to Route Table
        self.client
            .add_public_route(
                self.route_table_id.clone().expect("route_table_id not set"),
                internet_gateway_id.clone(),
            )
            .await?;

        // Enable auto-assignment of public IP addresses for subnet
        self.client
            .enable_auto_assign_ip_addresses_for_subnet(
                self.subnet_id.clone().expect("subnet_id not set"),
            )
            .await?;

        Ok(())
    }

    async fn destroy(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        match self.id.clone() {
            Some(internet_gateway_id) => {
                self.client
                    .delete_internet_gateway(
                        internet_gateway_id.clone(),
                        self.vpc_id.clone().expect("vpc_id not set"),
                    )
                    .await?;
                self.id = None;
            }
            None => {
                log::warn!("Internet gateway not found");
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct RouteTable {
    client: Ec2,

    pub id: Option<String>,

    pub vpc_id: Option<String>,
    pub subnet_id: Option<String>,

    pub region: String,
}

impl RouteTable {
    pub async fn new(
        id: Option<String>,
        vpc_id: Option<String>,
        subnet_id: Option<String>,
        region: String,
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
            vpc_id,
            subnet_id,
            region,
        }
    }
}

impl Resource for RouteTable {
    async fn create(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let route_table_id = self
            .client
            .create_route_table(self.vpc_id.clone().expect("vpc_id not set"))
            .await?;

        self.id = Some(route_table_id.clone());

        self.client
            .associate_route_table_with_subnet(
                route_table_id.clone(),
                self.subnet_id.clone().expect("subnet_id not set"),
            )
            .await?;

        Ok(())
    }

    async fn destroy(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        match self.id.clone() {
            Some(route_table_id) => {
                self.client
                    .disassociate_route_table_with_subnet(
                        route_table_id.clone(),
                        self.subnet_id.clone().expect("subnet_id not set"),
                    )
                    .await?;
                self.client
                    .delete_route_table(route_table_id.clone())
                    .await?;
                self.id = None;
            }
            None => {
                log::warn!("Route table not found");
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct SecurityGroup {
    client: Ec2,

    pub id: Option<String>,

    pub name: String,
    pub vpc_id: Option<String>,
    pub description: String,
    pub region: String,
    pub inbound_rules: Vec<InboundRule>,
}

impl SecurityGroup {
    pub async fn new(
        id: Option<String>,
        name: String,
        vpc_id: Option<String>,
        description: String,
        region: String,
        inbound_rules: Vec<InboundRule>,
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
            name,
            vpc_id,
            description,
            region,
            inbound_rules,
        }
    }
}

impl Resource for SecurityGroup {
    async fn create(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let security_group_id = self
            .client
            .create_security_group(
                self.vpc_id.clone().expect("vpc_id not set"),
                self.name.clone(),
                self.description.clone(),
            )
            .await?;

        self.id = Some(security_group_id.clone());

        for rule in &self.inbound_rules {
            self.client
                .allow_inbound_traffic_for_security_group(
                    security_group_id.clone(),
                    rule.protocol.clone(),
                    rule.port,
                    rule.cidr_block.clone(),
                )
                .await?;
        }

        Ok(())
    }

    async fn destroy(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        match self.id.clone() {
            Some(security_group_id) => {
                self.client
                    .delete_security_group(security_group_id.clone())
                    .await?;
                self.id = None;
            }
            None => {
                log::warn!("Security group not found");
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct InboundRule {
    pub protocol: String,
    pub port: i32,
    pub cidr_block: String,
}

impl InboundRule {
    pub fn new(protocol: String, port: i32, cidr_block: String) -> Self {
        Self {
            protocol,
            port,
            cidr_block,
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
    pub async fn new(name: String, region: String, instance_roles: Vec<InstanceRole>) -> Self {
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
            name,
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

    pub async fn new(name: String, region: String) -> Self {
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
            name,
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

        let mut ec2_impl_security_group_mock = Ec2::default();
        ec2_impl_security_group_mock
            .expect_create_security_group()
            .with(
                eq("vpc-12345".to_string()),
                eq("ct-app-security-group".to_string()),
                eq("ct-app-security-group".to_string()),
            )
            .return_once(|_, _, _| Ok("sg-12345".to_string()));

        ec2_impl_security_group_mock
            .expect_allow_inbound_traffic_for_security_group()
            .with(
                eq("sg-12345".to_string()),
                eq("tcp".to_string()),
                eq(22),
                eq("10.0.0.0/16".to_string()),
            )
            .return_once(|_, _, _, _| Ok(()));

        let mut ec2_impl_route_table_mock = Ec2::default();
        ec2_impl_route_table_mock
            .expect_create_route_table()
            .with(eq("vpc-12345".to_string()))
            .return_once(|_| Ok("rtb-12345".to_string()));

        ec2_impl_route_table_mock
            .expect_associate_route_table_with_subnet()
            .with(eq("rtb-12345".to_string()), eq("subnet-12345".to_string()))
            .return_once(|_, _| Ok(()));

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
                eq("instance_profile".to_string()),
                eq("subnet-12345".to_string()),
                eq("sg-12345".to_string()),
            )
            .return_once(|_, _, _, _, _, _| {
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
            instance_profile_name: "instance_profile".to_string(),
            subnet_id: "subnet-12345".to_string(),
            security_group_id: "sg-12345".to_string(),
        };

        // Act
        instance.create().await.unwrap();

        // Assert
        assert_eq!(instance.id, Some("id".to_string()));
        assert_eq!(instance.public_ip, Some("1.1.1.1".to_string()));
        assert_eq!(instance.public_dns, Some("example.com".to_string()));
        assert_eq!(instance.region, "us-west-2");
        assert_eq!(instance.ami, "ami-830c94e3");
        assert_eq!(instance.instance_type, InstanceType::T2_MICRO);
        assert_eq!(instance.name, "test");
        assert_eq!(instance.user_data, "test");
        assert_eq!(instance.user_data_base64, "test");
        assert_eq!(instance.instance_profile_name, "instance_profile");
        assert_eq!(instance.subnet_id, "subnet-12345".to_string());
    }

    #[tokio::test]
    async fn test_create_ec2_instance_no_instance() {
        // Arrange
        let mut ec2_impl_vpc_mock = Ec2::default();
        ec2_impl_vpc_mock
            .expect_create_vpc()
            .with(eq("10.0.0.0/16".to_string()), eq("test".to_string()))
            .return_once(|_, _| Ok("vpc-12345".to_string()));

        let mut ec2_impl_security_group_mock = Ec2::default();
        ec2_impl_security_group_mock
            .expect_create_security_group()
            .with(
                eq("vpc-12345".to_string()),
                eq("ct-app-security-group".to_string()),
                eq("ct-app-security-group".to_string()),
            )
            .return_once(|_, _, _| Ok("sg-12345".to_string()));

        ec2_impl_security_group_mock
            .expect_allow_inbound_traffic_for_security_group()
            .with(
                eq("sg-12345".to_string()),
                eq("tcp".to_string()),
                eq(22),
                eq("10.0.0.0/16".to_string()),
            )
            .return_once(|_, _, _, _| Ok(()));

        let mut ec2_impl_route_table_mock = Ec2::default();
        ec2_impl_route_table_mock
            .expect_create_route_table()
            .with(eq("vpc-12345".to_string()))
            .return_once(|_| Ok("rtb-12345".to_string()));

        ec2_impl_route_table_mock
            .expect_associate_route_table_with_subnet()
            .with(eq("rtb-12345".to_string()), eq("subnet-12345".to_string()))
            .return_once(|_, _| Ok(()));

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
                eq("instance_profile".to_string()),
                eq("subnet-12345".to_string()),
                eq("sg-12345".to_string()),
            )
            .return_once(|_, _, _, _, _, _| Ok(RunInstancesOutput::builder().build()));

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
            instance_profile_name: "instance_profile".to_string(),
            subnet_id: "subnet-12345".to_string(),
            security_group_id: "sg-12345".to_string(),
        };

        // Act
        let creation_result = instance.create().await;

        // Assert
        assert!(creation_result.is_err());

        assert_eq!(instance.id, None);
        assert_eq!(instance.public_ip, None);
        assert_eq!(instance.public_dns, None);
    }

    #[tokio::test]
    async fn test_destroy_ec2_instance() {
        // Arrange
        let mut ec2_impl_vpc_mock = Ec2::default();
        ec2_impl_vpc_mock
            .expect_delete_vpc()
            .with(eq("vpc-12345".to_string()))
            .return_once(|_| Ok(()));

        let mut ec2_impl_security_group_mock = Ec2::default();
        ec2_impl_security_group_mock
            .expect_create_security_group()
            .with(
                eq("vpc-12345".to_string()),
                eq("test".to_string()),
                eq("test_description".to_string()),
            )
            .return_once(|_, _, _| Ok("sg-12345".to_string()));

        ec2_impl_security_group_mock
            .expect_allow_inbound_traffic_for_security_group()
            .with(
                eq("sg-12345".to_string()),
                eq("tcp".to_string()),
                eq(22),
                eq("10.0.0.0/16".to_string()),
            )
            .return_once(|_, _, _, _| Ok(()));

        let mut ec2_impl_route_table_mock = Ec2::default();
        ec2_impl_route_table_mock
            .expect_create_route_table()
            .with(eq("vpc-12345".to_string()))
            .return_once(|_| Ok("rtb-12345".to_string()));

        ec2_impl_route_table_mock
            .expect_associate_route_table_with_subnet()
            .with(eq("rtb-12345".to_string()), eq("subnet-12345".to_string()))
            .return_once(|_, _| Ok(()));

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
            instance_profile_name: "instance_profile".to_string(),
            subnet_id: "subnet-12345".to_string(),
            security_group_id: "sg-12345".to_string(),
        };

        // Act
        instance.destroy().await.unwrap();

        // Assert
        assert_eq!(instance.id, None);
        assert_eq!(instance.public_ip, None);
        assert_eq!(instance.public_dns, None);
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
            instance_profile_name: "instance_profile".to_string(),
            subnet_id: "subnet-12345".to_string(),
            security_group_id: "sg-12345".to_string(),
        };

        // Act
        let destroy_result = instance.destroy().await;

        // Assert
        assert!(destroy_result.is_err());

        assert_eq!(instance.id, None);
        assert_eq!(instance.public_ip, Some("1.1.1.1".to_string()));
        assert_eq!(instance.public_dns, Some("example.com".to_string()));
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

        let mut ec2_impl_security_group_mock = Ec2::default();
        ec2_impl_security_group_mock
            .expect_create_security_group()
            .with(
                eq("vpc-12345".to_string()),
                eq("ct-app-security-group".to_string()),
                eq("ct-app-security-group".to_string()),
            )
            .return_once(|_, _, _| Ok("sg-12345".to_string()));

        ec2_impl_security_group_mock
            .expect_allow_inbound_traffic_for_security_group()
            .with(
                eq("sg-12345".to_string()),
                eq("tcp".to_string()),
                eq(22),
                eq("10.0.0.0/16".to_string()),
            )
            .return_once(|_, _, _, _| Ok(()));

        let mut ec2_impl_route_table_mock = Ec2::default();
        ec2_impl_route_table_mock
            .expect_create_route_table()
            .with(eq("vpc-12345".to_string()))
            .return_once(|_| Ok("rtb-12345".to_string()));

        ec2_impl_route_table_mock
            .expect_associate_route_table_with_subnet()
            .with(eq("rtb-12345".to_string()), eq("subnet-12345".to_string()))
            .return_once(|_, _| Ok(()));

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
            internet_gateway: None,
            route_table: RouteTable {
                client: ec2_impl_route_table_mock,
                id: None,
                vpc_id: None,
                subnet_id: None,
                region: "us-west-2".to_string(),
            },
            security_group: SecurityGroup {
                client: ec2_impl_security_group_mock,
                id: None,
                name: "ct-app-security-group".to_string(),
                vpc_id: None,
                description: "ct-app-security-group".to_string(),
                region: "us-west-2".to_string(),
                inbound_rules: vec![],
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
                client: Ec2::default(),
                id: None,
                region: "us-west-2".to_string(),
                cidr_block: "10.0.0.0/24".to_string(),
                vpc_id: None,
                name: "test".to_string(),
            },
            internet_gateway: None,
            route_table: RouteTable {
                client: Ec2::default(),
                id: None,
                vpc_id: None,
                subnet_id: None,
                region: "us-west-2".to_string(),
            },
            security_group: SecurityGroup {
                client: Ec2::default(),
                id: None,
                name: "ct-app-security-group".to_string(),
                vpc_id: None,
                description: "ct-app-security-group".to_string(),
                region: "us-west-2".to_string(),
                inbound_rules: vec![],
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
                client: Ec2::default(),
                id: None,
                region: "us-west-2".to_string(),
                cidr_block: "10.0.0.0/24".to_string(),
                vpc_id: None,
                name: "test".to_string(),
            },
            internet_gateway: None,
            route_table: RouteTable {
                client: Ec2::default(),
                id: None,
                vpc_id: None,
                subnet_id: None,
                region: "us-west-2".to_string(),
            },
            security_group: SecurityGroup {
                client: Ec2::default(),
                id: None,
                name: "ct-app-security-group".to_string(),
                vpc_id: None,
                description: "ct-app-security-group".to_string(),
                region: "us-west-2".to_string(),
                inbound_rules: vec![],
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
                client: Ec2::default(),
                id: None,
                region: "us-west-2".to_string(),
                cidr_block: "10.0.0.0/24".to_string(),
                vpc_id: None,
                name: "test".to_string(),
            },
            internet_gateway: None,
            route_table: RouteTable {
                client: Ec2::default(),
                id: None,
                vpc_id: None,
                subnet_id: None,
                region: "us-west-2".to_string(),
            },
            security_group: SecurityGroup {
                client: Ec2::default(),
                id: None,
                name: "ct-app-security-group".to_string(),
                vpc_id: None,
                description: "ct-app-security-group".to_string(),
                region: "us-west-2".to_string(),
                inbound_rules: vec![],
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
