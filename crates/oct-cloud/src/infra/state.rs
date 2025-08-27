use petgraph::visit::NodeIndexable;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

use petgraph::Graph;
use petgraph::graph::NodeIndex;

use crate::infra::resource::{Node, ResourceType};

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
