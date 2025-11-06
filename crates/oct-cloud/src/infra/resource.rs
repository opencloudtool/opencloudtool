use aws_sdk_ec2::types::InstanceStateName;
use base64::{Engine as _, engine::general_purpose};
use serde::{Deserialize, Serialize};

use crate::aws::client;
use crate::aws::types;

/// Defines the main methods to manage resources
pub trait Manager<'a, I, O>
where
    I: 'a + Send + Sync,
    O: 'a + Send + Sync,
{
    fn create(
        &self,
        input: &'a I,
        parents: Vec<&'a Node>,
    ) -> impl std::future::Future<Output = Result<O, Box<dyn std::error::Error>>> + Send;

    fn destroy(
        &self,
        input: &'a O,
        parents: Vec<&'a Node>,
    ) -> impl std::future::Future<Output = Result<(), Box<dyn std::error::Error>>> + Send;
}

#[derive(Debug)]
pub struct HostedZoneSpec {
    pub region: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostedZone {
    pub id: String,
    pub region: String,
    pub name: String,
}

pub struct HostedZoneManager<'a> {
    pub client: &'a client::Route53,
}

impl Manager<'_, HostedZoneSpec, HostedZone> for HostedZoneManager<'_> {
    async fn create(
        &self,
        input: &'_ HostedZoneSpec,
        _parents: Vec<&'_ Node>,
    ) -> Result<HostedZone, Box<dyn std::error::Error>> {
        let hosted_zone_id = self.client.create_hosted_zone(input.name.clone()).await?;

        Ok(HostedZone {
            id: hosted_zone_id,
            region: input.region.clone(),
            name: input.name.clone(),
        })
    }

