use aws_sdk_ec2::types::InstanceStateName;
use petgraph::{Incoming, Outgoing};

use base64::{engine::general_purpose, Engine as _};
use petgraph::visit::NodeIndexable;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

use petgraph::dot::Dot;
use petgraph::graph::NodeIndex;
use petgraph::Graph;

use crate::aws::client;
use crate::aws::types;

/// Defines the main methods to manage resources
trait Manager<'a, I, O>
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
    region: String,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostedZone {
    id: String,

    region: String,
    name: String,
}

struct HostedZoneManager<'a> {
    client: &'a client::Route53,
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

#[derive(Debug)]
pub struct DnsRecordSpec {
    record_type: types::RecordType,
    ttl: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsRecord {
    name: String,
    value: String,

    record_type: types::RecordType,
    ttl: Option<i64>,
}

struct DnsRecordManager<'a> {
    client: &'a client::Route53,
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
    region: String,
    cidr_block: String,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vpc {
    id: String,

    region: String,
    cidr_block: String,
    name: String,
}

struct VpcManager<'a> {
    client: &'a client::Ec2,
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

#[derive(Debug)]
pub struct InternetGatewaySpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternetGateway {
    id: String,
}

struct InternetGatewayManager<'a> {
    client: &'a client::Ec2,
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

#[derive(Debug)]
pub struct RouteTableSpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteTable {
    id: String,
}

struct RouteTableManager<'a> {
    client: &'a client::Ec2,
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

#[derive(Debug)]
pub struct SubnetSpec {
    name: String,
    cidr_block: String,
    availability_zone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subnet {
    id: String,

    name: String,
    cidr_block: String,
    availability_zone: String,
}

struct SubnetManager<'a> {
    client: &'a client::Ec2,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundRule {
    protocol: String,
    port: i32,
    cidr_block: String,
}

#[derive(Debug)]
pub struct SecurityGroupSpec {
    name: String,
    inbound_rules: Vec<InboundRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityGroup {
    id: String,

    name: String,
    inbound_rules: Vec<InboundRule>,
}

struct SecurityGroupManager<'a> {
    client: &'a client::Ec2,
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
    name: String,
    assume_role_policy: String,
    policy_arns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceRole {
    name: String,
    assume_role_policy: String,
    policy_arns: Vec<String>,
}

struct InstanceRoleManager<'a> {
    client: &'a client::IAM,
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

#[derive(Debug)]
pub struct InstanceProfileSpec {
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceProfile {
    name: String,
}

struct InstanceProfileManager<'a> {
    client: &'a client::IAM,
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

#[derive(Debug)]
pub struct EcrSpec {
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ecr {
    id: String,
    pub uri: String,

    name: String,
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

struct EcrManager<'a> {
    client: &'a client::ECR,
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
    instance_type: types::InstanceType,
    ami: String,
    user_data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vm {
    pub id: String,
    pub public_ip: String,

    pub instance_type: types::InstanceType,
    ami: String,
    user_data: String,
}

struct VmManager<'a> {
    client: &'a client::Ec2,
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

            log::info!(
                "VM is not terminated yet... \
                 retrying in {sleep_duration} sec...",
            );
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
        let user_data = format!("{}\n{}", input.user_data, ecr_login_string);
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

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
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
    fn name(&self) -> String {
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

#[derive(Debug, Default, Clone)]
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

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct State {
    resources: Vec<ResourceState>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ResourceState {
    name: String,
    resource: ResourceType,
    dependencies: Vec<String>,
}

impl State {
    pub fn from_graph(graph: &Graph<Node, String>) -> Self {
        let mut resource_states: Vec<ResourceState> = Vec::new();

        let mut parents: HashMap<NodeIndex, Vec<NodeIndex>> = HashMap::new();

        let mut queue: VecDeque<NodeIndex> = VecDeque::new();
        let root_index = graph.from_index(0);
        for node_index in graph.neighbors(root_index) {
            queue.push_back(node_index);

            parents
                .entry(node_index)
                .or_insert_with(Vec::new)
                .push(root_index);
        }

        while let Some(node_index) = queue.pop_front() {
            for neighbor_node_index in graph.neighbors(node_index) {
                if !parents.contains_key(&neighbor_node_index) {
                    queue.push_back(neighbor_node_index);
                }

                parents
                    .entry(neighbor_node_index)
                    .or_insert_with(Vec::new)
                    .push(node_index);
            }
        }

        for (child_index, parents) in &parents {
            let parent_node_names = parents
                .iter()
                .filter_map(|x| graph.node_weight(*x))
                .filter_map(|x| match x {
                    Node::Root => None,
                    Node::Resource(parent_resource_type) => Some(parent_resource_type.name()),
                })
                .collect();

            if let Some(Node::Resource(resource_type)) = graph.node_weight(*child_index) {
                log::info!("Add to state {resource_type:?}");

                resource_states.push(ResourceState {
                    name: resource_type.name(),
                    resource: resource_type.clone(),
                    dependencies: parent_node_names,
                });
            }
        }

        Self {
            resources: resource_states,
        }
    }

    pub fn to_graph(&self) -> Graph<Node, String> {
        let mut graph = Graph::<Node, String>::new();
        let mut edges = Vec::new();
        let root = graph.add_node(Node::Root);

        let mut resources_map: HashMap<String, NodeIndex> = HashMap::new();
        for resource_state in &self.resources {
            let node = graph.add_node(Node::Resource(resource_state.resource.clone()));

            resources_map.insert(resource_state.name.clone(), node);
        }

        for resource_state in &self.resources {
            let resource = resources_map
                .get(&resource_state.name)
                .expect("Missed resource value in resource_map");

            if resource_state.dependencies.is_empty() {
                edges.push((root, *resource, String::new()));
            } else {
                for dependency_name in &resource_state.dependencies {
                    let dependency_resource = resources_map
                        .get(dependency_name)
                        .expect("Missed dependency resource value in resource_map");

                    edges.push((*dependency_resource, *resource, String::new()));
                }
            }
        }

        graph.extend_with_edges(&edges);

        graph
    }
}

pub struct GraphManager {
    ec2_client: client::Ec2,
    iam_client: client::IAM,
    ecr_client: client::ECR,
    route53_client: client::Route53,
}

impl GraphManager {
    pub async fn new() -> Self {
        let region_provider = aws_sdk_ec2::config::Region::new("us-west-2");
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .credentials_provider(
                aws_config::profile::ProfileFileCredentialsProvider::builder()
                    .profile_name("default")
                    .build(),
            )
            .region(region_provider)
            .load()
            .await;

        let ec2_client = client::Ec2::new(aws_sdk_ec2::Client::new(&config));
        let iam_client = client::IAM::new(aws_sdk_iam::Client::new(&config));
        let ecr_client = client::ECR::new(aws_sdk_ecr::Client::new(&config));
        let route53_client = client::Route53::new(aws_sdk_route53::Client::new(&config));

        Self {
            ec2_client,
            iam_client,
            ecr_client,
            route53_client,
        }
    }

    pub fn get_spec_graph(
        number_of_instances: u32,
        instance_type: &types::InstanceType,
        domain_name: Option<String>,
    ) -> Graph<SpecNode, String> {
        let mut deps = Graph::<SpecNode, String>::new();
        let root = deps.add_node(SpecNode::Root);

        let vpc_1 = deps.add_node(SpecNode::Resource(ResourceSpecType::Vpc(VpcSpec {
            region: String::from("us-west-2"),
            cidr_block: String::from("10.0.0.0/16"),
            name: String::from("vpc-1"),
        })));

        let igw_1 = deps.add_node(SpecNode::Resource(ResourceSpecType::InternetGateway(
            InternetGatewaySpec,
        )));

        let route_table_1 = deps.add_node(SpecNode::Resource(ResourceSpecType::RouteTable(
            RouteTableSpec,
        )));

        let subnet_1 = deps.add_node(SpecNode::Resource(ResourceSpecType::Subnet(SubnetSpec {
            name: String::from("vpc-1-subnet"),
            cidr_block: String::from("10.0.1.0/24"),
            availability_zone: String::from("us-west-2a"),
        })));

        let security_group_1 = deps.add_node(SpecNode::Resource(ResourceSpecType::SecurityGroup(
            SecurityGroupSpec {
                name: String::from("vpc-1-security-group"),
                inbound_rules: vec![
                    InboundRule {
                        cidr_block: "0.0.0.0/0".to_string(),
                        protocol: "tcp".to_string(),
                        port: 80,
                    },
                    InboundRule {
                        cidr_block: "0.0.0.0/0".to_string(),
                        protocol: "tcp".to_string(),
                        port: 31888,
                    },
                    InboundRule {
                        cidr_block: "0.0.0.0/0".to_string(),
                        protocol: "tcp".to_string(),
                        port: 22,
                    },
                ],
            },
        )));

        let instance_role_1 = deps.add_node(SpecNode::Resource(ResourceSpecType::InstanceRole(
            InstanceRoleSpec {
                name: String::from("instance-role-1"),
                assume_role_policy: String::from(
                    r#"{
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
                    }"#,
                ),
                policy_arns: vec![String::from(
                    "arn:aws:iam::aws:policy/AmazonEC2ContainerRegistryReadOnly",
                )],
            },
        )));

        let instance_profile_1 = deps.add_node(SpecNode::Resource(
            ResourceSpecType::InstanceProfile(InstanceProfileSpec {
                name: String::from("instance_profile_1"),
            }),
        ));

        let ecr_1 = deps.add_node(SpecNode::Resource(ResourceSpecType::Ecr(EcrSpec {
            name: String::from("ecr_1"),
        })));

        let user_data = String::from(
            r#"#!/bin/bash
        set -e
        sudo apt update
        sudo apt -y install podman
        sudo systemctl start podman
        sudo snap install aws-cli --classic

        curl \
            --output /home/ubuntu/oct-ctl \
            -L \
            https://github.com/opencloudtool/opencloudtool/releases/download/tip/oct-ctl \
            && sudo chmod +x /home/ubuntu/oct-ctl \
            && /home/ubuntu/oct-ctl &
        "#,
        );

        // TODO: Add instance profile with instance role
        let mut instances = Vec::new();
        for _ in 0..number_of_instances {
            let instance_node = deps.add_node(SpecNode::Resource(ResourceSpecType::Vm(VmSpec {
                instance_type: instance_type.clone(),
                ami: String::from("ami-04dd23e62ed049936"),
                user_data: user_data.clone(),
            })));

            instances.push(instance_node);
        }

        // Order of the edges matters in this implementation
        // Nodes within the same parent are traversed from
        // the latest to the first
        let mut edges = vec![
            (root, ecr_1, String::new()),                         // 2
            (root, instance_role_1, String::new()),               // 1
            (root, vpc_1, String::new()),                         // 0
            (vpc_1, security_group_1, String::new()),             // 6
            (vpc_1, subnet_1, String::new()),                     // 5
            (vpc_1, route_table_1, String::new()),                // 4
            (vpc_1, igw_1, String::new()),                        // 3
            (igw_1, route_table_1, String::new()),                // 7
            (route_table_1, subnet_1, String::new()),             // 8
            (instance_role_1, instance_profile_1, String::new()), // 9
        ];
        for instance in &instances {
            edges.push((subnet_1, *instance, String::new()));
            edges.push((instance_profile_1, *instance, String::new()));
            edges.push((security_group_1, *instance, String::new()));
            edges.push((ecr_1, *instance, String::new()));
        }

        if let Some(domain_name) = domain_name {
            let hosted_zone = deps.add_node(SpecNode::Resource(ResourceSpecType::HostedZone(
                HostedZoneSpec {
                    region: String::from("us-west-2"),
                    name: domain_name,
                },
            )));

            // Insert at the first place to deploy it after all other root's children
            edges.insert(0, (root, hosted_zone, String::new()));

            for instance in instances {
                let dns_record = deps.add_node(SpecNode::Resource(ResourceSpecType::DnsRecord(
                    DnsRecordSpec {
                        record_type: types::RecordType::A,
                        ttl: Some(3600),
                    },
                )));

                edges.push((instance, dns_record, String::new()));
                edges.push((hosted_zone, dns_record, String::new()));
            }
        }

        deps.extend_with_edges(&edges);

        deps
    }

    /// Deploy spec graph
    ///
    /// Temporarily also returns a list of VMs and optional ECR
    /// to be used for user services deployment
    pub async fn deploy(
        &self,
        graph: &Graph<SpecNode, String>,
    ) -> (Graph<Node, String>, Vec<Vm>, Option<Ecr>) {
        let mut resource_graph = Graph::<Node, String>::new();
        let mut edges = vec![];
        let root_index = resource_graph.add_node(Node::Root);

        let mut parents: HashMap<NodeIndex, Vec<NodeIndex>> = HashMap::new();

        let mut queue: VecDeque<NodeIndex> = VecDeque::new();
        let root_node = graph.from_index(0);
        for node_index in graph.neighbors(root_node) {
            queue.push_back(node_index);

            parents
                .entry(node_index)
                .or_insert_with(Vec::new)
                .push(root_index);
        }

        let mut ecr: Option<Ecr> = None;
        let mut vms: Vec<Vm> = Vec::new();

        // TODO(minev-dev): Use Self::kahn_traverse to simplify traverse with no edge creation
        //  ordering required
        while let Some(node_index) = queue.pop_front() {
            let parent_node_indexes = match parents.get(&node_index) {
                Some(parent_node_indexes) => parent_node_indexes.clone(),
                None => Vec::new(),
            };
            let parent_nodes = parent_node_indexes
                .iter()
                .filter_map(|x| resource_graph.node_weight(*x))
                .collect();

            if let Some(elem) = graph.node_weight(node_index) {
                let created_resource_node_index = match elem {
                    SpecNode::Root => Ok(resource_graph.add_node(Node::Root)),
                    SpecNode::Resource(resource_type) => match resource_type {
                        ResourceSpecType::HostedZone(resource) => {
                            let manager = HostedZoneManager {
                                client: &self.route53_client,
                            };
                            let output_resource = manager.create(resource, parent_nodes).await;

                            match output_resource {
                                Ok(output_resource) => {
                                    log::info!(
                                        "Deployed {output_resource:?}, parents - {parent_node_indexes:?}"
                                    );

                                    let node =
                                        Node::Resource(ResourceType::HostedZone(output_resource));
                                    let resource_index = resource_graph.add_node(node.clone());

                                    for parent_node_index in parent_node_indexes {
                                        edges.push((
                                            parent_node_index,
                                            resource_index,
                                            String::new(),
                                        ));
                                    }

                                    Ok(resource_index)
                                }
                                Err(e) => Err(Box::new(e)),
                            }
                        }
                        ResourceSpecType::DnsRecord(resource) => {
                            let manager = DnsRecordManager {
                                client: &self.route53_client,
                            };
                            let output_resource = manager.create(resource, parent_nodes).await;

                            match output_resource {
                                Ok(output_resource) => {
                                    log::info!(
                                        "Deployed {output_resource:?}, parents - {parent_node_indexes:?}"
                                    );

                                    let node =
                                        Node::Resource(ResourceType::DnsRecord(output_resource));
                                    let resource_index = resource_graph.add_node(node.clone());

                                    for parent_node_index in parent_node_indexes {
                                        edges.push((
                                            parent_node_index,
                                            resource_index,
                                            String::new(),
                                        ));
                                    }

                                    Ok(resource_index)
                                }
                                Err(e) => Err(Box::new(e)),
                            }
                        }
                        ResourceSpecType::Vpc(resource) => {
                            let manager = VpcManager {
                                client: &self.ec2_client,
                            };
                            let output_vpc = manager.create(resource, parent_nodes).await;

                            match output_vpc {
                                Ok(output_vpc) => {
                                    log::info!(
                                        "Deployed {output_vpc:?}, parents - {parent_node_indexes:?}"
                                    );

                                    let node = Node::Resource(ResourceType::Vpc(output_vpc));
                                    let vpc_index = resource_graph.add_node(node.clone());

                                    for parent_node_index in parent_node_indexes {
                                        edges.push((parent_node_index, vpc_index, String::new()));
                                    }

                                    Ok(vpc_index)
                                }
                                Err(e) => Err(Box::new(e)),
                            }
                        }
                        ResourceSpecType::InternetGateway(resource) => {
                            let manager = InternetGatewayManager {
                                client: &self.ec2_client,
                            };
                            let output_igw = manager.create(resource, parent_nodes).await;

                            match output_igw {
                                Ok(output_igw) => {
                                    log::info!(
                                        "Deployed {output_igw:?}, parents - {parent_node_indexes:?}"
                                    );

                                    let node =
                                        Node::Resource(ResourceType::InternetGateway(output_igw));
                                    let igw_index = resource_graph.add_node(node.clone());

                                    for parent_node_index in parent_node_indexes {
                                        edges.push((parent_node_index, igw_index, String::new()));
                                    }

                                    Ok(igw_index)
                                }
                                Err(e) => Err(Box::new(e)),
                            }
                        }
                        ResourceSpecType::RouteTable(resource) => {
                            let manager = RouteTableManager {
                                client: &self.ec2_client,
                            };
                            let output_route_table = manager.create(resource, parent_nodes).await;

                            match output_route_table {
                                Ok(output_route_table) => {
                                    log::info!(
                                        "Deployed {output_route_table:?}, parents - {parent_node_indexes:?}"
                                    );

                                    let node = Node::Resource(ResourceType::RouteTable(
                                        output_route_table,
                                    ));
                                    let route_table_index = resource_graph.add_node(node.clone());

                                    for parent_node_index in parent_node_indexes {
                                        edges.push((
                                            parent_node_index,
                                            route_table_index,
                                            String::new(),
                                        ));
                                    }

                                    Ok(route_table_index)
                                }
                                Err(e) => Err(Box::new(e)),
                            }
                        }
                        ResourceSpecType::Subnet(resource) => {
                            let manager = SubnetManager {
                                client: &self.ec2_client,
                            };
                            let output_subnet = manager.create(resource, parent_nodes).await;

                            match output_subnet {
                                Ok(output_subnet) => {
                                    log::info!(
                                        "Deployed {output_subnet:?}, parents - {parent_node_indexes:?}"
                                    );

                                    let node = Node::Resource(ResourceType::Subnet(output_subnet));
                                    let subnet_index = resource_graph.add_node(node.clone());

                                    for parent_node_index in parent_node_indexes {
                                        edges.push((
                                            parent_node_index,
                                            subnet_index,
                                            String::new(),
                                        ));
                                    }

                                    Ok(subnet_index)
                                }
                                Err(e) => Err(Box::new(e)),
                            }
                        }
                        ResourceSpecType::SecurityGroup(resource) => {
                            let manager = SecurityGroupManager {
                                client: &self.ec2_client,
                            };
                            let output_security_group =
                                manager.create(resource, parent_nodes).await;

                            match output_security_group {
                                Ok(output_security_group) => {
                                    log::info!(
                                        "Deployed {output_security_group:?}, parents - {parent_node_indexes:?}"
                                    );

                                    let node = Node::Resource(ResourceType::SecurityGroup(
                                        output_security_group,
                                    ));
                                    let security_group_index =
                                        resource_graph.add_node(node.clone());

                                    for parent_node_index in parent_node_indexes {
                                        edges.push((
                                            parent_node_index,
                                            security_group_index,
                                            String::new(),
                                        ));
                                    }

                                    Ok(security_group_index)
                                }
                                Err(e) => Err(Box::new(e)),
                            }
                        }
                        ResourceSpecType::InstanceRole(resource) => {
                            let manager = InstanceRoleManager {
                                client: &self.iam_client,
                            };
                            let output_instance_role = manager.create(resource, parent_nodes).await;

                            match output_instance_role {
                                Ok(output_instance_role) => {
                                    log::info!(
                                        "Deployed {output_instance_role:?}, parents - {parent_node_indexes:?}"
                                    );

                                    let node = Node::Resource(ResourceType::InstanceRole(
                                        output_instance_role,
                                    ));
                                    let instance_role_index = resource_graph.add_node(node.clone());

                                    for parent_node_index in parent_node_indexes {
                                        edges.push((
                                            parent_node_index,
                                            instance_role_index,
                                            String::new(),
                                        ));
                                    }

                                    Ok(instance_role_index)
                                }
                                Err(e) => Err(Box::new(e)),
                            }
                        }
                        ResourceSpecType::InstanceProfile(resource) => {
                            let manager = InstanceProfileManager {
                                client: &self.iam_client,
                            };
                            let output_resource = manager.create(resource, parent_nodes).await;

                            match output_resource {
                                Ok(output_resource) => {
                                    log::info!(
                                        "Deployed {output_resource:?}, parents - {parent_node_indexes:?}"
                                    );

                                    let node = Node::Resource(ResourceType::InstanceProfile(
                                        output_resource,
                                    ));
                                    let resource_node_index = resource_graph.add_node(node.clone());

                                    for parent_node_index in parent_node_indexes {
                                        edges.push((
                                            parent_node_index,
                                            resource_node_index,
                                            String::new(),
                                        ));
                                    }

                                    Ok(resource_node_index)
                                }
                                Err(e) => Err(Box::new(e)),
                            }
                        }
                        ResourceSpecType::Ecr(resource) => {
                            let manager = EcrManager {
                                client: &self.ecr_client,
                            };
                            let output_resource = manager.create(resource, parent_nodes).await;

                            match output_resource {
                                Ok(output_resource) => {
                                    log::info!(
                                        "Deployed {output_resource:?}, parents - {parent_node_indexes:?}"
                                    );

                                    let node =
                                        Node::Resource(ResourceType::Ecr(output_resource.clone()));
                                    let resource_node_index = resource_graph.add_node(node.clone());

                                    for parent_node_index in parent_node_indexes {
                                        edges.push((
                                            parent_node_index,
                                            resource_node_index,
                                            String::new(),
                                        ));
                                    }

                                    ecr = Some(output_resource);

                                    Ok(resource_node_index)
                                }
                                Err(e) => Err(Box::new(e)),
                            }
                        }
                        ResourceSpecType::Vm(resource) => {
                            let manager = VmManager {
                                client: &self.ec2_client,
                            };
                            let output_vm = manager.create(resource, parent_nodes).await;

                            match output_vm {
                                Ok(output_vm) => {
                                    log::info!(
                                        "Deployed {output_vm:?}, parents - {parent_node_indexes:?}"
                                    );

                                    let node = Node::Resource(ResourceType::Vm(output_vm.clone()));
                                    let vm_index = resource_graph.add_node(node.clone());

                                    for parent_node_index in parent_node_indexes {
                                        edges.push((parent_node_index, vm_index, String::new()));
                                    }

                                    vms.push(output_vm);

                                    Ok(vm_index)
                                }
                                Err(e) => Err(Box::new(e)),
                            }
                        }
                    },
                };

                let Ok(created_resource_node_index) = created_resource_node_index else {
                    //TODO: Handle failed resource creation
                    log::error!("Failed to create a resource {created_resource_node_index:?}");

                    continue;
                };

                for neighbor_index in graph.neighbors(node_index) {
                    if !parents.contains_key(&neighbor_index) {
                        queue.push_back(neighbor_index);
                    }

                    parents
                        .entry(neighbor_index)
                        .or_insert_with(Vec::new)
                        .push(created_resource_node_index);
                }
            }
        }

        resource_graph.extend_with_edges(&edges);

        log::info!("Created graph {}", Dot::new(&resource_graph));

        (resource_graph, vms, ecr)
    }

    pub async fn destroy(&self, graph: &Graph<Node, String>) {
        log::info!("Graph to delete {}", Dot::new(&graph));

        let mut parents: HashMap<NodeIndex, Vec<NodeIndex>> = HashMap::new();

        // Remove resources
        let mut queue_to_traverse: VecDeque<NodeIndex> = VecDeque::new();
        let root_index = graph.from_index(0);
        for node_index in graph.neighbors(root_index) {
            queue_to_traverse.push_back(node_index);

            parents
                .entry(node_index)
                .or_insert_with(Vec::new)
                .push(root_index);
        }

        // Prepare queue to destroy
        while let Some(node_index) = queue_to_traverse.pop_front() {
            if let Some(_elem) = graph.node_weight(node_index) {
                for neighbor_index in graph.neighbors(node_index) {
                    if !parents.contains_key(&neighbor_index) {
                        queue_to_traverse.push_back(neighbor_index);
                    }

                    parents
                        .entry(neighbor_index)
                        .or_insert_with(Vec::new)
                        .push(node_index);
                }
            }
        }

        let result = Self::kahn_traverse(graph);

        // Destroying resources in reversed order
        for node_index in result.iter().rev() {
            let parent_node_indexes = match parents.get(node_index) {
                Some(parent_node_indexes) => parent_node_indexes.clone(),
                None => Vec::new(),
            };
            let parent_nodes = parent_node_indexes
                .iter()
                .filter_map(|x| graph.node_weight(*x))
                .collect();

            match &graph[*node_index] {
                Node::Root => (),
                Node::Resource(resource_type) => match resource_type {
                    ResourceType::HostedZone(resource) => {
                        let manager = HostedZoneManager {
                            client: &self.route53_client,
                        };
                        if let Err(e) = manager.destroy(resource, parent_nodes).await {
                            log::error!("Failed to destroy {resource:?}: {e}");
                        } else {
                            log::info!("Destroyed {resource:?}");
                        }
                    }
                    ResourceType::DnsRecord(resource) => {
                        let manager = DnsRecordManager {
                            client: &self.route53_client,
                        };
                        if let Err(e) = manager.destroy(resource, parent_nodes).await {
                            log::error!("Failed to destroy {resource:?}: {e}");
                        } else {
                            log::info!("Destroyed {resource:?}");
                        }
                    }
                    ResourceType::Vpc(resource) => {
                        let manager = VpcManager {
                            client: &self.ec2_client,
                        };
                        if let Err(e) = manager.destroy(resource, parent_nodes).await {
                            log::error!("Failed to destroy Vpc {resource:?}: {e}");
                        } else {
                            log::info!("Destroyed Vpc {resource:?}");
                        }
                    }
                    ResourceType::InternetGateway(resource) => {
                        let manager = InternetGatewayManager {
                            client: &self.ec2_client,
                        };
                        if let Err(e) = manager.destroy(resource, parent_nodes).await {
                            log::error!("Failed to destroy InternetGateway {resource:?}: {e}");
                        } else {
                            log::info!("Destroyed InternetGateway {resource:?}");
                        }
                    }
                    ResourceType::RouteTable(resource) => {
                        let manager = RouteTableManager {
                            client: &self.ec2_client,
                        };
                        if let Err(e) = manager.destroy(resource, parent_nodes).await {
                            log::error!("Failed to destroy RouteTable {resource:?}: {e}");
                        } else {
                            log::info!("Destroyed RouteTable {resource:?}");
                        }
                    }
                    ResourceType::Subnet(resource) => {
                        let manager = SubnetManager {
                            client: &self.ec2_client,
                        };
                        if let Err(e) = manager.destroy(resource, parent_nodes).await {
                            log::error!("Failed to destroy Subnet {resource:?}: {e}");
                        } else {
                            log::info!("Destroyed Subnet {resource:?}");
                        }
                    }
                    ResourceType::SecurityGroup(resource) => {
                        let manager = SecurityGroupManager {
                            client: &self.ec2_client,
                        };
                        if let Err(e) = manager.destroy(resource, parent_nodes).await {
                            log::error!("Failed to destroy SecurityGroup {resource:?}: {e}");
                        } else {
                            log::info!("Destroyed SecurityGroup {resource:?}");
                        }
                    }
                    ResourceType::InstanceRole(resource) => {
                        let manager = InstanceRoleManager {
                            client: &self.iam_client,
                        };
                        if let Err(e) = manager.destroy(resource, parent_nodes).await {
                            log::error!("Failed to destroy InstanceRole {resource:?}: {e}");
                        } else {
                            log::info!("Destroyed InstanceRole {resource:?}");
                        }
                    }
                    ResourceType::InstanceProfile(resource) => {
                        let manager = InstanceProfileManager {
                            client: &self.iam_client,
                        };
                        if let Err(e) = manager.destroy(resource, parent_nodes).await {
                            log::error!("Failed to destroy InstanceProfile {resource:?}: {e}");
                        } else {
                            log::info!("Destroyed InstanceProfile {resource:?}");
                        }
                    }
                    ResourceType::Ecr(resource) => {
                        let manager = EcrManager {
                            client: &self.ecr_client,
                        };
                        if let Err(e) = manager.destroy(resource, parent_nodes).await {
                            log::error!("Failed to destroy Ecr {resource:?}: {e}");
                        } else {
                            log::info!("Destroyed Ecr {resource:?}");
                        }
                    }
                    ResourceType::Vm(resource) => {
                        let manager = VmManager {
                            client: &self.ec2_client,
                        };
                        if let Err(e) = manager.destroy(resource, parent_nodes).await {
                            log::error!("Failed to destroy Vm {resource:?}: {e}");
                        } else {
                            log::info!("Destroyed Vm {resource:?}");
                        }
                    }
                    ResourceType::None => {
                        log::error!("Unexpected case ResourceType::None");
                    }
                },
            }
        }
    }

    /// Kahn's Algorithm Implementation
    fn kahn_traverse<T>(graph: &Graph<T, String>) -> Vec<NodeIndex> {
        // 1. Calculate the in-degree for each node.
        let mut in_degrees: Vec<usize> = graph
            .node_indices()
            .map(|i| graph.neighbors_directed(i, Incoming).count())
            .collect();

        // 2. Initialize a queue with all nodes having an in-degree of 0.
        let mut queue: VecDeque<NodeIndex> = graph
            .node_indices()
            .filter(|&i| in_degrees[i.index()] == 0)
            .collect();

        let mut result = Vec::new();

        // 3. Process the queue.
        while let Some(node) = queue.pop_front() {
            result.push(node);

            // For each neighbor of the processed node, decrement its in-degree.
            for neighbor in graph.neighbors_directed(node, Outgoing) {
                let neighbor_idx = neighbor.index();
                in_degrees[neighbor_idx] -= 1;

                // If a neighbor's in-degree becomes 0, add it to the queue.
                if in_degrees[neighbor_idx] == 0 {
                    queue.push_back(neighbor);
                }
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {}
