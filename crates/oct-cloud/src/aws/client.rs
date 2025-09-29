/// AWS service clients implementation
use aws_sdk_ec2::operation::run_instances::RunInstancesOutput;
use aws_sdk_ec2::types::{AttributeBooleanValue, IpPermission, IpRange};
use aws_sdk_route53::types::ChangeAction;
use uuid::Uuid;

use crate::aws::types::{InstanceType, RecordType};

#[cfg(test)]
use mockall::automock;

pub(super) struct S3Impl {
    inner: aws_sdk_s3::Client,
}

#[cfg_attr(test, allow(dead_code))]
#[cfg_attr(test, automock)]
impl S3Impl {
    pub(super) fn new(inner: aws_sdk_s3::Client) -> Self {
        Self { inner }
    }

    pub(super) async fn create_bucket(
        &self,
        region: &str,
        name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let response = self
            .inner
            .create_bucket()
            .create_bucket_configuration(
                aws_sdk_s3::types::CreateBucketConfiguration::builder()
                    .location_constraint(region.into())
                    .build(),
            )
            .bucket(name)
            .send()
            .await;

        match response {
            Ok(_) => Ok(()),
            Err(sdk_err) => {
                match sdk_err.into_service_error() {
                    aws_sdk_s3::operation::create_bucket::CreateBucketError::BucketAlreadyOwnedByYou(_) => Ok(()),
                    aws_sdk_s3::operation::create_bucket::CreateBucketError::BucketAlreadyExists(_) => Ok(()),
                    err => Err(Box::new(err)),
                }
            }
        }
    }

    pub(super) async fn delete_bucket(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.inner.delete_bucket().bucket(name).send().await?;

        Ok(())
    }

    pub(crate) async fn put_object(
        &self,
        bucket_name: &str,
        key: &str,
        data: Vec<u8>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.inner
            .put_object()
            .bucket(bucket_name)
            .key(key)
            .body(data.into())
            .send()
            .await?;

        Ok(())
    }

    pub(crate) async fn get_object(
        &self,
        bucket_name: &str,
        key: &str,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let response = self
            .inner
            .get_object()
            .bucket(bucket_name)
            .key(key)
            .send()
            .await?;

        Ok(response.body.collect().await?.to_vec())
    }

    pub(crate) async fn delete_object(
        &self,
        bucket_name: &str,
        key: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.inner
            .delete_object()
            .bucket(bucket_name)
            .key(key)
            .send()
            .await?;

        Ok(())
    }
}

/// AWS EC2 client implementation
#[derive(Debug)]
pub struct Ec2Impl {
    inner: aws_sdk_ec2::Client,
}

// TODO: Add tests using static replay
#[cfg_attr(test, allow(dead_code))]
#[cfg_attr(test, automock)]
impl Ec2Impl {
    pub fn new(inner: aws_sdk_ec2::Client) -> Self {
        Self { inner }
    }

