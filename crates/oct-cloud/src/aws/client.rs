/// AWS service clients implementation
use aws_sdk_ec2::operation::run_instances::RunInstancesOutput;
use aws_sdk_ec2::types::{AttributeBooleanValue, IpPermission, IpRange};

use crate::aws::types::InstanceType;

#[cfg(test)]
use mockall::automock;

/// AWS EC2 client implementation
#[derive(Debug)]
pub(super) struct Ec2Impl {
    inner: aws_sdk_ec2::Client,
}

// TODO: Add tests using static replay
#[cfg_attr(test, allow(dead_code))]
#[cfg_attr(test, automock)]
impl Ec2Impl {
    pub(super) fn new(inner: aws_sdk_ec2::Client) -> Self {
        Self { inner }
    }

    /// Create VPC
    pub(super) async fn create_vpc(
        &self,
        cidr_block: String,
        name: String,
    ) -> Result<String, Box<dyn std::error::Error>> {
        log::info!("Creating VPC");

        let response = self
            .inner
            .create_vpc()
            .cidr_block(cidr_block)
            .tag_specifications(
                aws_sdk_ec2::types::TagSpecification::builder()
                    .resource_type(aws_sdk_ec2::types::ResourceType::Vpc)
                    .tags(
                        aws_sdk_ec2::types::Tag::builder()
                            .key("Name")
                            .value(name)
                            .build(),
                    )
                    .build(),
            )
            .send()
            .await?;

        let vpc_id = response
            .vpc()
            .and_then(|vpc| vpc.vpc_id())
            .ok_or("Failed to retrieve VPC ID")?
            .to_string();

        log::info!("Created VPC: {vpc_id}");

        Ok(vpc_id)
    }

    /// Delete VPC
    pub(super) async fn delete_vpc(
        &self,
        vpc_id: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Deleting VPC");

        self.inner
            .delete_vpc()
            .vpc_id(vpc_id.clone())
            .send()
            .await?;

        log::info!("Deleted VPC: {vpc_id}");

        Ok(())
    }

    /// Create Security Group
    pub(super) async fn create_security_group(
        &self,
        vpc_id: String,
        name: String,
        description: String,
    ) -> Result<String, Box<dyn std::error::Error>> {
        log::info!("Creating security group");

        let response = self
            .inner
            .create_security_group()
            .vpc_id(vpc_id)
            .group_name(name)
            .description(description)
            .send()
            .await?;

        let security_group_id = response
            .group_id()
            .ok_or("Failed to retrieve security group ID")?
            .to_string();

        log::info!("Created security group: {security_group_id}");

        Ok(security_group_id)
    }

    /// Delete Security Group
    pub(super) async fn delete_security_group(
        &self,
        security_group_id: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Deleting security group");

        self.inner
            .delete_security_group()
            .group_id(security_group_id.clone())
            .send()
            .await?;

        log::info!("Deleted security group: {security_group_id}");

        Ok(())
    }

    /// Allow inbound traffic for security group
    pub(super) async fn allow_inbound_traffic_for_security_group(
        &self,
        security_group_id: String,
        protocol: String,
        port: i32,
        cidr_block: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Allowing inbound traffic for security group");

        self.inner
            .authorize_security_group_ingress()
            .group_id(security_group_id.clone())
            .ip_permissions(
                IpPermission::builder()
                    .ip_protocol(protocol.clone())
                    .from_port(port)
                    .to_port(port)
                    .ip_ranges(IpRange::builder().cidr_ip(cidr_block.clone()).build())
                    .build(),
            )
            .send()
            .await?;

        log::info!("Added inbound rule {protocol} {port} {cidr_block} to security group {security_group_id}");

        Ok(())
    }

    /// Create Subnet
    pub(super) async fn create_subnet(
        &self,
        vpc_id: String,
        cidr_block: String,
        name: String,
    ) -> Result<String, Box<dyn std::error::Error>> {
        log::info!("Creating subnet");

        let response = self
            .inner
            .create_subnet()
            .vpc_id(vpc_id)
            .cidr_block(cidr_block)
            .tag_specifications(
                aws_sdk_ec2::types::TagSpecification::builder()
                    .resource_type(aws_sdk_ec2::types::ResourceType::Subnet)
                    .tags(
                        aws_sdk_ec2::types::Tag::builder()
                            .key("Name")
                            .value(name)
                            .build(),
                    )
                    .build(),
            )
            .send()
            .await?;

        let subnet_id = response
            .subnet()
            .and_then(|subnet| subnet.subnet_id())
            .ok_or("Failed to retrieve subnet ID")?
            .to_string();

        log::info!("Created subnet: {subnet_id}");

        Ok(subnet_id)
    }

