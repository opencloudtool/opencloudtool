/// TODO:
/// - Deployer implementation
///   - Graph Node type?
/// - Resources refactoring:
///   - Split current Resource into Resource (plain struct) and `ResourceManager` (create, destroy, etc.)
///   - "State - graph" - communication
///
/// - Workflow:
///   - graph with resources (hard-coded)
///   - infra deployer (uses resources from the graph. How to )
///     - How to deploy resource? ResourceManager?
///   - save state
///     -
///
use petgraph::Direction;
use petgraph::Graph;
use petgraph::algo;
use petgraph::visit::IntoNodeIdentifiers;
use std::collections::{HashMap, VecDeque};

#[derive(Default)]
enum Node {
    /// The synthetic root node.
    #[default]
    Root,
    /// A package in the dependency graph.
    Package(Resource),
}

impl std::fmt::Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Node::Root => write!(f, "Root"),
            Node::Package(resource) => write!(f, "{}", resource.name),
        }
    }
}

#[derive(Default)]
struct Resource {
    name: String,
}

fn get_graph(number_of_instances: i32) -> Graph<Node, String> {
    let mut deps = Graph::<Node, String>::new();
    let root = deps.add_node(Node::Root);
    let vpc = deps.add_node(Node::Package(Resource {
        name: "VPC".to_string(),
    }));
    let subnet = deps.add_node(Node::Package(Resource {
        name: "Subnet".to_string(),
    }));
    let ecr = deps.add_node(Node::Package(Resource {
        name: "ECR".to_string(),
    }));
    let instance_profile = deps.add_node(Node::Package(Resource {
        name: "InstanceProfile".to_string(),
    }));
    let hosted_zone = deps.add_node(Node::Package(Resource {
        name: "HostedZone".to_string(),
    }));

    let instances: Vec<_> = (0..number_of_instances)
        .map(|i| {
            deps.add_node(Node::Package(Resource {
                name: format!("Ec2Instance{}", i + 1),
            }))
        })
        .collect();

    let mut edges = vec![
        (root, vpc, "".to_string()),
        (root, ecr, "".to_string()),
        (root, instance_profile, "".to_string()),
        (root, hosted_zone, "".to_string()),
        (vpc, subnet, "".to_string()),
    ];

    for instance in instances.iter() {
        edges.extend([
            (subnet, *instance, "".to_string()),
            (ecr, *instance, "".to_string()),
            (hosted_zone, *instance, "".to_string()),
            (instance_profile, *instance, "".to_string()),
        ]);
    }

    deps.extend_with_edges(&edges);

    deps
}

/// Perform topological sort using petgraph's built-in function
fn iterate_sequentially(graph: &Graph<Node, String>) -> Vec<&Node> {
    let mut sorted_nodes = Vec::new();

    if let Ok(sorted_node_indexes) = algo::toposort(graph, None) {
        for node_index in sorted_node_indexes {
            if let Some(node_data) = graph.node_weight(node_index) {
                sorted_nodes.push(node_data);
            };
        }
    }

    sorted_nodes
}

fn iterate_parallel(graph: &Graph<Node, String>) {
    // Perform level-by-level traversal based on dependencies (modified Kahn's)
    let mut in_degree = HashMap::new();
    let mut current_level_queue = VecDeque::new();
    let mut next_level_queue = VecDeque::new();
    let mut processed_nodes_count = 0;
    let node_count = graph.node_count();

    // Initialize in-degree for all nodes
    for node_idx in graph.node_identifiers() {
        let degree = graph.edges_directed(node_idx, Direction::Incoming).count();
        in_degree.insert(node_idx, degree);
        if degree == 0 {
            current_level_queue.push_back(node_idx);
        }
    }

    println!("Processing nodes level by level (dependencies first):");
    let mut level = 0;

    // Process nodes level by level
    while !current_level_queue.is_empty() {
        println!("--- Level {} ---", level);
        let mut nodes_in_level = Vec::new(); // Store nodes for printing later (simulates parallel processing)

        while let Some(node_idx) = current_level_queue.pop_front() {
            nodes_in_level.push(node_idx);
            processed_nodes_count += 1;

            // For each neighbor (dependent node), decrease its in-degree
            for neighbor_idx in graph.neighbors(node_idx) {
                // neighbors() = outgoing edges
                if let Some(degree) = in_degree.get_mut(&neighbor_idx) {
                    *degree -= 1;
                    if *degree == 0 {
                        next_level_queue.push_back(neighbor_idx);
                    }
                }
            }
        }

        // Print all nodes processed in this level
        for node_idx in nodes_in_level {
            match graph.node_weight(node_idx) {
                Some(node_data) => println!("  Node: {}", node_data),
                None => println!("  Node Index: {:?} (Weight not found)", node_idx),
            }
        }

        // Move to the next level
        std::mem::swap(&mut current_level_queue, &mut next_level_queue);
        // next_level_queue is already empty here after swap
        level += 1;
    }

    // Check for cycles
    if processed_nodes_count != node_count {
        println!("Error: Cycle detected in the graph. Processing stopped partially.");
    } else {
        println!("Level-by-level processing complete.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use petgraph::dot::Dot;

    #[test]
    fn test_get_graph() {
        let deps = get_graph(10);

        println!("{}", Dot::new(&deps));

        // assert_eq!(deps.node_count(), 10);
        // assert_eq!(deps.edge_count(), 12);
    }

    // #[test]
    // fn test_iterate_sequentially() {
    //     let deps = get_graph();
    //     iterate_sequentially(&deps);
    // }

    // #[test]
    // fn test_iterate_parallel() {
    //     let deps = get_graph();
    //     iterate_parallel(&deps);
    // }
}
