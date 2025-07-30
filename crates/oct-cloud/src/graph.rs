use aws_sdk_ec2::types::InstanceStateName;
use base64::{Engine as _, engine::general_purpose};
use petgraph::visit::NodeIndexable;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

use petgraph::Graph;
use petgraph::dot::Dot;
use petgraph::graph::NodeIndex;

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
    ) -> impl std::future::Future<Output = Result<(), Box<dyn std::error::Error>>> + Send;
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
    igw_id: String,
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

        let igw_id = self.client.create_internet_gateway(vpc_id.clone()).await?;

        let default_security_group_id = self
            .client
            .get_default_security_group_id(vpc_id.clone())
            .await?;

        let inbound_rules = vec![
            (String::from("0.0.0.0/0"), String::from("tcp"), 80),
            (String::from("0.0.0.0/0"), String::from("tcp"), 31888),
            (String::from("0.0.0.0/0"), String::from("tcp"), 22),
        ];

        for (cidr_block, protocol, port) in inbound_rules {
            self.client
                .allow_inbound_traffic_for_security_group(
                    default_security_group_id.clone(),
                    protocol,
                    port,
                    cidr_block,
                )
                .await?;
        }

        Ok(Vpc {
            id: vpc_id,
            region: input.region.clone(),
            cidr_block: input.cidr_block.clone(),
            name: input.name.clone(),
            igw_id,
        })
    }

    async fn destroy(&self, input: &'_ Vpc) -> Result<(), Box<dyn std::error::Error>> {
        self.client
            .delete_internet_gateway(input.igw_id.clone(), input.id.clone())
            .await?;

        self.client.delete_vpc(input.id.clone()).await
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

    route_table_id: String,
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
            Err("Unexpected parent")
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

        let route_table_id = self.client.create_route_table(vpc.id.clone()).await?;

        self.client
            .associate_route_table_with_subnet(route_table_id.clone(), subnet_id.clone())
            .await?;

        self.client
            .add_public_route(route_table_id.clone(), vpc.igw_id.clone())
            .await?;

        Ok(Subnet {
            id: subnet_id,
            name: input.name.clone(),
            cidr_block: input.cidr_block.clone(),
            availability_zone: input.availability_zone.clone(),
            route_table_id: route_table_id.clone(),
        })
    }

    async fn destroy(&self, input: &'_ Subnet) -> Result<(), Box<dyn std::error::Error>> {
        self.client
            .disassociate_route_table_with_subnet(input.route_table_id.clone(), input.id.clone())
            .await?;

        self.client
            .delete_route_table(input.route_table_id.clone())
            .await?;

        self.client.delete_subnet(input.id.clone()).await
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
            Err("Unexpected parent")
        };

        let user_data_base64 = general_purpose::STANDARD.encode(input.user_data.clone());

        let response = self
            .client
            .run_instances(
                input.instance_type.clone(),
                input.ami.clone(),
                user_data_base64,
                None,
                subnet_id?,
                None,
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
            user_data: input.user_data.clone(),
        })
    }

    async fn destroy(&self, input: &'_ Vm) -> Result<(), Box<dyn std::error::Error>> {
        self.client.terminate_instance(input.id.clone()).await?;

        self.is_terminated(input.id.clone()).await
    }
}