    /// Delete Subnet
    pub(super) async fn delete_subnet(
        &self,
        subnet_id: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Deleting subnet");

        self.inner
            .delete_subnet()
            .subnet_id(subnet_id.clone())
            .send()
            .await?;

        log::info!("Deleted subnet: {subnet_id}");

        Ok(())
    }

    /// Create Internet Gateway
    pub(super) async fn create_internet_gateway(
        &self,
        vpc_id: String,
    ) -> Result<String, Box<dyn std::error::Error>> {
        log::info!("Creating Internet Gateway");

        let response = self.inner.create_internet_gateway().send().await?;
        let internet_gateway_id = response
            .internet_gateway()
            .and_then(|igw| igw.internet_gateway_id())
            .ok_or("Failed to retrieve Internet Gateway ID")?
            .to_string();

        log::info!("Created Internet Gateway: {internet_gateway_id}");

        log::info!("Attaching Internet Gateway {internet_gateway_id} to VPC");
        self.inner
            .attach_internet_gateway()
            .internet_gateway_id(internet_gateway_id.clone())
            .vpc_id(vpc_id.clone())
            .send()
            .await?;

        log::info!("Attached Internet Gateway {internet_gateway_id} to VPC");

        Ok(internet_gateway_id)
    }

    /// Delete Internet Gateway
    pub(super) async fn delete_internet_gateway(
        &self,
        internet_gateway_id: String,
        vpc_id: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Detaching Internet Gateway {internet_gateway_id} from VPC");

        self.inner
            .detach_internet_gateway()
            .internet_gateway_id(internet_gateway_id.clone())
            .vpc_id(vpc_id.clone())
            .send()
            .await?;

        log::info!("Detached Internet Gateway {internet_gateway_id} from VPC");

        log::info!("Deleting Internet Gateway");
        self.inner
            .delete_internet_gateway()
            .internet_gateway_id(internet_gateway_id.clone())
            .send()
            .await?;

        log::info!("Deleted Internet Gateway {internet_gateway_id} from VPC");

        Ok(())
    }

    /// Create Route Table
    pub(super) async fn create_route_table(
        &self,
        vpc_id: String,
    ) -> Result<String, Box<dyn std::error::Error>> {
        log::info!("Creating Route Table");

        let response = self
            .inner
            .create_route_table()
            .vpc_id(vpc_id.clone())
            .send()
            .await?;
        let route_table_id = response
            .route_table()
            .and_then(|rt| rt.route_table_id())
            .ok_or("Failed to retrieve Route Table ID")?
            .to_string();

        log::info!("Created Route Table: {route_table_id}");

        Ok(route_table_id)
    }

    /// Delete Route Table
    pub(super) async fn delete_route_table(
        &self,
        route_table_id: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Deleting Route Table {route_table_id}");

        self.inner
            .delete_route_table()
            .route_table_id(route_table_id.clone())
            .send()
            .await?;

        log::info!("Deleted Route Table {route_table_id}");

        Ok(())
    }

    /// Add public route to Route Table
    pub(super) async fn add_public_route(
        &self,
        route_table_id: String,
        igw_id: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Adding public route to Route Table {route_table_id}");
        self.inner
            .create_route()
            .route_table_id(route_table_id.clone())
            .gateway_id(igw_id.clone())
            .destination_cidr_block("0.0.0.0/0")
            .send()
            .await?;

        log::info!("Added public route to Route Table {route_table_id}");

        Ok(())
    }

    /// Associate Route Table with Subnet
    pub(super) async fn associate_route_table_with_subnet(
        &self,
        route_table_id: String,
        subnet_id: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Associating Route Table {route_table_id} with Subnet {subnet_id}");

        self.inner
            .associate_route_table()
            .route_table_id(route_table_id.clone())
            .subnet_id(subnet_id.clone())
            .send()
            .await?;

        log::info!("Associated Route Table {route_table_id} with Subnet {subnet_id}");

        Ok(())
    }

