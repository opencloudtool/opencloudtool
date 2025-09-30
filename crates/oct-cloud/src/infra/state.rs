use petgraph::visit::NodeIndexable;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

use petgraph::Graph;
use petgraph::graph::NodeIndex;

use crate::infra::resource::{Node, ResourceType};

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct State {
    resources: Vec<ResourceState>,
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq, Clone)]
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
        if graph.node_count() > 0 {
            let root_index = graph.from_index(0);
            for node_index in graph.neighbors(root_index) {
                queue.push_back(node_index);

                parents
                    .entry(node_index)
                    .or_insert_with(Vec::new)
                    .push(root_index);
            }
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
            let mut parent_node_names: Vec<String> = parents
                .iter()
                .filter_map(|x| graph.node_weight(*x))
                .filter_map(|x| match x {
                    Node::Root => None,
                    Node::Resource(parent_resource_type) => Some(parent_resource_type.name()),
                })
                .collect();

            parent_node_names.sort();

            if let Some(Node::Resource(resource_type)) = graph.node_weight(*child_index) {
                log::info!("Add to state {resource_type:?}");

                resource_states.push(ResourceState {
                    name: resource_type.name(),
                    resource: resource_type.clone(),
                    dependencies: parent_node_names,
                });
            }
        }

        resource_states.sort_by(|a, b| {
            a.dependencies
                .len()
                .cmp(&b.dependencies.len())
                .then_with(|| a.name.cmp(&b.name))
        });

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::resource::{Node, ResourceType, Subnet, Vpc};
    use petgraph::Graph;

    #[test]
    fn test_state_from_graph_empty() {
        // Arrange
        let graph = Graph::<Node, String>::new();

        // Act
        let state = State::from_graph(&graph);

        // Assert
        assert!(state.resources.is_empty());
    }

    #[test]
    fn test_state_from_graph_single_node() {
        // Arrange
        let mut graph = Graph::<Node, String>::new();
        let root = graph.add_node(Node::Root);
        let vpc = Vpc {
            id: String::from("vpc-id"),
            name: String::from("vpc-name"),
            region: String::from("us-west-2"),
            cidr_block: String::from("10.0.0.0/16"),
        };
        let resource_node = graph.add_node(Node::Resource(ResourceType::Vpc(vpc.clone())));
        graph.add_edge(root, resource_node, String::new());

        // Act
        let state = State::from_graph(&graph);

        // Assert
        assert_eq!(
            state,
            State {
                resources: vec![ResourceState {
                    name: String::from("vpc.vpc-name"),
                    resource: ResourceType::Vpc(vpc),
                    dependencies: vec![],
                }]
            }
        );
    }

    #[test]
    fn test_state_from_graph_with_dependencies() {
        // Arrange
        let mut graph = Graph::<Node, String>::new();
        let root = graph.add_node(Node::Root);
        let vpc = Vpc {
            id: String::from("vpc-id"),
            name: String::from("vpc-name"),
            region: String::from("us-west-2"),
            cidr_block: String::from("10.0.0.0/16"),
        };
        let vpc_node = graph.add_node(Node::Resource(ResourceType::Vpc(vpc.clone())));
        graph.add_edge(root, vpc_node, String::new());

        let subnet = Subnet {
            id: String::from("subnet-id"),
            name: String::from("subnet-name"),
            cidr_block: String::from("10.0.1.0/24"),
            availability_zone: String::from("us-west-2a"),
        };
        let subnet_node = graph.add_node(Node::Resource(ResourceType::Subnet(subnet.clone())));
        graph.add_edge(vpc_node, subnet_node, String::new());

        // Act
        let state = State::from_graph(&graph);

        // Assert
        assert_eq!(
            state,
            State {
                resources: vec![
                    ResourceState {
                        name: String::from("vpc.vpc-name"),
                        resource: ResourceType::Vpc(vpc),
                        dependencies: vec![],
                    },
                    ResourceState {
                        name: String::from("subnet.subnet-name"),
                        resource: ResourceType::Subnet(subnet),
                        dependencies: vec![String::from("vpc.vpc-name")],
                    },
                ]
            }
        );
    }

    #[test]
    fn test_state_to_graph_empty() {
        // Arrange
        let state = State::default();

        // Act
        let graph = state.to_graph();

        // Assert
        assert_eq!(graph.node_count(), 1); // Root node
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn test_state_to_graph_single_node() {
        // Arrange
        let vpc = Vpc {
            id: String::from("vpc-id"),
            name: String::from("vpc-name"),
            region: String::from("us-west-2"),
            cidr_block: String::from("10.0.0.0/16"),
        };
        let resource_state = ResourceState {
            name: String::from("vpc.vpc-name"),
            resource: ResourceType::Vpc(vpc),
            dependencies: vec![],
        };
        let state = State {
            resources: vec![resource_state],
        };

        // Act
        let graph = state.to_graph();

        // Assert
        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);

        let root_node_index = graph
            .node_indices()
            .find(|i| matches!(graph[*i], Node::Root))
            .expect("Root node not found");
        let vpc_node_index = graph
            .node_indices()
            .find(|i| matches!(graph[*i], Node::Resource(ResourceType::Vpc(_))))
            .expect("VPC node not found");

        assert!(graph.contains_edge(root_node_index, vpc_node_index));
    }

    #[test]
    fn test_state_to_graph_with_dependencies() {
        // Arrange
        let vpc = Vpc {
            id: String::from("vpc-id"),
            name: String::from("vpc-name"),
            region: String::from("us-west-2"),
            cidr_block: String::from("10.0.0.0/16"),
        };
        let vpc_resource_state = ResourceState {
            name: String::from("vpc.vpc-name"),
            resource: ResourceType::Vpc(vpc),
            dependencies: vec![],
        };

        let subnet = Subnet {
            id: String::from("subnet-id"),
            name: String::from("subnet-name"),
            cidr_block: String::from("10.0.1.0/24"),
            availability_zone: String::from("us-west-2a"),
        };
        let subnet_resource_state = ResourceState {
            name: String::from("subnet.subnet-name"),
            resource: ResourceType::Subnet(subnet),
            dependencies: vec![String::from("vpc.vpc-name")],
        };

        let state = State {
            resources: vec![vpc_resource_state, subnet_resource_state],
        };

        // Act
        let graph = state.to_graph();

        // Assert
        assert_eq!(graph.node_count(), 3);
        assert_eq!(graph.edge_count(), 2);

        let root_node_index = graph
            .node_indices()
            .find(|i| matches!(graph[*i], Node::Root))
            .expect("Root node not found");
        let vpc_node_index = graph
            .node_indices()
            .find(|i| matches!(graph[*i], Node::Resource(ResourceType::Vpc(_))))
            .expect("VPC node not found");
        let subnet_node_index = graph
            .node_indices()
            .find(|i| matches!(graph[*i], Node::Resource(ResourceType::Subnet(_))))
            .expect("Subnet node not found");

        assert!(graph.contains_edge(root_node_index, vpc_node_index));
        assert!(graph.contains_edge(vpc_node_index, subnet_node_index));
    }
}
