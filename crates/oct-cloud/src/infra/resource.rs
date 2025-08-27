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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug)]
pub struct DnsRecordSpec {
    pub record_type: types::RecordType,
    pub ttl: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug)]
pub struct InternetGatewaySpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug)]
pub struct RouteTableSpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug)]
pub struct SubnetSpec {
    pub name: String,
    pub cidr_block: String,
    pub availability_zone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundRule {
    pub protocol: String,
    pub port: i32,
    pub cidr_block: String,
}

#[derive(Debug)]
pub struct SecurityGroupSpec {
    pub name: String,
    pub inbound_rules: Vec<InboundRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug)]
pub struct InstanceProfileSpec {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug)]
pub struct EcrSpec {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

            log::info!(
                "VM is not terminated yet... 
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