    /// Disassociate Route Table with Subnet
    pub(super) async fn disassociate_route_table_with_subnet(
        &self,
        route_table_id: String,
        subnet_id: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Disassociating Route Table {route_table_id} with Subnet {subnet_id}");

        let response = self
            .inner
            .describe_route_tables()
            .route_table_ids(route_table_id.clone())
            .send()
            .await?;

        // Extract association IDs
        let associations: Vec<String> = response
            .route_tables()
            .iter()
            .flat_map(|rt| rt.associations().iter())
            .filter_map(|assoc| assoc.route_table_association_id().map(str::to_string))
            .collect();

        if associations.is_empty() {
            log::warn!("No associations found for Route Table {route_table_id}");

            return Ok(());
        }

        // Disassociate each found Route Table Association
        for association_id in associations {
            log::info!("Disassociating Route Table {route_table_id} from {association_id}");
            self.inner
                .disassociate_route_table()
                .association_id(association_id.clone())
                .send()
                .await?;
        }

        for route_table in response.route_tables() {
            for route in route_table.routes() {
                if let Some(destination) = route.destination_cidr_block() {
                    if destination == "local" || destination.starts_with("10.0.0.") {
                        log::info!(
                            "Skipping local route {destination} in Route Table {route_table_id}"
                        );
                        continue;
                    }

                    log::info!("Deleting route {destination} from Route Table {route_table_id}");
                    self.inner
                        .delete_route()
                        .route_table_id(route_table_id.clone())
                        .destination_cidr_block(destination)
                        .send()
                        .await?;
                }
            }
        }

        log::info!("Disassociated Route Table {route_table_id} with Subnet {subnet_id}");

        Ok(())
    }

    /// Enable auto-assignment of public IP addresses for subnet
    pub(super) async fn enable_auto_assign_ip_addresses_for_subnet(
        &self,
        subnet_id: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Enabling auto-assignment of public IP addresses for Subnet {subnet_id}");

        self.inner
            .modify_subnet_attribute()
            .subnet_id(subnet_id.clone())
            .map_public_ip_on_launch(AttributeBooleanValue::builder().value(true).build())
            .send()
            .await?;

        log::info!("Enabled auto-assignment of public IP addresses for Subnet {subnet_id}");

        Ok(())
    }

    /// Retrieve metadata about specific EC2 instance
    pub(super) async fn describe_instances(
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
            .first()
            .ok_or("No reservations")?
            .instances()
            .first()
            .ok_or("No instances")?;

        Ok(instance.clone())
    }

    // TODO: Return Instance instead of response
    pub(super) async fn run_instances(
        &self,
        instance_type: InstanceType,
        ami: String,
        user_data_base64: String,
        instance_profile_name: String,
    ) -> Result<RunInstancesOutput, Box<dyn std::error::Error>> {
        log::info!("Starting EC2 instance");

        let response = self
            .inner
            .run_instances()
            .instance_type(instance_type.name.into())
            .image_id(ami.clone())
            .user_data(user_data_base64.clone())
            .iam_instance_profile(
                aws_sdk_ec2::types::IamInstanceProfileSpecification::builder()
                    .name(instance_profile_name)
                    .build(),
            )
            .min_count(1)
            .max_count(1)
            .send()
            .await?;

        log::info!("Created EC2 instance");

        Ok(response)
    }

    pub(super) async fn terminate_instance(
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

/// AWS IAM client implementation
#[derive(Debug)]
pub(super) struct IAMImpl {
    inner: aws_sdk_iam::Client,
}

// TODO: Add tests using static replay
#[cfg_attr(test, allow(dead_code))]
#[cfg_attr(test, automock)]
impl IAMImpl {
    pub(super) fn new(inner: aws_sdk_iam::Client) -> Self {
        Self { inner }
    }

    pub(super) async fn create_instance_iam_role(
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

    pub(super) async fn delete_instance_iam_role(
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

    pub(super) async fn create_instance_profile(
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

    pub(super) async fn delete_instance_profile(
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

// TODO: Is there a better way to expose mocked structs?
#[cfg(not(test))]
pub(super) use Ec2Impl as Ec2;
#[cfg(test)]
pub(super) use MockEc2Impl as Ec2;

#[cfg(not(test))]
pub(super) use IAMImpl as IAM;
#[cfg(test)]
pub(super) use MockIAMImpl as IAM;