#[derive(Debug)]
pub enum ResourceSpecType {
    Vpc(VpcSpec),
    Subnet(SubnetSpec),
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
                ResourceSpecType::Vpc(resource) => {
                    write!(f, "spec {}", resource.name)
                }
                ResourceSpecType::Subnet(resource) => {
                    write!(f, "spec {}", resource.cidr_block)
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

    Vpc(Vpc),
    Subnet(Subnet),
    Vm(Vm),
}

impl ResourceType {
    fn name(&self) -> String {
        match self {
            ResourceType::Vpc(vpc) => format!("vpc.{}", vpc.name),
            ResourceType::Subnet(subnet) => format!("subnet.{}", subnet.name),
            ResourceType::Vm(vm) => format!("vm.{}", vm.id),
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
                ResourceType::Vpc(resource) => {
                    write!(f, "cloud {}", resource.name)
                }
                ResourceType::Subnet(resource) => {
                    write!(f, "cloud {}", resource.cidr_block)
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

        let mut queue: VecDeque<(NodeIndex, NodeIndex)> = VecDeque::new();
        let root_node = graph.from_index(0);
        for node_index in graph.neighbors(root_node) {
            queue.push_back((node_index, root_node));
        }

        while let Some((node_index, parent_node_index)) = queue.pop_front() {
            let parent_node = graph
                .node_weight(parent_node_index)
                .expect("Failed to get parent");

            let dependencies = match parent_node {
                Node::Root => Vec::new(),
                Node::Resource(parent_resource_type) => vec![parent_resource_type.name()],
            };

            if let Some(Node::Resource(resource_type)) = graph.node_weight(node_index) {
                log::info!("Add to state {resource_type:?}");

                resource_states.push(ResourceState {
                    name: resource_type.name(),
                    resource: resource_type.clone(),
                    dependencies,
                });

                for neighbor_node_index in graph.neighbors(node_index) {
                    queue.push_back((neighbor_node_index, node_index));
                }
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

            let dependency = resource_state.dependencies.first();

            match dependency {
                None => edges.push((root, *resource, String::new())),
                Some(dependency_name) => {
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

        Self { ec2_client }
    }

    pub fn get_spec_graph(
        number_of_instances: u32,
        instance_type: &types::InstanceType,
    ) -> Graph<SpecNode, String> {
        let mut deps = Graph::<SpecNode, String>::new();
        let root = deps.add_node(SpecNode::Root);

        let vpc_1 = deps.add_node(SpecNode::Resource(ResourceSpecType::Vpc(VpcSpec {
            region: String::from("us-west-2"),
            cidr_block: String::from("10.0.0.0/16"),
            name: String::from("vpc-1"),
        })));

        let subnet_1 = deps.add_node(SpecNode::Resource(ResourceSpecType::Subnet(SubnetSpec {
            name: String::from("vpc-1-subnet"),
            cidr_block: String::from("10.0.1.0/24"),
            availability_zone: String::from("us-west-2a"),
        })));

        let user_data = String::from(
            r#"#!/bin/bash
        set -e
        sudo apt update
        sudo apt -y install podman
        sudo systemctl start podman

        curl \
            --output /home/ubuntu/oct-ctl \
            -L \
            https://github.com/opencloudtool/opencloudtool/releases/download/tip/oct-ctl \
            && sudo chmod +x /home/ubuntu/oct-ctl \
            && /home/ubuntu/oct-ctl &
        "#,
        );

        // TODO: Add instance profile with instance role
        let instances = (0..number_of_instances)
            .collect::<Vec<u32>>()
            .into_iter()
            .map(|_| {
                deps.add_node(SpecNode::Resource(ResourceSpecType::Vm(VmSpec {
                    instance_type: instance_type.clone(),
                    ami: String::from("ami-04dd23e62ed049936"),
                    user_data: user_data.clone(),
                })))
            });

        let mut edges = vec![
            (root, vpc_1, String::new()),
            (vpc_1, subnet_1, String::new()),
        ];
        for instance in instances {
            edges.push((subnet_1, instance, String::new()));
        }

        deps.extend_with_edges(&edges);

        deps
    }

    /// Deploy spec graph
    ///
    /// Temporarily also returns a list of VMs to be used for user
    /// services deployment
    /// TODO: Implement graph manager
    pub async fn deploy(&self, graph: &Graph<SpecNode, String>) -> (Graph<Node, String>, Vec<Vm>) {
        let mut resource_graph = Graph::<Node, String>::new();
        let mut edges = vec![];
        let root = resource_graph.add_node(Node::Root);

        let mut queue: VecDeque<(NodeIndex, NodeIndex)> = VecDeque::new();
        let root_node = graph.from_index(0);
        for node_index in graph.neighbors(root_node) {
            queue.push_back((node_index, root));
        }

        let mut vms: Vec<Vm> = Vec::new();

        while let Some((node_index, parent_node_index)) = queue.pop_front() {
            let parent_node = resource_graph
                .node_weight(parent_node_index)
                .expect("Failed to get parent");

            if let Some(elem) = graph.node_weight(node_index) {
                let created_resource_node_index = match elem {
                    SpecNode::Root => Some(resource_graph.add_node(Node::Root)),
                    SpecNode::Resource(resource_type) => match resource_type {
                        ResourceSpecType::Vpc(resource) => {
                            let manager = VpcManager {
                                client: &self.ec2_client,
                            };
                            let output_vpc = manager.create(resource, vec![parent_node]).await;

                            match output_vpc {
                                Ok(output_vpc) => {
                                    log::info!(
                                        "Deployed {output_vpc:?}, parent - {parent_node_index:?}"
                                    );

                                    let node = Node::Resource(ResourceType::Vpc(output_vpc));
                                    let vpc_index = resource_graph.add_node(node.clone());

                                    edges.push((parent_node_index, vpc_index, String::new()));

                                    Some(vpc_index)
                                }
                                Err(_) => None,
                            }
                        }
                        ResourceSpecType::Subnet(resource) => {
                            let manager = SubnetManager {
                                client: &self.ec2_client,
                            };
                            let output_subnet = manager.create(resource, vec![parent_node]).await;

                            match output_subnet {
                                Ok(output_subnet) => {
                                    log::info!(
                                        "Deployed {output_subnet:?}, parent - {parent_node_index:?}"
                                    );

                                    let node = Node::Resource(ResourceType::Subnet(output_subnet));
                                    let subnet_index = resource_graph.add_node(node.clone());

                                    edges.push((parent_node_index, subnet_index, String::new()));

                                    Some(subnet_index)
                                }
                                Err(_) => None,
                            }
                        }
                        ResourceSpecType::Vm(resource) => {
                            let manager = VmManager {
                                client: &self.ec2_client,
                            };
                            let output_vm = manager.create(resource, vec![parent_node]).await;

                            match output_vm {
                                Ok(output_vm) => {
                                    log::info!(
                                        "Deployed {output_vm:?}, parent - {parent_node_index:?}"
                                    );

                                    let node = Node::Resource(ResourceType::Vm(output_vm.clone()));
                                    let vm_index = resource_graph.add_node(node.clone());

                                    edges.push((parent_node_index, vm_index, String::new()));

                                    vms.push(output_vm);

                                    Some(vm_index)
                                }
                                Err(_) => None,
                            }
                        }
                    },
                };

                let Some(created_resource_node_index) = created_resource_node_index else {
                    //TODO: Handle failed resource creation
                    log::error!("Failed to create a resource");

                    continue;
                };

                for neighbor_index in graph.neighbors(node_index) {
                    queue.push_back((neighbor_index, created_resource_node_index));
                }
            }
        }

        resource_graph.extend_with_edges(&edges);

        log::info!("Created graph {}", Dot::new(&resource_graph));

        (resource_graph, vms)
    }

    pub async fn destroy(&self, graph: &Graph<Node, String>) {
        log::info!("Graph to delete {}", Dot::new(&graph));

        // Remove resources
        let mut queue_to_destroy: VecDeque<NodeIndex> = VecDeque::new();
        let mut queue_to_traverse: VecDeque<NodeIndex> = VecDeque::new();
        let root_node = graph.from_index(0);
        for node_index in graph.neighbors(root_node) {
            queue_to_traverse.push_back(node_index);
        }

        // Prepare queue to destroy
        while let Some(node_index) = queue_to_traverse.pop_front() {
            if let Some(_elem) = graph.node_weight(node_index) {
                queue_to_destroy.push_back(node_index);

                for neighbor_index in graph.neighbors(node_index) {
                    queue_to_traverse.push_back(neighbor_index);
                }
            }
        }

        // Destroy resources from back
        while let Some(node_index) = queue_to_destroy.pop_back() {
            if let Some(elem) = graph.node_weight(node_index) {
                match elem {
                    Node::Root => {}
                    Node::Resource(resource_type) => match resource_type {
                        ResourceType::Vpc(resource) => {
                            let manager = VpcManager {
                                client: &self.ec2_client,
                            };
                            let _ = manager.destroy(resource).await;

                            log::info!("Destroyed {resource:?}");
                        }
                        ResourceType::Subnet(resource) => {
                            let manager = SubnetManager {
                                client: &self.ec2_client,
                            };
                            let _ = manager.destroy(resource).await;

                            log::info!("Destroyed {resource:?}");
                        }
                        ResourceType::Vm(resource) => {
                            let manager = VmManager {
                                client: &self.ec2_client,
                            };
                            let _ = manager.destroy(resource).await;

                            log::info!("Destroyed {resource:?}");
                        }
                        ResourceType::None => {
                            panic!("Unexpected case ResourceType::None")
                        }
                    },
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {}