    /// Create VPC
    pub async fn create_vpc(
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
    pub async fn delete_vpc(&self, vpc_id: String) -> Result<(), Box<dyn std::error::Error>> {
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
    pub async fn create_security_group(
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

    /// Describe Security Group
    pub async fn get_default_security_group_id(
        &self,
        vpc_id: String,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let response = self
            .inner
            .get_security_groups_for_vpc()
            .vpc_id(vpc_id)
            .send()
            .await?;

        Ok(response
            .security_group_for_vpcs()
            .first()
            .expect("Failed to get the default security group as the first element")
            .group_id()
            .ok_or("Failed to get security group id")?
            .to_string())
    }

    /// Delete Security Group
    pub async fn delete_security_group(
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
    pub async fn allow_inbound_traffic_for_security_group(
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

        log::info!(
            "Added inbound rule {protocol} {port} {cidr_block} to security group {security_group_id}"
        );

        Ok(())
    }

    /// Create Subnet
    pub async fn create_subnet(
        &self,
        vpc_id: String,
        cidr_block: String,
        availability_zone: String,
        name: String,
    ) -> Result<String, Box<dyn std::error::Error>> {
        log::info!("Creating subnet");

        let response = self
            .inner
            .create_subnet()
            .vpc_id(vpc_id)
            .cidr_block(cidr_block)
            .availability_zone(availability_zone)
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
    pub async fn delete_subnet(&self, subnet_id: String) -> Result<(), Box<dyn std::error::Error>> {
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
    pub async fn create_internet_gateway(
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
    pub async fn delete_internet_gateway(
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
    pub async fn create_route_table(
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
    pub async fn delete_route_table(
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
    pub async fn add_public_route(
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
    pub async fn associate_route_table_with_subnet(
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
    pub async fn disassociate_route_table_with_subnet(
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
    pub async fn enable_auto_assign_ip_addresses_for_subnet(
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
    pub async fn describe_instances(
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
    pub async fn run_instances(
        &self,
        instance_type: InstanceType,
        ami: String,
        user_data_base64: String,
        instance_profile_name: String,
        subnet_id: String,
        security_group_id: String,
    ) -> Result<RunInstancesOutput, Box<dyn std::error::Error>> {
        log::info!("Starting EC2 instance");

        let request = self
            .inner
            .run_instances()
            .instance_type(instance_type.as_str().into())
            .image_id(ami.clone())
            .user_data(user_data_base64.clone())
            .subnet_id(subnet_id)
            .min_count(1)
            .max_count(1)
            .block_device_mappings(
                aws_sdk_ec2::types::BlockDeviceMapping::builder()
                    .device_name("/dev/sda1")
                    .ebs(
                        aws_sdk_ec2::types::EbsBlockDevice::builder()
                            .volume_size(100)
                            .volume_type(aws_sdk_ec2::types::VolumeType::Gp3)
                            .delete_on_termination(true)
                            .build(),
                    )
                    .build(),
            )
            .iam_instance_profile(
                aws_sdk_ec2::types::IamInstanceProfileSpecification::builder()
                    .name(instance_profile_name)
                    .build(),
            )
            .security_group_ids(security_group_id);

        let response = request.send().await?;

        log::info!("Created EC2 instance");

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

/// AWS Route53 client implementation
#[derive(Debug)]
pub struct Route53Impl {
    inner: aws_sdk_route53::Client,
}

/// Represents a DNS record.
///
/// The tuple contains the following elements in order:
/// 1. The domain name of the record.
/// 2. The type of the DNS record (e.g., A, AAAA, CNAME, etc.).
/// 3. The value of the record (e.g., an IP address).
/// 4. The time-to-live (TTL) for the record in seconds.
pub type DnsRecord = (String, RecordType, String, Option<i64>);

#[cfg_attr(test, allow(dead_code))]
#[cfg_attr(test, automock)]
impl Route53Impl {
    pub fn new(inner: aws_sdk_route53::Client) -> Self {
        Self { inner }
    }

    pub async fn create_hosted_zone(
        &self,
        domain_name: String,
    ) -> Result<String, Box<dyn std::error::Error>> {
        log::info!("Creating Route53 hosted zone for {domain_name}");

        let response = self
            .inner
            .create_hosted_zone()
            .name(domain_name.clone())
            .caller_reference(Uuid::new_v4().to_string())
            .send()
            .await?;

        log::info!("Created Route53 hosted zone");

        let hosted_zone_id = response
            .hosted_zone()
            .ok_or("Failed to retrieve hosted zone")?
            .id()
            .to_string();

        log::info!("Getting DNS records for hosted zone");

        let dns_records_data = self.get_dns_records(hosted_zone_id.clone()).await?;

        log::info!("Please map these NS records in your domain provider:");

        for (_name, record_type, value, _ttl) in dns_records_data {
            if record_type == RecordType::NS {
                log::info!("{value}");
            }
        }

        self.check_ns_records(&domain_name, &hosted_zone_id).await?;

        Ok(hosted_zone_id)
    }

    async fn check_ns_records(
        &self,
        domain_name: &str,
        hosted_zone_id: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let verification_id = Uuid::new_v4().simple().to_string();
        let subdomain = format!("_verify.{domain_name}");

        self.create_dns_record(
            hosted_zone_id.to_string(),
            subdomain.clone(),
            RecordType::TXT,
            format!("\"{verification_id}\""),
            Some(300),
        )
        .await?;

        log::info!("Record created: {subdomain} - {verification_id}");

        log::info!("Checking record...");

        let mut attempts = 0;
        let max_attempts = 70;
        let sleep_duration_s = 5;

        while attempts < max_attempts {
            let output = std::process::Command::new("dig")
                .arg("-t")
                .arg("TXT")
                .arg(subdomain.clone())
                .arg("+short")
                .output()?;

            if output.status.success() {
                let output = String::from_utf8_lossy(&output.stdout);
                if output.contains(&verification_id) {
                    log::info!("Record verified");

                    return Ok(());
                }
            }

            log::info!(
                "Record not verified. \
                Retrying in {sleep_duration_s} seconds..."
            );

            attempts += 1;
            tokio::time::sleep(std::time::Duration::from_secs(sleep_duration_s)).await;
        }

        Err("Failed to verify record".into())
    }

    pub async fn delete_hosted_zone(&self, id: String) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Deleting Route53 hosted zone {id}");
        // List all record sets
        let record_sets = self
            .inner
            .list_resource_record_sets()
            .hosted_zone_id(id.clone())
            .send()
            .await?
            .resource_record_sets()
            .to_vec();

        // Filter out NS and SOA, and delete the rest
        for record_set in record_sets {
            if record_set.r#type().as_str() != "NS" && record_set.r#type().as_str() != "SOA" {
                for record in record_set.resource_records() {
                    self.delete_dns_record(
                        id.clone(),
                        record_set.name().to_string(),
                        RecordType::from(record_set.r#type().clone()),
                        record.value().to_string(),
                        record_set.ttl(),
                    )
                    .await?;
                }
            }
        }

        log::info!("Deleted non-default record sets from hosted zone {id}");

        self.inner
            .delete_hosted_zone()
            .id(id.clone())
            .send()
            .await?;

        log::info!("Deleted Route53 hosted zone {id}");

        Ok(())
    }

    pub async fn get_dns_records(
        &self,
        hosted_zone_id: String,
    ) -> Result<Vec<DnsRecord>, Box<dyn std::error::Error>> {
        log::info!("Getting DNS records for {hosted_zone_id}");

        let response = self
            .inner
            .list_resource_record_sets()
            .hosted_zone_id(hosted_zone_id)
            .send()
            .await?;

        let resource_record_sets = response.resource_record_sets().to_vec();

        let mut result = Vec::new();
        for record_set in resource_record_sets {
            for record in record_set.resource_records() {
                let name = record_set.name().to_string();
                let record_type = RecordType::from(record_set.r#type().clone());
                let value = record.value().to_string();
                let ttl = record_set.ttl;

                result.push((name, record_type, value, ttl));
            }
        }

        Ok(result)
    }

    async fn change_dns_record(
        &self,
        hosted_zone_id: String,
        domain_name: String,
        record_type: RecordType,
        record_value: String,
        ttl: Option<i64>,
        action: ChangeAction,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Starting to {action} {record_type} record for {domain_name}");

        let resource_record = aws_sdk_route53::types::ResourceRecord::builder()
            .value(record_value)
            .build()?;

        let record_set = aws_sdk_route53::types::ResourceRecordSet::builder()
            .name(domain_name.clone())
            .r#type(record_type.into())
            .ttl(ttl.unwrap_or(3600))
            .resource_records(resource_record)
            .build()?;

        let change = aws_sdk_route53::types::Change::builder()
            .action(action.clone())
            .resource_record_set(record_set)
            .build()?;

        let changes = aws_sdk_route53::types::ChangeBatch::builder()
            .changes(change)
            .build()?;

        self.inner
            .change_resource_record_sets()
            .hosted_zone_id(hosted_zone_id)
            .change_batch(changes)
            .send()
            .await?;

        log::info!("Finished to {action} {record_type} record for {domain_name}");

        Ok(())
    }

    pub async fn create_dns_record(
        &self,
        hosted_zone_id: String,
        domain_name: String,
        record_type: RecordType,
        record_value: String,
        ttl: Option<i64>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.change_dns_record(
            hosted_zone_id,
            domain_name,
            record_type,
            record_value,
            ttl,
            ChangeAction::Create,
        )
        .await
    }

    pub async fn delete_dns_record(
        &self,
        hosted_zone_id: String,
        domain_name: String,
        record_type: RecordType,
        record_value: String,
        ttl: Option<i64>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.change_dns_record(
            hosted_zone_id,
            domain_name,
            record_type,
            record_value,
            ttl,
            ChangeAction::Delete,
        )
        .await
    }
}

/// AWS IAM client implementation
#[derive(Debug)]
pub struct IAMImpl {
    inner: aws_sdk_iam::Client,
}

// TODO: Add tests using static replay
#[cfg_attr(test, allow(dead_code))]
#[cfg_attr(test, automock)]
impl IAMImpl {
    pub fn new(inner: aws_sdk_iam::Client) -> Self {
        Self { inner }
    }

    pub async fn create_instance_iam_role(
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

    pub async fn delete_instance_iam_role(
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

    pub async fn create_instance_profile(
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

    pub async fn delete_instance_profile(
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

/// AWS ECR client implementation
#[derive(Debug)]
pub struct ECRImpl {
    inner: aws_sdk_ecr::Client,
}

// TODO: Add tests using static replay
#[cfg_attr(test, allow(dead_code))]
#[cfg_attr(test, automock)]
impl ECRImpl {
    pub fn new(inner: aws_sdk_ecr::Client) -> Self {
        Self { inner }
    }

    pub async fn create_repository(
        &self,
        name: String,
    ) -> Result<(String, String), Box<dyn std::error::Error>> {
        log::info!("Creating ECR repository");
        let response = self
            .inner
            .create_repository()
            .repository_name(name)
            .send()
            .await?;

        let repository = response.repository();

        match repository {
            Some(repo) => {
                let registry_id = repo.registry_id().ok_or("Failed to retrieve registry ID")?;
                let repository_uri = repo
                    .repository_uri()
                    .ok_or("Failed to retrieve registry URI")?;

                Ok((registry_id.to_string(), repository_uri.to_string()))
            }
            None => Err("Failed to create ECR repository".into()),
        }
    }

    pub async fn delete_repository(&self, name: String) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Deleting ECR repository");
        self.inner
            .delete_repository()
            .repository_name(name)
            .force(true)
            .send()
            .await?;

        log::info!("Deleted ECR repository");

        Ok(())
    }
}

// TODO: Is there a better way to expose mocked structs?
#[cfg(test)]
pub(super) use MockS3Impl as S3;
#[cfg(not(test))]
pub(super) use S3Impl as S3;

#[cfg(not(test))]
pub use Ec2Impl as Ec2;
#[cfg(test)]
pub use MockEc2Impl as Ec2;

#[cfg(not(test))]
pub use IAMImpl as IAM;
#[cfg(test)]
pub use MockIAMImpl as IAM;

#[cfg(not(test))]
pub use ECRImpl as ECR;
#[cfg(test)]
pub use MockECRImpl as ECR;

#[cfg(test)]
pub use MockRoute53Impl as Route53;
#[cfg(not(test))]
pub use Route53Impl as Route53;