    async fn destroy(
        &self,
        input: &'_ HostedZone,
        _parents: Vec<&'_ Node>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.client.delete_hosted_zone(input.id.clone()).await
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsRecordSpec {
    pub record_type: types::RecordType,
    pub ttl: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DnsRecord {
    pub name: String,
    pub value: String,
    pub record_type: types::RecordType,
    pub ttl: Option<i64>,
}

pub struct DnsRecordManager<'a> {
    pub client: &'a client::Route53,
}

impl Manager<'_, DnsRecordSpec, DnsRecord> for DnsRecordManager<'_> {
    async fn create(
        &self,
        input: &'_ DnsRecordSpec,
        parents: Vec<&'_ Node>,
    ) -> Result<DnsRecord, Box<dyn std::error::Error>> {
        let hosted_zone_node = parents
            .iter()
            .find(|parent| matches!(parent, Node::Resource(ResourceType::HostedZone(_))));

        let hosted_zone =
            if let Some(Node::Resource(ResourceType::HostedZone(hosted_zone))) = hosted_zone_node {
                Ok(hosted_zone.clone())
            } else {
                Err("DnsRecord expects HostedZone as a parent")
            }?;

        let vm_node = parents
            .iter()
            .find(|parent| matches!(parent, Node::Resource(ResourceType::Vm(_))));

        let vm = if let Some(Node::Resource(ResourceType::Vm(vm))) = vm_node {
            Ok(vm.clone())
        } else {
            Err("DnsRecord expects Vm as a parent")
        }?;

        let domain_name = format!("{}.{}", vm.id, hosted_zone.name);

        self.client
            .create_dns_record(
                hosted_zone.id.clone(),
                domain_name.clone(),
                input.record_type,
                vm.public_ip.clone(),
                input.ttl,
            )
            .await?;

        Ok(DnsRecord {
            record_type: input.record_type,
            name: domain_name.clone(),
            value: vm.public_ip.clone(),
            ttl: input.ttl,
        })
    }

    async fn destroy(
        &self,
        input: &'_ DnsRecord,
        parents: Vec<&'_ Node>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let hosted_zone_node = parents
            .iter()
            .find(|parent| matches!(parent, Node::Resource(ResourceType::HostedZone(_))));

        let hosted_zone =
            if let Some(Node::Resource(ResourceType::HostedZone(hosted_zone))) = hosted_zone_node {
                Ok(hosted_zone.clone())
            } else {
                Err("DnsRecord expects HostedZone as a parent")
            }?;

        self.client
            .delete_dns_record(
                hosted_zone.id.clone(),
                input.name.clone(),
                input.record_type,
                input.value.clone(),
                input.ttl,
            )
            .await
    }
}

#[derive(Debug)]
pub struct VpcSpec {
    pub region: String,
    pub cidr_block: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Vpc {
    pub id: String,
    pub region: String,
    pub cidr_block: String,
    pub name: String,
}

pub struct VpcManager<'a> {
    pub client: &'a client::Ec2,
}

impl Manager<'_, VpcSpec, Vpc> for VpcManager<'_> {
    async fn create(
        &self,
        input: &'_ VpcSpec,
        _parents: Vec<&Node>,
    ) -> Result<Vpc, Box<dyn std::error::Error>> {
        let vpc_id = self
            .client
            .create_vpc(input.cidr_block.clone(), input.name.clone())
            .await?;

        Ok(Vpc {
            id: vpc_id,
            region: input.region.clone(),
            cidr_block: input.cidr_block.clone(),
            name: input.name.clone(),
        })
    }

    async fn destroy(
        &self,
        input: &'_ Vpc,
        _parents: Vec<&Node>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.client.delete_vpc(input.id.clone()).await
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InternetGatewaySpec;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InternetGateway {
    pub id: String,
}

pub struct InternetGatewayManager<'a> {
    pub client: &'a client::Ec2,
}

impl Manager<'_, InternetGatewaySpec, InternetGateway> for InternetGatewayManager<'_> {
    async fn create(
        &self,
        _input: &'_ InternetGatewaySpec,
        parents: Vec<&'_ Node>,
    ) -> Result<InternetGateway, Box<dyn std::error::Error>> {
        let vpc_node = parents
            .iter()
            .find(|parent| matches!(parent, Node::Resource(ResourceType::Vpc(_))));

        let vpc = if let Some(Node::Resource(ResourceType::Vpc(vpc))) = vpc_node {
            Ok(vpc.clone())
        } else {
            Err("Igw expects VPC as a parent")
        }?;

        let igw_id = self.client.create_internet_gateway(vpc.id.clone()).await?;

        Ok(InternetGateway { id: igw_id })
    }

    async fn destroy(
        &self,
        input: &'_ InternetGateway,
        parents: Vec<&Node>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let vpc_node = parents
            .iter()
            .find(|parent| matches!(parent, Node::Resource(ResourceType::Vpc(_))));

        let vpc = if let Some(Node::Resource(ResourceType::Vpc(vpc))) = vpc_node {
            Ok(vpc.clone())
        } else {
            Err("Igw expects VPC as a parent")
        }?;

        self.client
            .delete_internet_gateway(input.id.clone(), vpc.id.clone())
            .await?;

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteTableSpec;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteTable {
    pub id: String,
}

pub struct RouteTableManager<'a> {
    pub client: &'a client::Ec2,
}

impl Manager<'_, RouteTableSpec, RouteTable> for RouteTableManager<'_> {
    async fn create(
        &self,
        _input: &'_ RouteTableSpec,
        parents: Vec<&'_ Node>,
    ) -> Result<RouteTable, Box<dyn std::error::Error>> {
        let vpc_node = parents
            .iter()
            .find(|parent| matches!(parent, Node::Resource(ResourceType::Vpc(_))));

        let vpc = if let Some(Node::Resource(ResourceType::Vpc(vpc))) = vpc_node {
            Ok(vpc.clone())
        } else {
            Err("RouteTable expects VPC as a parent")
        }?;

        let igw_node = parents
            .iter()
            .find(|parent| matches!(parent, Node::Resource(ResourceType::InternetGateway(_))));

        let igw = if let Some(Node::Resource(ResourceType::InternetGateway(igw))) = igw_node {
            Ok(igw.clone())
        } else {
            Err("RouteTable expects IGW as a parent")
        }?;

        let id = self.client.create_route_table(vpc.id.clone()).await?;

        self.client
            .add_public_route(id.clone(), igw.id.clone())
            .await?;

        Ok(RouteTable { id })
    }

    async fn destroy(
        &self,
        input: &'_ RouteTable,
        _parents: Vec<&'_ Node>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.client.delete_route_table(input.id.clone()).await
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubnetSpec {
    pub name: String,
    pub cidr_block: String,
    pub availability_zone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Subnet {
    pub id: String,
    pub name: String,
    pub cidr_block: String,
    pub availability_zone: String,
}

pub struct SubnetManager<'a> {
    pub client: &'a client::Ec2,
}

impl Manager<'_, SubnetSpec, Subnet> for SubnetManager<'_> {
    async fn create(
        &self,
        input: &'_ SubnetSpec,
        parents: Vec<&Node>,
    ) -> Result<Subnet, Box<dyn std::error::Error>> {
        let vpc_node = parents
            .iter()
            .find(|parent| matches!(parent, Node::Resource(ResourceType::Vpc(_))));

        let vpc = if let Some(Node::Resource(ResourceType::Vpc(vpc))) = vpc_node {
            Ok(vpc.clone())
        } else {
            Err("Subnet expects VPC as a parent")
        }?;

        let route_table_node = parents
            .iter()
            .find(|parent| matches!(parent, Node::Resource(ResourceType::RouteTable(_))));

        let route_table =
            if let Some(Node::Resource(ResourceType::RouteTable(route_table))) = route_table_node {
                Ok(route_table.clone())
            } else {
                Err("Subnet expects RouteTable as a parent")
            }?;

        let subnet_id = self
            .client
            .create_subnet(
                vpc.id.clone(),
                input.cidr_block.clone(),
                input.availability_zone.clone(),
                input.name.clone(),
            )
            .await?;

        self.client
            .enable_auto_assign_ip_addresses_for_subnet(subnet_id.clone())
            .await?;

        self.client
            .associate_route_table_with_subnet(route_table.id.clone(), subnet_id.clone())
            .await?;

        Ok(Subnet {
            id: subnet_id,
            name: input.name.clone(),
            cidr_block: input.cidr_block.clone(),
            availability_zone: input.availability_zone.clone(),
        })
    }

    async fn destroy(
        &self,
        input: &'_ Subnet,
        parents: Vec<&Node>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let route_table_node = parents
            .iter()
            .find(|parent| matches!(parent, Node::Resource(ResourceType::RouteTable(_))));

        let route_table =
            if let Some(Node::Resource(ResourceType::RouteTable(route_table))) = route_table_node {
                Ok(route_table.clone())
            } else {
                Err("Subnet expects RouteTable as a parent")
            }?;

        self.client
            .disassociate_route_table_with_subnet(route_table.id.clone(), input.id.clone())
            .await?;

        self.client.delete_subnet(input.id.clone()).await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InboundRule {
    pub protocol: String,
    pub port: i32,
    pub cidr_block: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityGroupSpec {
    pub name: String,
    pub inbound_rules: Vec<InboundRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityGroup {
    pub id: String,
    pub name: String,
    pub inbound_rules: Vec<InboundRule>,
}

pub struct SecurityGroupManager<'a> {
    pub client: &'a client::Ec2,
}

impl Manager<'_, SecurityGroupSpec, SecurityGroup> for SecurityGroupManager<'_> {
    async fn create(
        &self,
        input: &'_ SecurityGroupSpec,
        parents: Vec<&Node>,
    ) -> Result<SecurityGroup, Box<dyn std::error::Error>> {
        let vpc_node = parents
            .iter()
            .find(|parent| matches!(parent, Node::Resource(ResourceType::Vpc(_))));

        let vpc = if let Some(Node::Resource(ResourceType::Vpc(vpc))) = vpc_node {
            Ok(vpc.clone())
        } else {
            Err("SecurityGroup expects VPC as a parent")
        }?;

        let security_group_id = self
            .client
            .create_security_group(
                vpc.id.clone(),
                input.name.clone(),
                String::from("No description"),
            )
            .await?;

        for rule in &input.inbound_rules {
            self.client
                .allow_inbound_traffic_for_security_group(
                    security_group_id.clone(),
                    rule.protocol.clone(),
                    rule.port,
                    rule.cidr_block.clone(),
                )
                .await?;
        }

        Ok(SecurityGroup {
            id: security_group_id.clone(),
            name: input.name.clone(),
            inbound_rules: input.inbound_rules.clone(),
        })
    }

    async fn destroy(
        &self,
        input: &'_ SecurityGroup,
        _parents: Vec<&Node>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.client.delete_security_group(input.id.clone()).await
    }
}

#[derive(Debug)]
pub struct InstanceRoleSpec {
    pub name: String,
    pub assume_role_policy: String,
    pub policy_arns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstanceRole {
    pub name: String,
    pub assume_role_policy: String,
    pub policy_arns: Vec<String>,
}

pub struct InstanceRoleManager<'a> {
    pub client: &'a client::IAM,
}

impl Manager<'_, InstanceRoleSpec, InstanceRole> for InstanceRoleManager<'_> {
    async fn create(
        &self,
        input: &'_ InstanceRoleSpec,
        _parents: Vec<&'_ Node>,
    ) -> Result<InstanceRole, Box<dyn std::error::Error>> {
        let () = self
            .client
            .create_instance_iam_role(
                input.name.clone(),
                input.assume_role_policy.clone(),
                input.policy_arns.clone(),
            )
            .await?;

        Ok(InstanceRole {
            name: input.name.clone(),
            assume_role_policy: input.assume_role_policy.clone(),
            policy_arns: input.policy_arns.clone(),
        })
    }

    async fn destroy(
        &self,
        input: &'_ InstanceRole,
        _parents: Vec<&'_ Node>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.client
            .delete_instance_iam_role(input.name.clone(), input.policy_arns.clone())
            .await
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceProfileSpec {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstanceProfile {
    pub name: String,
}

pub struct InstanceProfileManager<'a> {
    pub client: &'a client::IAM,
}

impl Manager<'_, InstanceProfileSpec, InstanceProfile> for InstanceProfileManager<'_> {
    async fn create(
        &self,
        input: &'_ InstanceProfileSpec,
        parents: Vec<&'_ Node>,
    ) -> Result<InstanceProfile, Box<dyn std::error::Error>> {
        let instance_role_names = parents
            .iter()
            .filter_map(|parent| match parent {
                Node::Resource(ResourceType::InstanceRole(instance_role)) => {
                    Some(instance_role.name.clone())
                }
                _ => None,
            })
            .collect();

        self.client
            .create_instance_profile(input.name.clone(), instance_role_names)
            .await?;

        Ok(InstanceProfile {
            name: input.name.clone(),
        })
    }

    async fn destroy(
        &self,
        input: &'_ InstanceProfile,
        parents: Vec<&'_ Node>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let instance_role_names = parents
            .iter()
            .filter_map(|parent| match parent {
                Node::Resource(ResourceType::InstanceRole(instance_role)) => {
                    Some(instance_role.name.clone())
                }
                _ => None,
            })
            .collect();

        self.client
            .delete_instance_profile(input.name.clone(), instance_role_names)
            .await
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EcrSpec {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Ecr {
    pub id: String,
    pub uri: String,
    pub name: String,
}

impl Ecr {
    pub fn get_base_uri(&self) -> &str {
        let (base_uri, _) = self
            .uri
            .split_once('/')
            .expect("Failed to split `uri` by `/` delimiter");

        base_uri
    }
}

pub struct EcrManager<'a> {
    pub client: &'a client::ECR,
}

impl Manager<'_, EcrSpec, Ecr> for EcrManager<'_> {
    async fn create(
        &self,
        input: &'_ EcrSpec,
        _parents: Vec<&'_ Node>,
    ) -> Result<Ecr, Box<dyn std::error::Error>> {
        let (id, uri) = self.client.create_repository(input.name.clone()).await?;

        Ok(Ecr {
            id,
            uri,
            name: input.name.clone(),
        })
    }

    async fn destroy(
        &self,
        input: &'_ Ecr,
        _parents: Vec<&'_ Node>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.client.delete_repository(input.name.clone()).await
    }
}

#[derive(Debug)]
pub struct VmSpec {
    pub instance_type: types::InstanceType,
    pub ami: String,
    pub user_data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Vm {
    pub id: String,
    pub public_ip: String,
    pub instance_type: types::InstanceType,
    pub ami: String,
    pub user_data: String,
}

pub struct VmManager<'a> {
    pub client: &'a client::Ec2,
}

impl VmManager<'_> {
    /// TODO: Move the full VM initialization logic to client
    async fn get_public_ip(&self, instance_id: &str) -> Option<String> {
        const MAX_ATTEMPTS: usize = 10;
        const SLEEP_DURATION: std::time::Duration = std::time::Duration::from_secs(5);

        for _ in 0..MAX_ATTEMPTS {
            if let Ok(instance) = self
                .client
                .describe_instances(String::from(instance_id))
                .await
            {
                if let Some(public_ip) = instance.public_ip_address() {
                    return Some(public_ip.to_string());
                }
            }

            tokio::time::sleep(SLEEP_DURATION).await;
        }

        None
    }

    async fn is_terminated(&self, id: String) -> Result<(), Box<dyn std::error::Error>> {
        let max_attempts = 24;
        let sleep_duration = 5;

        log::info!("Waiting for VM {id:?} to be terminated...");

        for _ in 0..max_attempts {
            let vm = self.client.describe_instances(id.clone()).await?;

            let vm_status = vm.state().and_then(|s| s.name());

            if vm_status == Some(&InstanceStateName::Terminated) {
                log::info!("VM {id:?} terminated");
                return Ok(());
            }

            log::info!("VM is not terminated yet. Retrying in {sleep_duration} sec...",);

            tokio::time::sleep(std::time::Duration::from_secs(sleep_duration)).await;
        }

        Err("VM failed to terminate".into())
    }
}

impl Manager<'_, VmSpec, Vm> for VmManager<'_> {
    async fn create(
        &self,
        input: &'_ VmSpec,
        parents: Vec<&Node>,
    ) -> Result<Vm, Box<dyn std::error::Error>> {
        let subnet_node = parents
            .iter()
            .find(|parent| matches!(parent, Node::Resource(ResourceType::Subnet(_))));

        let subnet_id = if let Some(Node::Resource(ResourceType::Subnet(subnet))) = subnet_node {
            Ok(subnet.id.clone())
        } else {
            Err("VM expects Subnet as a parent")
        };

        let ecr_node = parents
            .iter()
            .find(|parent| matches!(parent, Node::Resource(ResourceType::Ecr(_))));

        let ecr = if let Some(Node::Resource(ResourceType::Ecr(ecr))) = ecr_node {
            Ok(ecr.clone())
        } else {
            Err("VM expects Ecr as a parent")
        };

        let instance_profile_node = parents
            .iter()
            .find(|parent| matches!(parent, Node::Resource(ResourceType::InstanceProfile(_))));

        let instance_profile_name =
            if let Some(Node::Resource(ResourceType::InstanceProfile(instance_profile))) =
                instance_profile_node
            {
                Ok(instance_profile.name.clone())
            } else {
                Err("VM expects InstanceProfile as a parent")
            };

        let security_group_node = parents
            .iter()
            .find(|parent| matches!(parent, Node::Resource(ResourceType::SecurityGroup(_))));

        let security_group_id =
            if let Some(Node::Resource(ResourceType::SecurityGroup(security_group))) =
                security_group_node
            {
                Ok(security_group.id.clone())
            } else {
                Err("SecurityGroup expects VPC as a parent")
            };

        let ecr_login_string = format!(
            "aws ecr get-login-password --region us-west-2 | podman login --username AWS --password-stdin {}",
            ecr?.get_base_uri()
        );
        let user_data = format!(
            "{}
{}",
            input.user_data, ecr_login_string
        );
        let user_data_base64 = general_purpose::STANDARD.encode(&user_data);

        let response = self
            .client
            .run_instances(
                input.instance_type.clone(),
                input.ami.clone(),
                user_data_base64,
                instance_profile_name?,
                subnet_id?,
                security_group_id?,
            )
            .await?;

        let instance = response
            .instances()
            .first()
            .ok_or("No instances returned")?;

        let instance_id = instance.instance_id.as_ref().ok_or("No instance id")?;

        let public_ip = self
            .get_public_ip(instance_id)
            .await
            .expect("In this implementation we always expect public ip");

        Ok(Vm {
            id: instance_id.clone(),
            public_ip,
            instance_type: input.instance_type.clone(),
            ami: input.ami.clone(),
            user_data,
        })
    }

    async fn destroy(
        &self,
        input: &'_ Vm,
        _parents: Vec<&Node>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.client.terminate_instance(input.id.clone()).await?;

        self.is_terminated(input.id.clone()).await
    }
}

#[derive(Debug)]
pub enum ResourceSpecType {
    HostedZone(HostedZoneSpec),
    DnsRecord(DnsRecordSpec),
    Vpc(VpcSpec),
    InternetGateway(InternetGatewaySpec),
    RouteTable(RouteTableSpec),
    Subnet(SubnetSpec),
    SecurityGroup(SecurityGroupSpec),
    InstanceRole(InstanceRoleSpec),
    InstanceProfile(InstanceProfileSpec),
    Ecr(EcrSpec),
    Vm(VmSpec),
}

#[derive(Debug, Default)]
pub enum SpecNode {
    /// The synthetic root node.
    #[default]
    Root,
    /// A resource spec in the dependency graph.
    Resource(ResourceSpecType),
}

impl std::fmt::Display for SpecNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpecNode::Root => write!(f, "Root"),
            SpecNode::Resource(resource_type) => match resource_type {
                ResourceSpecType::HostedZone(resource) => {
                    write!(f, "spec HostedZone {}", resource.name)
                }
                ResourceSpecType::DnsRecord(_resource) => {
                    write!(f, "spec DnsRecord")
                }
                ResourceSpecType::Vpc(resource) => {
                    write!(f, "spec {}", resource.name)
                }
                ResourceSpecType::InternetGateway(_resource) => {
                    write!(f, "spec IGW")
                }
                ResourceSpecType::RouteTable(_resource) => {
                    write!(f, "spec RouteTable")
                }
                ResourceSpecType::Subnet(resource) => {
                    write!(f, "spec {}", resource.cidr_block)
                }
                ResourceSpecType::SecurityGroup(resource) => {
                    write!(f, "spec SecurityGroup {}", resource.name)
                }
                ResourceSpecType::InstanceRole(resource) => {
                    write!(f, "spec InstanceRole {}", resource.name)
                }
                ResourceSpecType::InstanceProfile(resource) => {
                    write!(f, "spec InstanceProfile {}", resource.name)
                }
                ResourceSpecType::Ecr(resource) => {
                    write!(f, "spec Ecr {}", resource.name)
                }
                ResourceSpecType::Vm(_resource) => {
                    write!(f, "spec VM")
                }
            },
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ResourceType {
    #[default] // TODO: Remove
    None,

    HostedZone(HostedZone),
    DnsRecord(DnsRecord),
    Vpc(Vpc),
    InternetGateway(InternetGateway),
    RouteTable(RouteTable),
    Subnet(Subnet),
    SecurityGroup(SecurityGroup),
    InstanceRole(InstanceRole),
    InstanceProfile(InstanceProfile),
    Ecr(Ecr),
    Vm(Vm),
}

impl ResourceType {
    pub fn name(&self) -> String {
        match self {
            ResourceType::HostedZone(resource) => format!("hosted_zone.{}", resource.id),
            ResourceType::DnsRecord(resource) => format!("dns_record.{}", resource.name),
            ResourceType::Vpc(resource) => format!("vpc.{}", resource.name),
            ResourceType::InternetGateway(resource) => format!("igw.{}", resource.id),
            ResourceType::RouteTable(resource) => format!("route_table.{}", resource.id),
            ResourceType::Subnet(resource) => format!("subnet.{}", resource.name),
            ResourceType::SecurityGroup(resource) => format!("security_group.{}", resource.id),
            ResourceType::InstanceRole(resource) => format!("instance_role.{}", resource.name),
            ResourceType::InstanceProfile(resource) => {
                format!("instance_profile.{}", resource.name)
            }
            ResourceType::Ecr(resource) => format!("ecr.{}", resource.id),
            ResourceType::Vm(resource) => format!("vm.{}", resource.id),
            ResourceType::None => String::from("none"),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum Node {
    /// The synthetic root node.
    #[default]
    Root,
    /// A cloud resource in the dependency graph.
    Resource(ResourceType),
}

impl std::fmt::Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Node::Root => write!(f, "Root"),
            Node::Resource(resource_type) => match resource_type {
                ResourceType::HostedZone(resource) => {
                    write!(f, "cloud HostedZone {}", resource.id)
                }
                ResourceType::DnsRecord(resource) => {
                    write!(f, "cloud DnsRecord {}", resource.name)
                }
                ResourceType::Vpc(resource) => {
                    write!(f, "cloud VPC {}", resource.name)
                }
                ResourceType::InternetGateway(resource) => {
                    write!(f, "cloud IGW {}", resource.id)
                }
                ResourceType::RouteTable(resource) => {
                    write!(f, "cloud RouteTable {}", resource.id)
                }
                ResourceType::Subnet(resource) => {
                    write!(f, "cloud Subnet {}", resource.cidr_block)
                }
                ResourceType::SecurityGroup(resource) => {
                    write!(f, "cloud SecurityGroup {}", resource.id)
                }
                ResourceType::InstanceRole(resource) => {
                    write!(f, "cloud InstanceRole {}", resource.name)
                }
                ResourceType::InstanceProfile(resource) => {
                    write!(f, "cloud InstanceProfile {}", resource.name)
                }
                ResourceType::Ecr(resource) => {
                    write!(f, "cloud Ecr {}", resource.id)
                }
                ResourceType::Vm(resource) => {
                    write!(f, "cloud VM {}", resource.id)
                }
                ResourceType::None => {
                    write!(f, "cloud None")
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use mockall::predicate::eq;

    #[tokio::test]
    async fn test_vpc_manager_create() {
        // Arrange
        let mut ec2_client_mock = client::Ec2::default();
        ec2_client_mock
            .expect_create_vpc()
            .with(eq(String::from("0.0.0.0/0")), eq(String::from("vpc")))
            .return_once(|_, _| Ok(String::from("vpc-id")));

        let vpc_manager = VpcManager {
            client: &ec2_client_mock,
        };

        let vpc_spec = VpcSpec {
            region: String::from("us-west-2"),
            cidr_block: String::from("0.0.0.0/0"),
            name: String::from("vpc"),
        };

        // Act
        let vpc = vpc_manager.create(&vpc_spec, vec![]).await;

        // Assert
        assert!(vpc.is_ok());
        assert_eq!(
            vpc.expect("Failed to get VPC"),
            Vpc {
                id: String::from("vpc-id"),
                region: String::from("us-west-2"),
                cidr_block: String::from("0.0.0.0/0"),
                name: String::from("vpc"),
            }
        );
    }

    #[tokio::test]
    async fn test_vpc_manager_create_error() {
        // Arrange
        let mut ec2_client_mock = client::Ec2::default();
        ec2_client_mock
            .expect_create_vpc()
            .with(eq(String::new()), eq(String::new()))
            .return_once(|_, _| Err("Error".into()));

        let vpc_manager = VpcManager {
            client: &ec2_client_mock,
        };

        let vpc_spec = VpcSpec {
            region: String::new(),
            cidr_block: String::new(),
            name: String::new(),
        };

        // Act
        let vpc = vpc_manager.create(&vpc_spec, vec![]).await;

        // Assert
        assert!(vpc.is_err());
    }

    #[tokio::test]
    async fn test_vpc_manager_destroy() {
        // Arrange
        let mut ec2_client_mock = client::Ec2::default();
        ec2_client_mock
            .expect_delete_vpc()
            .with(eq(String::from("vpc-id")))
            .return_once(|_| Ok(()));

        let vpc_manager = VpcManager {
            client: &ec2_client_mock,
        };

        let vpc = Vpc {
            id: String::from("vpc-id"),
            region: String::from("us-west-2"),
            cidr_block: String::from("0.0.0.0/0"),
            name: String::from("vpc"),
        };

        // Act
        let result = vpc_manager.destroy(&vpc, vec![]).await;

        // Assert
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_vpc_manager_destroy_error() {
        // Arrange
        let mut ec2_client_mock = client::Ec2::default();
        ec2_client_mock
            .expect_delete_vpc()
            .with(eq(String::from("vpc-id")))
            .return_once(|_| Err("Error".into()));

        let vpc_manager = VpcManager {
            client: &ec2_client_mock,
        };

        let vpc = Vpc {
            id: String::from("vpc-id"),
            region: String::from("us-west-2"),
            cidr_block: String::from("0.0.0.0/0"),
            name: String::from("vpc"),
        };

        // Act
        let vpc = vpc_manager.destroy(&vpc, vec![]).await;

        // Assert
        assert!(vpc.is_err());
    }

    #[tokio::test]
    async fn test_hosted_zone_manager_create() {
        // Arrange
        let mut route53_client_mock = client::Route53::default();
        route53_client_mock
            .expect_create_hosted_zone()
            .with(eq(String::from("example.com")))
            .return_once(|_| Ok(String::from("hosted-zone-id")));

        let hosted_zone_manager = HostedZoneManager {
            client: &route53_client_mock,
        };

        let hosted_zone_spec = HostedZoneSpec {
            region: String::from("us-west-2"),
            name: String::from("example.com"),
        };

        // Act
        let hosted_zone = hosted_zone_manager.create(&hosted_zone_spec, vec![]).await;

        // Assert
        assert!(hosted_zone.is_ok());
        assert_eq!(
            hosted_zone.expect("Failed to get HostedZone"),
            HostedZone {
                id: String::from("hosted-zone-id"),
                region: String::from("us-west-2"),
                name: String::from("example.com"),
            }
        );
    }

    #[tokio::test]
    async fn test_hosted_zone_manager_create_error() {
        // Arrange
        let mut route53_client_mock = client::Route53::default();
        route53_client_mock
            .expect_create_hosted_zone()
            .with(eq(String::from("example.com")))
            .return_once(|_| Err("Error".into()));

        let hosted_zone_manager = HostedZoneManager {
            client: &route53_client_mock,
        };

        let hosted_zone_spec = HostedZoneSpec {
            region: String::from("us-west-2"),
            name: String::from("example.com"),
        };

        // Act
        let hosted_zone = hosted_zone_manager.create(&hosted_zone_spec, vec![]).await;

        // Assert
        assert!(hosted_zone.is_err());
    }

    #[tokio::test]
    async fn test_hosted_zone_manager_destroy() {
        // Arrange
        let mut route53_client_mock = client::Route53::default();
        route53_client_mock
            .expect_delete_hosted_zone()
            .with(eq(String::from("hosted-zone-id")))
            .return_once(|_| Ok(()));

        let hosted_zone_manager = HostedZoneManager {
            client: &route53_client_mock,
        };

        let hosted_zone = HostedZone {
            id: String::from("hosted-zone-id"),
            region: String::from("us-west-2"),
            name: String::from("example.com"),
        };

        // Act
        let result = hosted_zone_manager.destroy(&hosted_zone, vec![]).await;

        // Assert
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_hosted_zone_manager_destroy_error() {
        // Arrange
        let mut route53_client_mock = client::Route53::default();
        route53_client_mock
            .expect_delete_hosted_zone()
            .with(eq(String::from("hosted-zone-id")))
            .return_once(|_| Err("Error".into()));

        let hosted_zone_manager = HostedZoneManager {
            client: &route53_client_mock,
        };

        let hosted_zone = HostedZone {
            id: String::from("hosted-zone-id"),
            region: String::from("us-west-2"),
            name: String::from("example.com"),
        };

        // Act
        let result = hosted_zone_manager.destroy(&hosted_zone, vec![]).await;

        // Assert
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_instance_role_manager_create() {
        // Arrange
        let mut iam_client_mock = client::IAM::default();
        iam_client_mock
            .expect_create_instance_iam_role()
            .with(
                eq(String::from("role-name")),
                eq(String::from("assume-policy")),
                eq(vec![String::from(
                    "arn:aws:iam::aws:policy/AmazonS3ReadOnlyAccess",
                )]),
            )
            .return_once(|_, _, _| Ok(()));

        let instance_role_manager = InstanceRoleManager {
            client: &iam_client_mock,
        };

        let instance_role_spec = InstanceRoleSpec {
            name: String::from("role-name"),
            assume_role_policy: String::from("assume-policy"),
            policy_arns: vec![String::from(
                "arn:aws:iam::aws:policy/AmazonS3ReadOnlyAccess",
            )],
        };

        // Act
        let instance_role = instance_role_manager
            .create(&instance_role_spec, vec![])
            .await;

        // Assert
        assert!(instance_role.is_ok());
        assert_eq!(
            instance_role.expect("Failed to get InstanceRole"),
            InstanceRole {
                name: String::from("role-name"),
                assume_role_policy: String::from("assume-policy"),
                policy_arns: vec![String::from(
                    "arn:aws:iam::aws:policy/AmazonS3ReadOnlyAccess",
                )],
            }
        );
    }

    #[tokio::test]
    async fn test_instance_role_manager_create_error() {
        // Arrange
        let mut iam_client_mock = client::IAM::default();
        iam_client_mock
            .expect_create_instance_iam_role()
            .return_once(|_, _, _| Err("Error".into()));

        let instance_role_manager = InstanceRoleManager {
            client: &iam_client_mock,
        };

        let instance_role_spec = InstanceRoleSpec {
            name: String::from("role-name"),
            assume_role_policy: String::from("assume-policy"),
            policy_arns: vec![String::from(
                "arn:aws:iam::aws:policy/AmazonS3ReadOnlyAccess",
            )],
        };

        // Act
        let instance_role = instance_role_manager
            .create(&instance_role_spec, vec![])
            .await;

        // Assert
        assert!(instance_role.is_err());
    }

    #[tokio::test]
    async fn test_instance_role_manager_destroy() {
        // Arrange
        let mut iam_client_mock = client::IAM::default();
        iam_client_mock
            .expect_delete_instance_iam_role()
            .with(
                eq(String::from("role-name")),
                eq(vec![String::from(
                    "arn:aws:iam::aws:policy/AmazonS3ReadOnlyAccess",
                )]),
            )
            .return_once(|_, _| Ok(()));

        let instance_role_manager = InstanceRoleManager {
            client: &iam_client_mock,
        };

        let instance_role = InstanceRole {
            name: String::from("role-name"),
            assume_role_policy: String::from("assume-policy"),
            policy_arns: vec![String::from(
                "arn:aws:iam::aws:policy/AmazonS3ReadOnlyAccess",
            )],
        };

        // Act
        let result = instance_role_manager.destroy(&instance_role, vec![]).await;

        // Assert
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_instance_role_manager_destroy_error() {
        // Arrange
        let mut iam_client_mock = client::IAM::default();
        iam_client_mock
            .expect_delete_instance_iam_role()
            .return_once(|_, _| Err("Error".into()));

        let instance_role_manager = InstanceRoleManager {
            client: &iam_client_mock,
        };

        let instance_role = InstanceRole {
            name: String::from("role-name"),
            assume_role_policy: String::from("assume-policy"),
            policy_arns: vec![String::from(
                "arn:aws:iam::aws:policy/AmazonS3ReadOnlyAccess",
            )],
        };

        // Act
        let result = instance_role_manager.destroy(&instance_role, vec![]).await;

        // Assert
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_ecr_manager_create() {
        // Arrange
        let mut ecr_client_mock = client::ECR::default();
        ecr_client_mock
            .expect_create_repository()
            .with(eq(String::from("repo-name")))
            .return_once(|_| Ok((String::from("repo-id"), String::from("repo-uri"))));

        let ecr_manager = EcrManager {
            client: &ecr_client_mock,
        };

        let ecr_spec = EcrSpec {
            name: String::from("repo-name"),
        };

        // Act
        let ecr = ecr_manager.create(&ecr_spec, vec![]).await;

        // Assert
        assert!(ecr.is_ok());
        assert_eq!(
            ecr.expect("Failed to create ECR"),
            Ecr {
                id: String::from("repo-id"),
                uri: String::from("repo-uri"),
                name: String::from("repo-name"),
            }
        );
    }

    #[tokio::test]
    async fn test_ecr_manager_create_error() {
        // Arrange
        let mut ecr_client_mock = client::ECR::default();
        ecr_client_mock
            .expect_create_repository()
            .with(eq(String::from("repo-name")))
            .return_once(|_| Err("Error".into()));

        let ecr_manager = EcrManager {
            client: &ecr_client_mock,
        };

        let ecr_spec = EcrSpec {
            name: String::from("repo-name"),
        };

        // Act
        let ecr = ecr_manager.create(&ecr_spec, vec![]).await;

        // Assert
        assert!(ecr.is_err());
    }

    #[tokio::test]
    async fn test_ecr_manager_destroy() {
        // Arrange
        let mut ecr_client_mock = client::ECR::default();
        ecr_client_mock
            .expect_delete_repository()
            .with(eq(String::from("repo-name")))
            .return_once(|_| Ok(()));

        let ecr_manager = EcrManager {
            client: &ecr_client_mock,
        };

        let ecr = Ecr {
            id: String::from("repo-id"),
            uri: String::from("repo-uri"),
            name: String::from("repo-name"),
        };

        // Act
        let result = ecr_manager.destroy(&ecr, vec![]).await;

        // Assert
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_ecr_manager_destroy_error() {
        // Arrange
        let mut ecr_client_mock = client::ECR::default();
        ecr_client_mock
            .expect_delete_repository()
            .with(eq(String::from("repo-name")))
            .return_once(|_| Err("Error".into()));

        let ecr_manager = EcrManager {
            client: &ecr_client_mock,
        };

        let ecr = Ecr {
            id: String::from("repo-id"),
            uri: String::from("repo-uri"),
            name: String::from("repo-name"),
        };

        // Act
        let result = ecr_manager.destroy(&ecr, vec![]).await;

        // Assert
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_internet_gateway_manager_create() {
        // Arrange
        let mut ec2_client_mock = client::Ec2::default();
        ec2_client_mock
            .expect_create_internet_gateway()
            .with(eq(String::from("vpc-id")))
            .return_once(|_| Ok(String::from("igw-id")));

        let igw_manager = InternetGatewayManager {
            client: &ec2_client_mock,
        };

        let igw_spec = InternetGatewaySpec;
        let vpc = Vpc {
            id: String::from("vpc-id"),
            region: String::from("us-west-2"),
            cidr_block: String::from("0.0.0.0/0"),
            name: String::from("vpc"),
        };
        let parents = [Node::Resource(ResourceType::Vpc(vpc))];

        // Act
        let igw = igw_manager
            .create(&igw_spec, parents.iter().collect())
            .await;

        // Assert
        assert!(igw.is_ok());
        assert_eq!(
            igw.expect("Failed to create InternetGateway"),
            InternetGateway {
                id: String::from("igw-id"),
            }
        );
    }

    #[tokio::test]
    async fn test_internet_gateway_manager_create_no_vpc_parent() {
        // Arrange
        let ec2_client_mock = client::Ec2::default();
        let igw_manager = InternetGatewayManager {
            client: &ec2_client_mock,
        };
        let igw_spec = InternetGatewaySpec;

        // Act
        let igw = igw_manager.create(&igw_spec, vec![]).await;

        // Assert
        assert!(igw.is_err());
        assert_eq!(
            igw.expect_err("Expected error").to_string(),
            "Igw expects VPC as a parent"
        );
    }

    #[tokio::test]
    async fn test_internet_gateway_manager_create_error() {
        // Arrange
        let mut ec2_client_mock = client::Ec2::default();
        ec2_client_mock
            .expect_create_internet_gateway()
            .return_once(|_| Err("Error".into()));

        let igw_manager = InternetGatewayManager {
            client: &ec2_client_mock,
        };

        let igw_spec = InternetGatewaySpec;
        let vpc = Vpc {
            id: String::from("vpc-id"),
            region: String::from("us-west-2"),
            cidr_block: String::from("0.0.0.0/0"),
            name: String::from("vpc"),
        };
        let parents = [Node::Resource(ResourceType::Vpc(vpc))];

        // Act
        let igw = igw_manager
            .create(&igw_spec, parents.iter().collect())
            .await;

        // Assert
        assert!(igw.is_err());
    }

    #[tokio::test]
    async fn test_internet_gateway_manager_destroy() {
        // Arrange
        let mut ec2_client_mock = client::Ec2::default();
        ec2_client_mock
            .expect_delete_internet_gateway()
            .with(eq(String::from("igw-id")), eq(String::from("vpc-id")))
            .return_once(|_, _| Ok(()));

        let igw_manager = InternetGatewayManager {
            client: &ec2_client_mock,
        };

        let igw = InternetGateway {
            id: String::from("igw-id"),
        };
        let vpc = Vpc {
            id: String::from("vpc-id"),
            region: String::from("us-west-2"),
            cidr_block: String::from("0.0.0.0/0"),
            name: String::from("vpc"),
        };
        let parents = [Node::Resource(ResourceType::Vpc(vpc))];

        // Act
        let result = igw_manager.destroy(&igw, parents.iter().collect()).await;

        // Assert
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_internet_gateway_manager_destroy_no_vpc_parent() {
        // Arrange
        let ec2_client_mock = client::Ec2::default();
        let igw_manager = InternetGatewayManager {
            client: &ec2_client_mock,
        };
        let igw = InternetGateway {
            id: String::from("igw-id"),
        };

        // Act
        let result = igw_manager.destroy(&igw, vec![]).await;

        // Assert
        assert!(result.is_err());
        assert_eq!(
            result.expect_err("Expected error").to_string(),
            "Igw expects VPC as a parent"
        );
    }

    #[tokio::test]
    async fn test_internet_gateway_manager_destroy_error() {
        // Arrange
        let mut ec2_client_mock = client::Ec2::default();
        ec2_client_mock
            .expect_delete_internet_gateway()
            .return_once(|_, _| Err("Error".into()));

        let igw_manager = InternetGatewayManager {
            client: &ec2_client_mock,
        };

        let igw = InternetGateway {
            id: String::from("igw-id"),
        };
        let vpc = Vpc {
            id: String::from("vpc-id"),
            region: String::from("us-west-2"),
            cidr_block: String::from("0.0.0.0/0"),
            name: String::from("vpc"),
        };
        let parents = [Node::Resource(ResourceType::Vpc(vpc))];

        // Act
        let result = igw_manager.destroy(&igw, parents.iter().collect()).await;

        // Assert
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_subnet_manager_create() {
        // Arrange
        let mut ec2_client_mock = client::Ec2::default();
        ec2_client_mock
            .expect_create_subnet()
            .with(
                eq(String::from("vpc-id")),
                eq(String::from("10.0.1.0/24")),
                eq(String::from("us-west-2a")),
                eq(String::from("subnet-name")),
            )
            .return_once(|_, _, _, _| Ok(String::from("subnet-id")));
        ec2_client_mock
            .expect_enable_auto_assign_ip_addresses_for_subnet()
            .with(eq(String::from("subnet-id")))
            .return_once(|_| Ok(()));
        ec2_client_mock
            .expect_associate_route_table_with_subnet()
            .with(eq(String::from("rt-id")), eq(String::from("subnet-id")))
            .return_once(|_, _| Ok(()));

        let subnet_manager = SubnetManager {
            client: &ec2_client_mock,
        };

        let subnet_spec = SubnetSpec {
            name: String::from("subnet-name"),
            cidr_block: String::from("10.0.1.0/24"),
            availability_zone: String::from("us-west-2a"),
        };
        let vpc = Vpc {
            id: String::from("vpc-id"),
            region: String::from("us-west-2"),
            cidr_block: String::from("10.0.0.0/16"),
            name: String::from("vpc-name"),
        };
        let route_table = RouteTable {
            id: String::from("rt-id"),
        };
        let parents = [
            Node::Resource(ResourceType::Vpc(vpc)),
            Node::Resource(ResourceType::RouteTable(route_table)),
        ];

        // Act
        let subnet = subnet_manager
            .create(&subnet_spec, parents.iter().collect())
            .await;

        // Assert
        assert!(subnet.is_ok());
        assert_eq!(
            subnet.expect("Failed to create subnet"),
            Subnet {
                id: String::from("subnet-id"),
                name: String::from("subnet-name"),
                cidr_block: String::from("10.0.1.0/24"),
                availability_zone: String::from("us-west-2a"),
            }
        );
    }

    #[tokio::test]
    async fn test_subnet_manager_create_no_vpc_parent() {
        // Arrange
        let ec2_client_mock = client::Ec2::default();
        let subnet_manager = SubnetManager {
            client: &ec2_client_mock,
        };
        let subnet_spec = SubnetSpec {
            name: String::from("subnet-name"),
            cidr_block: String::from("10.0.1.0/24"),
            availability_zone: String::from("us-west-2a"),
        };
        let route_table = RouteTable {
            id: String::from("rt-id"),
        };
        let parents = [Node::Resource(ResourceType::RouteTable(route_table))];

        // Act
        let result = subnet_manager
            .create(&subnet_spec, parents.iter().collect())
            .await;

        // Assert
        assert!(result.is_err());
        assert_eq!(
            result.expect_err("Expected error").to_string(),
            "Subnet expects VPC as a parent"
        );
    }

    #[tokio::test]
    async fn test_subnet_manager_create_no_route_table_parent() {
        // Arrange
        let ec2_client_mock = client::Ec2::default();
        let subnet_manager = SubnetManager {
            client: &ec2_client_mock,
        };
        let subnet_spec = SubnetSpec {
            name: String::from("subnet-name"),
            cidr_block: String::from("10.0.1.0/24"),
            availability_zone: String::from("us-west-2a"),
        };
        let vpc = Vpc {
            id: String::from("vpc-id"),
            region: String::from("us-west-2"),
            cidr_block: String::from("10.0.0.0/16"),
            name: String::from("vpc-name"),
        };
        let parents = [Node::Resource(ResourceType::Vpc(vpc))];

        // Act
        let result = subnet_manager
            .create(&subnet_spec, parents.iter().collect())
            .await;

        // Assert
        assert!(result.is_err());
        assert_eq!(
            result.expect_err("Expected error").to_string(),
            "Subnet expects RouteTable as a parent"
        );
    }

    #[tokio::test]
    async fn test_subnet_manager_create_error() {
        // Arrange
        let mut ec2_client_mock = client::Ec2::default();
        ec2_client_mock
            .expect_create_subnet()
            .return_once(|_, _, _, _| Err("Error".into()));

        let subnet_manager = SubnetManager {
            client: &ec2_client_mock,
        };

        let subnet_spec = SubnetSpec {
            name: String::from("subnet-name"),
            cidr_block: String::from("10.0.1.0/24"),
            availability_zone: String::from("us-west-2a"),
        };
        let vpc = Vpc {
            id: String::from("vpc-id"),
            region: String::from("us-west-2"),
            cidr_block: String::from("10.0.0.0/16"),
            name: String::from("vpc-name"),
        };
        let route_table = RouteTable {
            id: String::from("rt-id"),
        };
        let parents = [
            Node::Resource(ResourceType::Vpc(vpc)),
            Node::Resource(ResourceType::RouteTable(route_table)),
        ];

        // Act
        let subnet = subnet_manager
            .create(&subnet_spec, parents.iter().collect())
            .await;

        // Assert
        assert!(subnet.is_err());
    }

    #[tokio::test]
    async fn test_subnet_manager_destroy() {
        // Arrange
        let mut ec2_client_mock = client::Ec2::default();
        ec2_client_mock
            .expect_disassociate_route_table_with_subnet()
            .with(eq(String::from("rt-id")), eq(String::from("subnet-id")))
            .return_once(|_, _| Ok(()));
        ec2_client_mock
            .expect_delete_subnet()
            .with(eq(String::from("subnet-id")))
            .return_once(|_| Ok(()));

        let subnet_manager = SubnetManager {
            client: &ec2_client_mock,
        };

        let subnet = Subnet {
            id: String::from("subnet-id"),
            name: String::from("subnet-name"),
            cidr_block: String::from("10.0.1.0/24"),
            availability_zone: String::from("us-west-2a"),
        };
        let route_table = RouteTable {
            id: String::from("rt-id"),
        };
        let parents = [Node::Resource(ResourceType::RouteTable(route_table))];

        // Act
        let result = subnet_manager
            .destroy(&subnet, parents.iter().collect())
            .await;

        // Assert
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_subnet_manager_destroy_no_route_table_parent() {
        // Arrange
        let ec2_client_mock = client::Ec2::default();
        let subnet_manager = SubnetManager {
            client: &ec2_client_mock,
        };
        let subnet = Subnet {
            id: String::from("subnet-id"),
            name: String::from("subnet-name"),
            cidr_block: String::from("10.0.1.0/24"),
            availability_zone: String::from("us-west-2a"),
        };

        // Act
        let result = subnet_manager.destroy(&subnet, vec![]).await;

        // Assert
        assert!(result.is_err());
        assert_eq!(
            result.expect_err("Expected error").to_string(),
            "Subnet expects RouteTable as a parent"
        );
    }

    #[tokio::test]
    async fn test_subnet_manager_destroy_error() {
        // Arrange
        let mut ec2_client_mock = client::Ec2::default();
        ec2_client_mock
            .expect_disassociate_route_table_with_subnet()
            .return_once(|_, _| Err("Error".into()));

        let subnet_manager = SubnetManager {
            client: &ec2_client_mock,
        };

        let subnet = Subnet {
            id: String::from("subnet-id"),
            name: String::from("subnet-name"),
            cidr_block: String::from("10.0.1.0/24"),
            availability_zone: String::from("us-west-2a"),
        };
        let route_table = RouteTable {
            id: String::from("rt-id"),
        };
        let parents = [Node::Resource(ResourceType::RouteTable(route_table))];

        // Act
        let result = subnet_manager
            .destroy(&subnet, parents.iter().collect())
            .await;

        // Assert
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_route_table_manager_create() {
        // Arrange
        let mut ec2_client_mock = client::Ec2::default();
        ec2_client_mock
            .expect_create_route_table()
            .with(eq(String::from("vpc-id")))
            .return_once(|_| Ok(String::from("rt-id")));
        ec2_client_mock
            .expect_add_public_route()
            .with(eq(String::from("rt-id")), eq(String::from("igw-id")))
            .return_once(|_, _| Ok(()));

        let route_table_manager = RouteTableManager {
            client: &ec2_client_mock,
        };

        let route_table_spec = RouteTableSpec;
        let vpc = Vpc {
            id: String::from("vpc-id"),
            region: String::from("us-west-2"),
            cidr_block: String::from("10.0.0.0/16"),
            name: String::from("vpc-name"),
        };
        let igw = InternetGateway {
            id: String::from("igw-id"),
        };
        let parents = [
            Node::Resource(ResourceType::Vpc(vpc)),
            Node::Resource(ResourceType::InternetGateway(igw)),
        ];

        // Act
        let route_table = route_table_manager
            .create(&route_table_spec, parents.iter().collect())
            .await;

        // Assert
        assert!(route_table.is_ok());
        assert_eq!(
            route_table.expect("Failed to create route table"),
            RouteTable {
                id: String::from("rt-id"),
            }
        );
    }

    #[tokio::test]
    async fn test_route_table_manager_create_no_vpc_parent() {
        // Arrange
        let ec2_client_mock = client::Ec2::default();
        let route_table_manager = RouteTableManager {
            client: &ec2_client_mock,
        };
        let route_table_spec = RouteTableSpec;
        let igw = InternetGateway {
            id: String::from("igw-id"),
        };
        let parents = [Node::Resource(ResourceType::InternetGateway(igw))];

        // Act
        let result = route_table_manager
            .create(&route_table_spec, parents.iter().collect())
            .await;

        // Assert
        assert!(result.is_err());
        assert_eq!(
            result.expect_err("Expected error").to_string(),
            "RouteTable expects VPC as a parent"
        );
    }

    #[tokio::test]
    async fn test_route_table_manager_create_no_igw_parent() {
        // Arrange
        let ec2_client_mock = client::Ec2::default();
        let route_table_manager = RouteTableManager {
            client: &ec2_client_mock,
        };
        let route_table_spec = RouteTableSpec;
        let vpc = Vpc {
            id: String::from("vpc-id"),
            region: String::from("us-west-2"),
            cidr_block: String::from("10.0.0.0/16"),
            name: String::from("vpc-name"),
        };
        let parents = [Node::Resource(ResourceType::Vpc(vpc))];

        // Act
        let result = route_table_manager
            .create(&route_table_spec, parents.iter().collect())
            .await;

        // Assert
        assert!(result.is_err());
        assert_eq!(
            result.expect_err("Expected error").to_string(),
            "RouteTable expects IGW as a parent"
        );
    }

    #[tokio::test]
    async fn test_route_table_manager_create_error() {
        // Arrange
        let mut ec2_client_mock = client::Ec2::default();
        ec2_client_mock
            .expect_create_route_table()
            .return_once(|_| Err("Error".into()));

        let route_table_manager = RouteTableManager {
            client: &ec2_client_mock,
        };

        let route_table_spec = RouteTableSpec;
        let vpc = Vpc {
            id: String::from("vpc-id"),
            region: String::from("us-west-2"),
            cidr_block: String::from("10.0.0.0/16"),
            name: String::from("vpc-name"),
        };
        let igw = InternetGateway {
            id: String::from("igw-id"),
        };
        let parents = [
            Node::Resource(ResourceType::Vpc(vpc)),
            Node::Resource(ResourceType::InternetGateway(igw)),
        ];

        // Act
        let route_table = route_table_manager
            .create(&route_table_spec, parents.iter().collect())
            .await;

        // Assert
        assert!(route_table.is_err());
    }

    #[tokio::test]
    async fn test_route_table_manager_destroy() {
        // Arrange
        let mut ec2_client_mock = client::Ec2::default();
        ec2_client_mock
            .expect_delete_route_table()
            .with(eq(String::from("rt-id")))
            .return_once(|_| Ok(()));

        let route_table_manager = RouteTableManager {
            client: &ec2_client_mock,
        };

        let route_table = RouteTable {
            id: String::from("rt-id"),
        };

        // Act
        let result = route_table_manager.destroy(&route_table, vec![]).await;

        // Assert
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_route_table_manager_destroy_error() {
        // Arrange
        let mut ec2_client_mock = client::Ec2::default();
        ec2_client_mock
            .expect_delete_route_table()
            .return_once(|_| Err("Error".into()));

        let route_table_manager = RouteTableManager {
            client: &ec2_client_mock,
        };

        let route_table = RouteTable {
            id: String::from("rt-id"),
        };

        // Act
        let result = route_table_manager.destroy(&route_table, vec![]).await;

        // Assert
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_dns_record_manager_create() {
        // Arrange
        let mut route53_client_mock = client::Route53::default();
        route53_client_mock
            .expect_create_dns_record()
            .with(
                eq(String::from("hz-id")),
                eq(String::from("vm-id.example.com")),
                eq(types::RecordType::A),
                eq(String::from("1.2.3.4")),
                eq(Some(300)),
            )
            .return_once(|_, _, _, _, _| Ok(()));

        let dns_record_manager = DnsRecordManager {
            client: &route53_client_mock,
        };

        let dns_record_spec = DnsRecordSpec {
            record_type: types::RecordType::A,
            ttl: Some(300),
        };
        let hosted_zone = HostedZone {
            id: String::from("hz-id"),
            name: String::from("example.com"),
            region: String::from("us-west-2"),
        };
        let vm = Vm {
            id: String::from("vm-id"),
            public_ip: String::from("1.2.3.4"),
            instance_type: types::InstanceType::T2Micro,
            ami: String::from("ami-123"),
            user_data: String::new(),
        };
        let parents = [
            Node::Resource(ResourceType::HostedZone(hosted_zone)),
            Node::Resource(ResourceType::Vm(vm)),
        ];

        // Act
        let dns_record = dns_record_manager
            .create(&dns_record_spec, parents.iter().collect())
            .await;

        // Assert
        assert!(dns_record.is_ok());
        assert_eq!(
            dns_record.expect("Failed to create dns record"),
            DnsRecord {
                name: String::from("vm-id.example.com"),
                value: String::from("1.2.3.4"),
                record_type: types::RecordType::A,
                ttl: Some(300),
            }
        );
    }

    #[tokio::test]
    async fn test_dns_record_manager_create_no_hosted_zone_parent() {
        // Arrange
        let route53_client_mock = client::Route53::default();
        let dns_record_manager = DnsRecordManager {
            client: &route53_client_mock,
        };
        let dns_record_spec = DnsRecordSpec {
            record_type: types::RecordType::A,
            ttl: Some(300),
        };
        let vm = Vm {
            id: String::from("vm-id"),
            public_ip: String::from("1.2.3.4"),
            instance_type: types::InstanceType::T2Micro,
            ami: String::from("ami-123"),
            user_data: String::new(),
        };
        let parents = [Node::Resource(ResourceType::Vm(vm))];

        // Act
        let result = dns_record_manager
            .create(&dns_record_spec, parents.iter().collect())
            .await;

        // Assert
        assert!(result.is_err());
        assert_eq!(
            result.expect_err("Expected error").to_string(),
            "DnsRecord expects HostedZone as a parent"
        );
    }

    #[tokio::test]
    async fn test_dns_record_manager_create_no_vm_parent() {
        // Arrange
        let route53_client_mock = client::Route53::default();
        let dns_record_manager = DnsRecordManager {
            client: &route53_client_mock,
        };
        let dns_record_spec = DnsRecordSpec {
            record_type: types::RecordType::A,
            ttl: Some(300),
        };
        let hosted_zone = HostedZone {
            id: String::from("hz-id"),
            name: String::from("example.com"),
            region: String::from("us-west-2"),
        };
        let parents = [Node::Resource(ResourceType::HostedZone(hosted_zone))];

        // Act
        let result = dns_record_manager
            .create(&dns_record_spec, parents.iter().collect())
            .await;

        // Assert
        assert!(result.is_err());
        assert_eq!(
            result.expect_err("Expected error").to_string(),
            "DnsRecord expects Vm as a parent"
        );
    }

    #[tokio::test]
    async fn test_dns_record_manager_create_error() {
        // Arrange
        let mut route53_client_mock = client::Route53::default();
        route53_client_mock
            .expect_create_dns_record()
            .return_once(|_, _, _, _, _| Err("Error".into()));

        let dns_record_manager = DnsRecordManager {
            client: &route53_client_mock,
        };

        let dns_record_spec = DnsRecordSpec {
            record_type: types::RecordType::A,
            ttl: Some(300),
        };
        let hosted_zone = HostedZone {
            id: String::from("hz-id"),
            name: String::from("example.com"),
            region: String::from("us-west-2"),
        };
        let vm = Vm {
            id: String::from("vm-id"),
            public_ip: String::from("1.2.3.4"),
            instance_type: types::InstanceType::T2Micro,
            ami: String::from("ami-123"),
            user_data: String::new(),
        };
        let parents = [
            Node::Resource(ResourceType::HostedZone(hosted_zone)),
            Node::Resource(ResourceType::Vm(vm)),
        ];

        // Act
        let result = dns_record_manager
            .create(&dns_record_spec, parents.iter().collect())
            .await;

        // Assert
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_dns_record_manager_destroy() {
        // Arrange
        let mut route53_client_mock = client::Route53::default();
        route53_client_mock
            .expect_delete_dns_record()
            .with(
                eq(String::from("hz-id")),
                eq(String::from("vm-id.example.com")),
                eq(types::RecordType::A),
                eq(String::from("1.2.3.4")),
                eq(Some(300)),
            )
            .return_once(|_, _, _, _, _| Ok(()));

        let dns_record_manager = DnsRecordManager {
            client: &route53_client_mock,
        };

        let dns_record = DnsRecord {
            name: String::from("vm-id.example.com"),
            value: String::from("1.2.3.4"),
            record_type: types::RecordType::A,
            ttl: Some(300),
        };
        let hosted_zone = HostedZone {
            id: String::from("hz-id"),
            name: String::from("example.com"),
            region: String::from("us-west-2"),
        };
        let parents = [Node::Resource(ResourceType::HostedZone(hosted_zone))];

        // Act
        let result = dns_record_manager
            .destroy(&dns_record, parents.iter().collect())
            .await;

        // Assert
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_dns_record_manager_destroy_no_hosted_zone_parent() {
        // Arrange
        let route53_client_mock = client::Route53::default();
        let dns_record_manager = DnsRecordManager {
            client: &route53_client_mock,
        };
        let dns_record = DnsRecord {
            name: String::from("vm-id.example.com"),
            value: String::from("1.2.3.4"),
            record_type: types::RecordType::A,
            ttl: Some(300),
        };

        // Act
        let result = dns_record_manager.destroy(&dns_record, vec![]).await;

        // Assert
        assert!(result.is_err());
        assert_eq!(
            result.expect_err("Expected error").to_string(),
            "DnsRecord expects HostedZone as a parent"
        );
    }

    #[tokio::test]
    async fn test_dns_record_manager_destroy_error() {
        // Arrange
        let mut route53_client_mock = client::Route53::default();
        route53_client_mock
            .expect_delete_dns_record()
            .return_once(|_, _, _, _, _| Err("Error".into()));

        let dns_record_manager = DnsRecordManager {
            client: &route53_client_mock,
        };

        let dns_record = DnsRecord {
            name: String::from("vm-id.example.com"),
            value: String::from("1.2.3.4"),
            record_type: types::RecordType::A,
            ttl: Some(300),
        };
        let hosted_zone = HostedZone {
            id: String::from("hz-id"),
            name: String::from("example.com"),
            region: String::from("us-west-2"),
        };
        let parents = [Node::Resource(ResourceType::HostedZone(hosted_zone))];

        // Act
        let result = dns_record_manager
            .destroy(&dns_record, parents.iter().collect())
            .await;

        // Assert
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_security_group_manager_create() {
        // Arrange
        let mut ec2_client_mock = client::Ec2::default();
        ec2_client_mock
            .expect_create_security_group()
            .with(
                eq(String::from("vpc-id")),
                eq(String::from("sg-name")),
                eq(String::from("No description")),
            )
            .return_once(|_, _, _| Ok(String::from("sg-id")));
        ec2_client_mock
            .expect_allow_inbound_traffic_for_security_group()
            .with(
                eq(String::from("sg-id")),
                eq(String::from("tcp")),
                eq(80),
                eq(String::from("0.0.0.0/0")),
            )
            .return_once(|_, _, _, _| Ok(()));

        let security_group_manager = SecurityGroupManager {
            client: &ec2_client_mock,
        };

        let security_group_spec = SecurityGroupSpec {
            name: String::from("sg-name"),
            inbound_rules: vec![InboundRule {
                protocol: String::from("tcp"),
                port: 80,
                cidr_block: String::from("0.0.0.0/0"),
            }],
        };
        let vpc = Vpc {
            id: String::from("vpc-id"),
            region: String::from("us-west-2"),
            cidr_block: String::from("10.0.0.0/16"),
            name: String::from("vpc-name"),
        };
        let parents = [Node::Resource(ResourceType::Vpc(vpc))];

        // Act
        let security_group = security_group_manager
            .create(&security_group_spec, parents.iter().collect())
            .await;

        // Assert
        assert!(security_group.is_ok());
        assert_eq!(
            security_group.expect("Failed to create security group"),
            SecurityGroup {
                id: String::from("sg-id"),
                name: String::from("sg-name"),
                inbound_rules: vec![InboundRule {
                    protocol: String::from("tcp"),
                    port: 80,
                    cidr_block: String::from("0.0.0.0/0"),
                }],
            }
        );
    }

    #[tokio::test]
    async fn test_security_group_manager_create_no_vpc_parent() {
        // Arrange
        let ec2_client_mock = client::Ec2::default();
        let security_group_manager = SecurityGroupManager {
            client: &ec2_client_mock,
        };
        let security_group_spec = SecurityGroupSpec {
            name: String::from("sg-name"),
            inbound_rules: vec![],
        };

        // Act
        let result = security_group_manager
            .create(&security_group_spec, vec![])
            .await;

        // Assert
        assert!(result.is_err());
        assert_eq!(
            result.expect_err("Expected error").to_string(),
            "SecurityGroup expects VPC as a parent"
        );
    }

    #[tokio::test]
    async fn test_security_group_manager_create_error() {
        // Arrange
        let mut ec2_client_mock = client::Ec2::default();
        ec2_client_mock
            .expect_create_security_group()
            .return_once(|_, _, _| Err("Error".into()));

        let security_group_manager = SecurityGroupManager {
            client: &ec2_client_mock,
        };

        let security_group_spec = SecurityGroupSpec {
            name: String::from("sg-name"),
            inbound_rules: vec![],
        };
        let vpc = Vpc {
            id: String::from("vpc-id"),
            region: String::from("us-west-2"),
            cidr_block: String::from("10.0.0.0/16"),
            name: String::from("vpc-name"),
        };
        let parents = [Node::Resource(ResourceType::Vpc(vpc))];

        // Act
        let security_group = security_group_manager
            .create(&security_group_spec, parents.iter().collect())
            .await;

        // Assert
        assert!(security_group.is_err());
    }

    #[tokio::test]
    async fn test_security_group_manager_destroy() {
        // Arrange
        let mut ec2_client_mock = client::Ec2::default();
        ec2_client_mock
            .expect_delete_security_group()
            .with(eq(String::from("sg-id")))
            .return_once(|_| Ok(()));

        let security_group_manager = SecurityGroupManager {
            client: &ec2_client_mock,
        };

        let security_group = SecurityGroup {
            id: String::from("sg-id"),
            name: String::from("sg-name"),
            inbound_rules: vec![],
        };

        // Act
        let result = security_group_manager
            .destroy(&security_group, vec![])
            .await;

        // Assert
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_security_group_manager_destroy_error() {
        // Arrange
        let mut ec2_client_mock = client::Ec2::default();
        ec2_client_mock
            .expect_delete_security_group()
            .return_once(|_| Err("Error".into()));

        let security_group_manager = SecurityGroupManager {
            client: &ec2_client_mock,
        };

        let security_group = SecurityGroup {
            id: String::from("sg-id"),
            name: String::from("sg-name"),
            inbound_rules: vec![],
        };

        // Act
        let result = security_group_manager
            .destroy(&security_group, vec![])
            .await;

        // Assert
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_instance_profile_manager_create() {
        // Arrange
        let mut iam_client_mock = client::IAM::default();
        iam_client_mock
            .expect_create_instance_profile()
            .with(
                eq(String::from("profile-name")),
                eq(vec![String::from("role-name")]),
            )
            .return_once(|_, _| Ok(()));

        let instance_profile_manager = InstanceProfileManager {
            client: &iam_client_mock,
        };

        let instance_profile_spec = InstanceProfileSpec {
            name: String::from("profile-name"),
        };
        let instance_role = InstanceRole {
            name: String::from("role-name"),
            assume_role_policy: String::new(),
            policy_arns: vec![],
        };
        let parents = [Node::Resource(ResourceType::InstanceRole(instance_role))];

        // Act
        let instance_profile = instance_profile_manager
            .create(&instance_profile_spec, parents.iter().collect())
            .await;

        // Assert
        assert!(instance_profile.is_ok());
        assert_eq!(
            instance_profile.expect("Failed to create instance profile"),
            InstanceProfile {
                name: String::from("profile-name"),
            }
        );
    }

    #[tokio::test]
    async fn test_instance_profile_manager_create_no_instance_role_parent() {
        // Arrange
        let mut iam_client_mock = client::IAM::default();
        iam_client_mock
            .expect_create_instance_profile()
            .with(eq(String::from("profile-name")), eq(Vec::<String>::new()))
            .return_once(|_, _| Ok(()));
        let instance_profile_manager = InstanceProfileManager {
            client: &iam_client_mock,
        };
        let instance_profile_spec = InstanceProfileSpec {
            name: String::from("profile-name"),
        };

        // Act
        let result = instance_profile_manager
            .create(&instance_profile_spec, vec![])
            .await;

        // Assert
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_instance_profile_manager_create_error() {
        // Arrange
        let mut iam_client_mock = client::IAM::default();
        iam_client_mock
            .expect_create_instance_profile()
            .return_once(|_, _| Err("Error".into()));

        let instance_profile_manager = InstanceProfileManager {
            client: &iam_client_mock,
        };

        let instance_profile_spec = InstanceProfileSpec {
            name: String::from("profile-name"),
        };

        // Act
        let result = instance_profile_manager
            .create(&instance_profile_spec, vec![])
            .await;

        // Assert
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_instance_profile_manager_destroy() {
        // Arrange
        let mut iam_client_mock = client::IAM::default();
        iam_client_mock
            .expect_delete_instance_profile()
            .with(
                eq(String::from("profile-name")),
                eq(vec![String::from("role-name")]),
            )
            .return_once(|_, _| Ok(()));

        let instance_profile_manager = InstanceProfileManager {
            client: &iam_client_mock,
        };

        let instance_profile = InstanceProfile {
            name: String::from("profile-name"),
        };
        let instance_role = InstanceRole {
            name: String::from("role-name"),
            assume_role_policy: String::new(),
            policy_arns: vec![],
        };
        let parents = [Node::Resource(ResourceType::InstanceRole(instance_role))];

        // Act
        let result = instance_profile_manager
            .destroy(&instance_profile, parents.iter().collect())
            .await;

        // Assert
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_instance_profile_manager_destroy_no_instance_role_parent() {
        // Arrange
        let mut iam_client_mock = client::IAM::default();
        iam_client_mock
            .expect_delete_instance_profile()
            .with(eq(String::from("profile-name")), eq(Vec::<String>::new()))
            .return_once(|_, _| Ok(()));
        let instance_profile_manager = InstanceProfileManager {
            client: &iam_client_mock,
        };
        let instance_profile = InstanceProfile {
            name: String::from("profile-name"),
        };

        // Act
        let result = instance_profile_manager
            .destroy(&instance_profile, vec![])
            .await;

        // Assert
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_instance_profile_manager_destroy_error() {
        // Arrange
        let mut iam_client_mock = client::IAM::default();
        iam_client_mock
            .expect_delete_instance_profile()
            .return_once(|_, _| Err("Error".into()));

        let instance_profile_manager = InstanceProfileManager {
            client: &iam_client_mock,
        };

        let instance_profile = InstanceProfile {
            name: String::from("profile-name"),
        };

        // Act
        let result = instance_profile_manager
            .destroy(&instance_profile, vec![])
            .await;

        // Assert
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_vm_manager_create() {
        // Arrange
        let mut ec2_client_mock = client::Ec2::default();
        ec2_client_mock
            .expect_run_instances()
            .return_once(|_, _, _, _, _, _| {
                let instance = aws_sdk_ec2::types::Instance::builder()
                    .instance_id("vm-id")
                    .build();
                let output = aws_sdk_ec2::operation::run_instances::RunInstancesOutput::builder()
                    .instances(instance)
                    .build();
                Ok(output)
            });
        ec2_client_mock
            .expect_describe_instances()
            .return_once(|_| {
                let instance = aws_sdk_ec2::types::Instance::builder()
                    .public_ip_address("1.2.3.4")
                    .build();
                Ok(instance)
            });

        let vm_manager = VmManager {
            client: &ec2_client_mock,
        };

        let vm_spec = VmSpec {
            instance_type: types::InstanceType::T2Micro,
            ami: String::from("ami-123"),
            user_data: String::from("user-data"),
        };

        let subnet = Subnet {
            id: String::from("subnet-id"),
            name: String::from("subnet-name"),
            cidr_block: String::from("10.0.1.0/24"),
            availability_zone: String::from("us-west-2a"),
        };
        let ecr = Ecr {
            id: String::from("ecr-id"),
            uri: String::from("dkr.ecr.region.amazonaws.com/repo"),
            name: String::from("ecr-name"),
        };
        let instance_profile = InstanceProfile {
            name: String::from("instance-profile-name"),
        };
        let security_group = SecurityGroup {
            id: String::from("sg-id"),
            name: String::from("sg-name"),
            inbound_rules: vec![],
        };

        let parents = [
            Node::Resource(ResourceType::Subnet(subnet)),
            Node::Resource(ResourceType::Ecr(ecr)),
            Node::Resource(ResourceType::InstanceProfile(instance_profile)),
            Node::Resource(ResourceType::SecurityGroup(security_group)),
        ];

        // Act
        let vm = vm_manager.create(&vm_spec, parents.iter().collect()).await;

        // Assert
        assert!(vm.is_ok());
        assert_eq!(
            vm.expect("failed to get vm"),
            Vm {
                id: String::from("vm-id"),
                public_ip: String::from("1.2.3.4"),
                instance_type: types::InstanceType::T2Micro,
                ami: String::from("ami-123"),
                user_data: String::from(
                    "user-data\naws ecr get-login-password --region us-west-2 | podman login --username AWS --password-stdin dkr.ecr.region.amazonaws.com",
                ),
            }
        );
    }

    #[tokio::test]
    async fn test_vm_manager_create_no_subnet_parent() {
        // Arrange
        let ec2_client_mock = client::Ec2::default();
        let vm_manager = VmManager {
            client: &ec2_client_mock,
        };
        let vm_spec = VmSpec {
            instance_type: types::InstanceType::T2Micro,
            ami: String::from("ami-123"),
            user_data: String::from("user-data"),
        };
        let ecr = Ecr {
            id: String::from("ecr-id"),
            uri: String::from("dkr.ecr.region.amazonaws.com/repo"),
            name: String::from("ecr-name"),
        };
        let instance_profile = InstanceProfile {
            name: String::from("instance-profile-name"),
        };
        let security_group = SecurityGroup {
            id: String::from("sg-id"),
            name: String::from("sg-name"),
            inbound_rules: vec![],
        };
        let parents = [
            Node::Resource(ResourceType::Ecr(ecr)),
            Node::Resource(ResourceType::InstanceProfile(instance_profile)),
            Node::Resource(ResourceType::SecurityGroup(security_group)),
        ];

        // Act
        let result = vm_manager.create(&vm_spec, parents.iter().collect()).await;

        // Assert
        assert!(result.is_err());
        assert_eq!(
            result.expect_err("Expected error").to_string(),
            "VM expects Subnet as a parent"
        );
    }

    #[tokio::test]
    async fn test_vm_manager_create_no_ecr_parent() {
        // Arrange
        let ec2_client_mock = client::Ec2::default();
        let vm_manager = VmManager {
            client: &ec2_client_mock,
        };
        let vm_spec = VmSpec {
            instance_type: types::InstanceType::T2Micro,
            ami: String::from("ami-123"),
            user_data: String::from("user-data"),
        };
        let subnet = Subnet {
            id: String::from("subnet-id"),
            name: String::from("subnet-name"),
            cidr_block: String::from("10.0.1.0/24"),
            availability_zone: String::from("us-west-2a"),
        };
        let instance_profile = InstanceProfile {
            name: String::from("instance-profile-name"),
        };
        let security_group = SecurityGroup {
            id: String::from("sg-id"),
            name: String::from("sg-name"),
            inbound_rules: vec![],
        };
        let parents = [
            Node::Resource(ResourceType::Subnet(subnet)),
            Node::Resource(ResourceType::InstanceProfile(instance_profile)),
            Node::Resource(ResourceType::SecurityGroup(security_group)),
        ];

        // Act
        let result = vm_manager.create(&vm_spec, parents.iter().collect()).await;

        // Assert
        assert!(result.is_err());
        assert_eq!(
            result.expect_err("Expected error").to_string(),
            "VM expects Ecr as a parent"
        );
    }

    #[tokio::test]
    async fn test_vm_manager_create_no_instance_profile_parent() {
        // Arrange
        let ec2_client_mock = client::Ec2::default();
        let vm_manager = VmManager {
            client: &ec2_client_mock,
        };
        let vm_spec = VmSpec {
            instance_type: types::InstanceType::T2Micro,
            ami: String::from("ami-123"),
            user_data: String::from("user-data"),
        };
        let subnet = Subnet {
            id: String::from("subnet-id"),
            name: String::from("subnet-name"),
            cidr_block: String::from("10.0.1.0/24"),
            availability_zone: String::from("us-west-2a"),
        };
        let ecr = Ecr {
            id: String::from("ecr-id"),
            uri: String::from("dkr.ecr.region.amazonaws.com/repo"),
            name: String::from("ecr-name"),
        };
        let security_group = SecurityGroup {
            id: String::from("sg-id"),
            name: String::from("sg-name"),
            inbound_rules: vec![],
        };
        let parents = [
            Node::Resource(ResourceType::Subnet(subnet)),
            Node::Resource(ResourceType::Ecr(ecr)),
            Node::Resource(ResourceType::SecurityGroup(security_group)),
        ];

        // Act
        let result = vm_manager.create(&vm_spec, parents.iter().collect()).await;

        // Assert
        assert!(result.is_err());
        assert_eq!(
            result.expect_err("Expected error").to_string(),
            "VM expects InstanceProfile as a parent"
        );
    }

    #[tokio::test]
    async fn test_vm_manager_create_no_security_group_parent() {
        // Arrange
        let ec2_client_mock = client::Ec2::default();
        let vm_manager = VmManager {
            client: &ec2_client_mock,
        };
        let vm_spec = VmSpec {
            instance_type: types::InstanceType::T2Micro,
            ami: String::from("ami-123"),
            user_data: String::from("user-data"),
        };
        let subnet = Subnet {
            id: String::from("subnet-id"),
            name: String::from("subnet-name"),
            cidr_block: String::from("10.0.1.0/24"),
            availability_zone: String::from("us-west-2a"),
        };
        let ecr = Ecr {
            id: String::from("ecr-id"),
            uri: String::from("dkr.ecr.region.amazonaws.com/repo"),
            name: String::from("ecr-name"),
        };
        let instance_profile = InstanceProfile {
            name: String::from("instance-profile-name"),
        };
        let parents = [
            Node::Resource(ResourceType::Subnet(subnet)),
            Node::Resource(ResourceType::Ecr(ecr)),
            Node::Resource(ResourceType::InstanceProfile(instance_profile)),
        ];

        // Act
        let result = vm_manager.create(&vm_spec, parents.iter().collect()).await;

        // Assert
        assert!(result.is_err());
        assert_eq!(
            result.expect_err("Expected error").to_string(),
            "SecurityGroup expects VPC as a parent"
        );
    }

    #[tokio::test]
    async fn test_vm_manager_create_error() {
        // Arrange
        let mut ec2_client_mock = client::Ec2::default();
        ec2_client_mock
            .expect_run_instances()
            .return_once(|_, _, _, _, _, _| Err("Error".into()));

        let vm_manager = VmManager {
            client: &ec2_client_mock,
        };

        let vm_spec = VmSpec {
            instance_type: types::InstanceType::T2Micro,
            ami: String::from("ami-123"),
            user_data: String::from("user-data"),
        };
        let subnet = Subnet {
            id: String::from("subnet-id"),
            name: String::from("subnet-name"),
            cidr_block: String::from("10.0.1.0/24"),
            availability_zone: String::from("us-west-2a"),
        };
        let ecr = Ecr {
            id: String::from("ecr-id"),
            uri: String::from("dkr.ecr.region.amazonaws.com/repo"),
            name: String::from("ecr-name"),
        };
        let instance_profile = InstanceProfile {
            name: String::from("instance-profile-name"),
        };
        let security_group = SecurityGroup {
            id: String::from("sg-id"),
            name: String::from("sg-name"),
            inbound_rules: vec![],
        };
        let parents = [
            Node::Resource(ResourceType::Subnet(subnet)),
            Node::Resource(ResourceType::Ecr(ecr)),
            Node::Resource(ResourceType::InstanceProfile(instance_profile)),
            Node::Resource(ResourceType::SecurityGroup(security_group)),
        ];

        // Act
        let result = vm_manager.create(&vm_spec, parents.iter().collect()).await;

        // Assert
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_vm_manager_destroy() {
        // Arrange
        let mut ec2_client_mock = client::Ec2::default();
        ec2_client_mock
            .expect_terminate_instance()
            .with(eq(String::from("vm-id")))
            .return_once(|_| Ok(()));
        ec2_client_mock
            .expect_describe_instances()
            .with(eq(String::from("vm-id")))
            .return_once(|_| {
                let state = aws_sdk_ec2::types::InstanceState::builder()
                    .name(aws_sdk_ec2::types::InstanceStateName::Terminated)
                    .build();
                let instance = aws_sdk_ec2::types::Instance::builder().state(state).build();
                Ok(instance)
            });

        let vm_manager = VmManager {
            client: &ec2_client_mock,
        };

        let vm = Vm {
            id: String::from("vm-id"),
            public_ip: String::from("1.2.3.4"),
            instance_type: types::InstanceType::T2Micro,
            ami: String::from("ami-123"),
            user_data: String::from("user-data"),
        };

        // Act
        let result = vm_manager.destroy(&vm, vec![]).await;

        // Assert
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_vm_manager_destroy_error() {
        // Arrange
        let mut ec2_client_mock = client::Ec2::default();
        ec2_client_mock
            .expect_terminate_instance()
            .with(eq(String::from("vm-id")))
            .return_once(|_| Err("Error".into()));

        let vm_manager = VmManager {
            client: &ec2_client_mock,
        };

        let vm = Vm {
            id: String::from("vm-id"),
            public_ip: String::from("1.2.3.4"),
            instance_type: types::InstanceType::T2Micro,
            ami: String::from("ami-123"),
            user_data: String::from("user-data"),
        };

        // Act
        let result = vm_manager.destroy(&vm, vec![]).await;

        // Assert
        assert!(result.is_err());
    }
}
