use petgraph::{Incoming, Outgoing};

use petgraph::visit::NodeIndexable;
use std::collections::{HashMap, VecDeque};

use petgraph::Graph;
use petgraph::dot::Dot;
use petgraph::graph::NodeIndex;

use crate::aws::client;
use crate::aws::types;
use crate::infra::resource::{
    DnsRecordManager, DnsRecordSpec, Ecr, EcrManager, EcrSpec, HostedZoneManager, HostedZoneSpec,
    InboundRule, InstanceProfileManager, InstanceProfileSpec, InstanceRoleManager,
    InstanceRoleSpec, InternetGatewayManager, InternetGatewaySpec, Manager, Node, ResourceSpecType,
    ResourceType, RouteTableManager, RouteTableSpec, SecurityGroupManager, SecurityGroupSpec,
    SpecNode, SubnetManager, SubnetSpec, Vm, VmManager, VmSpec, VpcManager, VpcSpec,
};

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

    #[cfg(test)]
    pub fn new_with_clients(
        ec2_client: client::Ec2,
        iam_client: client::IAM,
        ecr_client: client::ECR,
        route53_client: client::Route53,
    ) -> Self {
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

        let root_node = graph.from_index(0);
        for node_index in graph.neighbors(root_node) {
            parents
                .entry(node_index)
                .or_insert_with(Vec::new)
                .push(root_index);
        }

        let mut ecr: Option<Ecr> = None;
        let mut vms: Vec<Vm> = Vec::new();

        let result = Self::kahn_traverse(graph);

        for node_index in &result {
            let parent_node_indexes = match parents.get(node_index) {
                Some(parent_node_indexes) => parent_node_indexes.clone(),
                None => Vec::new(),
            };
            let parent_nodes = parent_node_indexes
                .iter()
                .filter_map(|x| resource_graph.node_weight(*x))
                .collect();

            let created_resource_node_index = match &graph[*node_index] {
                SpecNode::Root => Ok(root_index),
                SpecNode::Resource(resource_type) => match resource_type {
                    ResourceSpecType::HostedZone(resource) => {
                        let manager = HostedZoneManager {
                            client: &self.route53_client,
                        };
                        let output_resource = manager.create(resource, parent_nodes).await;

                        match output_resource {
                            Ok(output_resource) => {
                                log::info!(
                                    "Deployed {output_resource:?}, parents - {parent_node_indexes:?}",
                                );

                                let node =
                                    Node::Resource(ResourceType::HostedZone(output_resource));
                                let resource_index = resource_graph.add_node(node.clone());

                                for parent_node_index in parent_node_indexes {
                                    edges.push((parent_node_index, resource_index, String::new()));
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
                                    "Deployed {output_resource:?}, parents - {parent_node_indexes:?}",
                                );

                                let node = Node::Resource(ResourceType::DnsRecord(output_resource));
                                let resource_index = resource_graph.add_node(node.clone());

                                for parent_node_index in parent_node_indexes {
                                    edges.push((parent_node_index, resource_index, String::new()));
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
                                    "Deployed {output_vpc:?}, parents - {parent_node_indexes:?}",
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
                                    "Deployed {output_igw:?}, parents - {parent_node_indexes:?}",
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
                                    "Deployed {output_route_table:?}, parents - {parent_node_indexes:?}",
                                );

                                let node =
                                    Node::Resource(ResourceType::RouteTable(output_route_table));
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
                                    "Deployed {output_subnet:?}, parents - {parent_node_indexes:?}",
                                );

                                let node = Node::Resource(ResourceType::Subnet(output_subnet));
                                let subnet_index = resource_graph.add_node(node.clone());

                                for parent_node_index in parent_node_indexes {
                                    edges.push((parent_node_index, subnet_index, String::new()));
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
                        let output_security_group = manager.create(resource, parent_nodes).await;

                        match output_security_group {
                            Ok(output_security_group) => {
                                log::info!(
                                    "Deployed {output_security_group:?}, parents - {parent_node_indexes:?}",
                                );

                                let node = Node::Resource(ResourceType::SecurityGroup(
                                    output_security_group,
                                ));
                                let security_group_index = resource_graph.add_node(node.clone());

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
                                    "Deployed {output_instance_role:?}, parents - {parent_node_indexes:?}",
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
                                    "Deployed {output_resource:?}, parents - {parent_node_indexes:?}",
                                );

                                let node =
                                    Node::Resource(ResourceType::InstanceProfile(output_resource));
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
                                    "Deployed {output_resource:?}, parents - {parent_node_indexes:?}",
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
                                    "Deployed {output_vm:?}, parents - {parent_node_indexes:?}",
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

            for neighbor_index in graph.neighbors(*node_index) {
                parents
                    .entry(neighbor_index)
                    .or_insert_with(Vec::new)
                    .push(created_resource_node_index);
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
mod tests {
    use super::*;
    use crate::aws::types::InstanceType;
    use crate::infra::resource::{ResourceSpecType, SpecNode};
    use mockall::predicate::eq;

    #[test]
    fn test_get_spec_graph_with_one_instance_no_domain() {
        // Arrange
        let number_of_instances = 1;
        let instance_type = InstanceType::T2Micro;
        let domain_name = None;

        // Act
        let graph = GraphManager::get_spec_graph(number_of_instances, &instance_type, domain_name);

        // Assert
        let number_of_nodes = 9 + number_of_instances;
        let number_of_edges = 10 + 4 * number_of_instances;
        assert_eq!(graph.node_count(), number_of_nodes as usize);
        assert_eq!(graph.edge_count(), number_of_edges as usize);

        let vm_nodes_count = graph
            .raw_nodes()
            .iter()
            .filter(|node| matches!(&node.weight, SpecNode::Resource(ResourceSpecType::Vm(_))))
            .count();
        assert_eq!(vm_nodes_count, number_of_instances as usize);
    }

    #[test]
    fn test_get_spec_graph_with_multiple_instances_no_domain() {
        // Arrange
        let number_of_instances = 3;
        let instance_type = InstanceType::T2Micro;
        let domain_name = None;

        // Act
        let graph = GraphManager::get_spec_graph(number_of_instances, &instance_type, domain_name);

        // Assert
        let number_of_nodes = 9 + number_of_instances;
        let number_of_edges = 10 + 4 * number_of_instances;
        assert_eq!(graph.node_count(), number_of_nodes as usize);
        assert_eq!(graph.edge_count(), number_of_edges as usize);

        let vm_nodes_count = graph
            .raw_nodes()
            .iter()
            .filter(|node| matches!(&node.weight, SpecNode::Resource(ResourceSpecType::Vm(_))))
            .count();
        assert_eq!(vm_nodes_count, number_of_instances as usize);
    }

    #[test]
    fn test_get_spec_graph_with_one_instance_and_domain() {
        // Arrange
        let number_of_instances = 1;
        let instance_type = InstanceType::T2Micro;
        let domain_name = Some(String::from("example.com"));

        // Act
        let graph =
            GraphManager::get_spec_graph(number_of_instances, &instance_type, domain_name.clone());

        // Assert
        let number_of_nodes = 10 + 2 * number_of_instances;
        let number_of_edges = 11 + 6 * number_of_instances;
        assert_eq!(graph.node_count(), number_of_nodes as usize);
        assert_eq!(graph.edge_count(), number_of_edges as usize);

        let vm_nodes_count = graph
            .raw_nodes()
            .iter()
            .filter(|node| matches!(&node.weight, SpecNode::Resource(ResourceSpecType::Vm(_))))
            .count();
        assert_eq!(vm_nodes_count, number_of_instances as usize);

        let hosted_zone_nodes_count = graph
            .raw_nodes()
            .iter()
            .filter(|node| {
                matches!(
                    &node.weight,
                    SpecNode::Resource(ResourceSpecType::HostedZone(_))
                )
            })
            .count();
        assert_eq!(hosted_zone_nodes_count, 1);

        let dns_record_nodes_count = graph
            .raw_nodes()
            .iter()
            .filter(|node| {
                matches!(
                    &node.weight,
                    SpecNode::Resource(ResourceSpecType::DnsRecord(_))
                )
            })
            .count();
        assert_eq!(dns_record_nodes_count, number_of_instances as usize);
    }

    #[test]
    fn test_get_spec_graph_with_multiple_instances_and_domain() {
        // Arrange
        let number_of_instances = 3;
        let instance_type = InstanceType::T2Micro;
        let domain_name = Some(String::from("example.com"));

        // Act
        let graph =
            GraphManager::get_spec_graph(number_of_instances, &instance_type, domain_name.clone());

        // Assert
        let number_of_nodes = 10 + 2 * number_of_instances;
        let number_of_edges = 11 + 6 * number_of_instances;
        assert_eq!(graph.node_count(), number_of_nodes as usize);
        assert_eq!(graph.edge_count(), number_of_edges as usize);

        let vm_nodes_count = graph
            .raw_nodes()
            .iter()
            .filter(|node| matches!(&node.weight, SpecNode::Resource(ResourceSpecType::Vm(_))))
            .count();
        assert_eq!(vm_nodes_count, number_of_instances as usize);

        let hosted_zone_nodes_count = graph
            .raw_nodes()
            .iter()
            .filter(|node| {
                matches!(
                    &node.weight,
                    SpecNode::Resource(ResourceSpecType::HostedZone(_))
                )
            })
            .count();
        assert_eq!(hosted_zone_nodes_count, 1);

        let dns_record_nodes_count = graph
            .raw_nodes()
            .iter()
            .filter(|node| {
                matches!(
                    &node.weight,
                    SpecNode::Resource(ResourceSpecType::DnsRecord(_))
                )
            })
            .count();
        assert_eq!(dns_record_nodes_count, number_of_instances as usize);
    }

    #[tokio::test]
    async fn test_deploy_with_one_instance_no_domain() {
        // Arrange
        let number_of_instances = 1;
        let instance_type = InstanceType::T2Micro;
        let domain_name = None;

        let spec_graph =
            GraphManager::get_spec_graph(number_of_instances, &instance_type, domain_name);

        let mut ec2_client_mock = client::Ec2::default();
        let mut iam_client_mock = client::IAM::default();
        let mut ecr_client_mock = client::ECR::default();
        let route53_client_mock = client::Route53::default();

        // Expectations for resource creation
        ec2_client_mock
            .expect_create_vpc()
            .with(eq(String::from("10.0.0.0/16")), eq(String::from("vpc-1")))
            .return_once(|_, _| Ok(String::from("vpc-id-1")));

        iam_client_mock
            .expect_create_instance_iam_role()
            .with(
                eq(String::from("instance-role-1")),
                eq(String::from(
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
                )),
                eq(vec![String::from(
                    "arn:aws:iam::aws:policy/AmazonEC2ContainerRegistryReadOnly",
                )]),
            )
            .return_once(|_, _, _| Ok(()));

        ecr_client_mock
            .expect_create_repository()
            .with(eq(String::from("ecr_1")))
            .return_once(|_| Ok((String::from("ecr-id-1"), String::from("ecr-uri-1/foo"))));

        ec2_client_mock
            .expect_create_internet_gateway()
            .with(eq(String::from("vpc-id-1")))
            .return_once(|_| Ok(String::from("igw-id-1")));

        ec2_client_mock
            .expect_create_route_table()
            .with(eq(String::from("vpc-id-1")))
            .return_once(|_| Ok(String::from("rt-id-1")));

        ec2_client_mock
            .expect_add_public_route()
            .with(eq(String::from("rt-id-1")), eq(String::from("igw-id-1")))
            .return_once(|_, _| Ok(()));

        ec2_client_mock
            .expect_create_subnet()
            .with(
                eq(String::from("vpc-id-1")),
                eq(String::from("10.0.1.0/24")),
                eq(String::from("us-west-2a")),
                eq(String::from("vpc-1-subnet")),
            )
            .return_once(|_, _, _, _| Ok(String::from("subnet-id-1")));

        ec2_client_mock
            .expect_enable_auto_assign_ip_addresses_for_subnet()
            .with(eq(String::from("subnet-id-1")))
            .return_once(|_| Ok(()));

        ec2_client_mock
            .expect_associate_route_table_with_subnet()
            .with(eq(String::from("rt-id-1")), eq(String::from("subnet-id-1")))
            .return_once(|_, _| Ok(()));

        ec2_client_mock
            .expect_create_security_group()
            .with(
                eq(String::from("vpc-id-1")),
                eq(String::from("vpc-1-security-group")),
                eq(String::from("No description")),
            )
            .return_once(|_, _, _| Ok(String::from("sg-id-1")));

        ec2_client_mock
            .expect_allow_inbound_traffic_for_security_group()
            .with(
                eq(String::from("sg-id-1")),
                eq(String::from("tcp")),
                eq(80),
                eq(String::from("0.0.0.0/0")),
            )
            .return_once(|_, _, _, _| Ok(()));
        ec2_client_mock
            .expect_allow_inbound_traffic_for_security_group()
            .with(
                eq(String::from("sg-id-1")),
                eq(String::from("tcp")),
                eq(31888),
                eq(String::from("0.0.0.0/0")),
            )
            .return_once(|_, _, _, _| Ok(()));
        ec2_client_mock
            .expect_allow_inbound_traffic_for_security_group()
            .with(
                eq(String::from("sg-id-1")),
                eq(String::from("tcp")),
                eq(22),
                eq(String::from("0.0.0.0/0")),
            )
            .return_once(|_, _, _, _| Ok(()));

        iam_client_mock
            .expect_create_instance_profile()
            .with(
                eq(String::from("instance_profile_1")),
                eq(vec![String::from("instance-role-1")]),
            )
            .return_once(|_, _| Ok(()));

        ec2_client_mock
            .expect_run_instances()
            .return_once(|_, _, _, _, _, _| {
                let instance = aws_sdk_ec2::types::Instance::builder()
                    .instance_id("vm-id-1")
                    .build();
                Ok(
                    aws_sdk_ec2::operation::run_instances::RunInstancesOutput::builder()
                        .instances(instance)
                        .build(),
                )
            });

        ec2_client_mock
            .expect_describe_instances()
            .with(eq(String::from("vm-id-1")))
            .return_once(|_| {
                Ok(aws_sdk_ec2::types::Instance::builder()
                    .public_ip_address("1.2.3.4")
                    .build())
            });

        let graph_manager = GraphManager::new_with_clients(
            ec2_client_mock,
            iam_client_mock,
            ecr_client_mock,
            route53_client_mock,
        );

        // Act
        let (resource_graph, vms, ecr) = graph_manager.deploy(&spec_graph).await;

        // Assert
        assert_eq!(resource_graph.node_count(), 10); // root + 9 resources
        assert_eq!(resource_graph.edge_count(), 17);

        assert_eq!(
            vms,
            vec![Vm {
                id: String::from("vm-id-1"),
                public_ip: String::from("1.2.3.4"),
                ami: String::from("ami-04dd23e62ed049936"),
                instance_type: InstanceType::T2Micro,
                user_data: String::from(
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
        
aws ecr get-login-password --region us-west-2 | podman login --username AWS --password-stdin ecr-uri-1"#
                )
            }]
        );

        assert_eq!(
            ecr.expect("Failed to get ECR"),
            Ecr {
                id: String::from("ecr-id-1"),
                name: String::from("ecr_1"),
                uri: String::from("ecr-uri-1/foo"),
            }
        );
    }

    #[tokio::test]
    async fn test_deploy_empty_graph() {
        // Arrange
        let spec_graph = Graph::<SpecNode, String>::new();

        let ec2_client_mock = client::Ec2::default();
        let iam_client_mock = client::IAM::default();
        let ecr_client_mock = client::ECR::default();
        let route53_client_mock = client::Route53::default();

        let graph_manager = GraphManager::new_with_clients(
            ec2_client_mock,
            iam_client_mock,
            ecr_client_mock,
            route53_client_mock,
        );

        // Act
        let (resource_graph, vms, ecr) = graph_manager.deploy(&spec_graph).await;

        // Assert
        assert_eq!(resource_graph.node_count(), 1); // Just the root node
        assert!(
            resource_graph
                .node_weights()
                .any(|w| matches!(w, Node::Root))
        );
        assert_eq!(resource_graph.edge_count(), 0);
        assert!(vms.is_empty());
        assert!(ecr.is_none());
    }

    #[tokio::test]
    async fn test_deploy_resource_creation_fails() {
        // Arrange
        let number_of_instances = 1;
        let instance_type = InstanceType::T2Micro;
        let domain_name = None;

        let spec_graph =
            GraphManager::get_spec_graph(number_of_instances, &instance_type, domain_name);

        let mut ec2_client_mock = client::Ec2::default();
        let mut iam_client_mock = client::IAM::default();
        let mut ecr_client_mock = client::ECR::default();
        let route53_client_mock = client::Route53::default();

        // Expectations for resource creation
        iam_client_mock
            .expect_create_instance_iam_role()
            .with(
                eq(String::from("instance-role-1")),
                eq(String::from(
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
                )),
                eq(vec![String::from(
                    "arn:aws:iam::aws:policy/AmazonEC2ContainerRegistryReadOnly",
                )]),
            )
            .return_once(|_, _, _| Ok(()));

        iam_client_mock
            .expect_create_instance_profile()
            .with(
                eq(String::from("instance_profile_1")),
                eq(vec![String::from("instance-role-1")]),
            )
            .return_once(|_, _| Ok(()));

        ecr_client_mock
            .expect_create_repository()
            .with(eq(String::from("ecr_1")))
            .return_once(|_| Ok((String::from("ecr-id-1"), String::from("ecr-uri-1/foo"))));

        // Simulate VPC creation failure
        ec2_client_mock
            .expect_create_vpc()
            .with(eq(String::from("10.0.0.0/16")), eq(String::from("vpc-1")))
            .return_once(|_, _| Err("VPC creation failed".into()));

        let graph_manager = GraphManager::new_with_clients(
            ec2_client_mock,
            iam_client_mock,
            ecr_client_mock,
            route53_client_mock,
        );

        // Act
        let (resource_graph, vms, ecr) = graph_manager.deploy(&spec_graph).await;

        // Assert
        // 1 root + ECR + InstanceRole + InstanceProfile
        assert_eq!(resource_graph.node_count(), 4);
        assert_eq!(resource_graph.edge_count(), 5);
        assert!(vms.is_empty());
        assert!(ecr.is_some());

        let ecr_node_exists = resource_graph
            .node_weights()
            .any(|w| matches!(w, Node::Resource(ResourceType::Ecr(_))));
        assert!(ecr_node_exists);

        let instance_role_node_exists = resource_graph
            .node_weights()
            .any(|w| matches!(w, Node::Resource(ResourceType::InstanceRole(_))));
        assert!(instance_role_node_exists);

        let instance_profile_node_exists = resource_graph
            .node_weights()
            .any(|w| matches!(w, Node::Resource(ResourceType::InstanceProfile(_))));
        assert!(instance_profile_node_exists);

        let vpc_node_exists = resource_graph
            .node_weights()
            .any(|w| matches!(w, Node::Resource(ResourceType::Vpc(_))));
        assert!(!vpc_node_exists);
    }
}
