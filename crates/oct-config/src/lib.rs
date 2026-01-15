use std::collections::HashMap;
use std::fs;

use petgraph::Graph;
use petgraph::graph::NodeIndex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    pub project: Project,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum Node {
    /// The synthetic root node.
    #[default]
    Root,
    /// A user service in the dependency graph.
    Resource(Service),
}

impl std::fmt::Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Node::Root => write!(f, "Root"),
            Node::Resource(service) => write!(f, "service: {service:?}"),
        }
    }
}

impl Config {
    const DEFAULT_CONFIG_PATH: &'static str = "oct.toml";

    pub fn new(path: Option<&str>) -> Result<Self, Box<dyn std::error::Error>> {
        let config =
            fs::read_to_string(path.unwrap_or(Self::DEFAULT_CONFIG_PATH)).map_err(|e| {
                format!(
                    "Failed to read config file {}: {}",
                    Self::DEFAULT_CONFIG_PATH,
                    e
                )
            })?;

        let config_with_injected_envs = Self::render_system_envs(config);

        let mut toml_data: Config = toml::from_str(&config_with_injected_envs)?;

        for (service_name, service) in &mut toml_data.project.services {
            service.name.clone_from(service_name);
        }

        Ok(toml_data)
    }

    /// Converts user services to a graph
    pub fn to_graph(&self) -> Result<Graph<Node, String>, Box<dyn std::error::Error>> {
        let mut graph = Graph::<Node, String>::new();
        let mut edges = Vec::new();
        let root = graph.add_node(Node::Root);

        let mut services_map: HashMap<String, NodeIndex> = HashMap::new();
        for (service_name, service) in &self.project.services {
            let node = graph.add_node(Node::Resource(service.clone()));

            services_map.insert(service_name.clone(), node);
        }

        for (service_name, service) in &self.project.services {
            let resource = services_map
                .get(service_name)
                .expect("Missed resource value in resource_map");

            if service.depends_on.is_empty() {
                edges.push((root, *resource, String::new()));
            } else {
                for dependency_name in &service.depends_on {
                    let dependency_resource = services_map.get(dependency_name);

                    match dependency_resource {
                        Some(dependency_resource) => {
                            edges.push((*dependency_resource, *resource, String::new()));
                        }
                        None => {
                            return Err(format!(
                                "Missed resource with name '{dependency_name}' referenced as dependency in '{service_name}' service"
                            )
                            .into());
                        }
                    }
                }
            }
        }

        graph.extend_with_edges(&edges);

        Ok(graph)
    }

    /// Renders environment variables using [tera](https://docs.rs/tera/latest/tera/)
    /// All system environment variables are available under the `env` context variable
    fn render_system_envs(config: String) -> String {
        let mut context = tera::Context::new();
        context.insert("env", &std::env::vars().collect::<HashMap<_, _>>());

        let render_result = tera::Tera::one_off(&config, &context, true);

        match render_result {
            Ok(render_result) => {
                log::info!("Config with injected env vars:\n{render_result}");

                render_result
            }
            Err(e) => {
                log::warn!("Failed to render string: '{config}', error: {e}, context: {context:?}");

                config
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum StateBackend {
    #[serde(rename = "local")]
    Local {
        /// Local path to the state file
        path: String,
    },

    #[serde(rename = "s3")]
    S3 {
        /// Bucket region
        region: String,
        /// Bucket name
        bucket: String,
        /// Path to the file inside the S3 bucket
        key: String,
    },
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Project {
    pub name: String,

    pub state_backend: StateBackend,
    pub user_state_backend: StateBackend,

    pub services: HashMap<String, Service>,

    pub domain: Option<String>,
}

/// Configuration for a service
/// This configuration is managed by the user and used to deploy the service
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Service {
    /// Service name, injected from the key in the services map.
    #[serde(skip_deserializing, default)]
    pub name: String,
    /// Image to use for the container
    pub image: String,
    /// Path to the Dockerfile
    pub dockerfile_path: Option<String>,
    /// Command to run in the container
    pub command: Option<String>,
    /// Internal port exposed from the container
    pub internal_port: Option<u32>,
    /// External port exposed to the public internet
    pub external_port: Option<u32>,
    /// CPU millicores
    pub cpus: u32,
    /// Memory in MB
    pub memory: u64,
    /// List of services that this service depends on
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Raw environment variables to set in the container
    /// All values are rendered using in `render_envs` method
    #[serde(default)]
    pub envs: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn test_config_new_success_path_privided() {
        // Arrange
        let config_file_content = r#" 
[project]
name = "example"
domain = "opencloudtool.com"

[project.state_backend.local]
path = "./state.json"

[project.user_state_backend.local]
path = "./user_state.json"

[project.services.app_1]
image = ""
dockerfile_path = "Dockerfile"
command = "echo Hello World!"
internal_port = 80
external_port = 80
cpus = 250
memory = 64

[project.services.app_1.envs]
KEY1 = "VALUE1"
KEY2 = """
Multiline
string"""
KEY_WITH_INJECTED_ENV = "{{ env.CARGO_PKG_NAME }}"
KEY_WITH_OTHER_TEMPLATE_VARIABLE = "{{ other_vars.some_var }}"

[project.services.app_2]
image = "nginx:latest"
cpus = 250
memory = 64
depends_on = ["app_1"]
"#;

        let mut config_file = tempfile::NamedTempFile::new().expect("Failed to create a temp file");
        config_file
            .write_all(config_file_content.as_bytes())
            .expect("Failed to write to file");

        // Act
        let config =
            Config::new(config_file.path().to_str()).expect("Failed to create a new config");

        // Assert
        assert_eq!(
            config,
            Config {
                project: Project {
                    name: "example".to_string(),
                    state_backend: StateBackend::Local {
                        path: "./state.json".to_string()
                    },
                    user_state_backend: StateBackend::Local {
                        path: "./user_state.json".to_string()
                    },
                    services: HashMap::from([
                        (
                            "app_1".to_string(),
                            Service {
                                name: "app_1".to_string(),
                                image: String::new(),
                                dockerfile_path: Some("Dockerfile".to_string()),
                                command: Some("echo Hello World!".to_string()),
                                internal_port: Some(80),
                                external_port: Some(80),
                                cpus: 250,
                                memory: 64,
                                depends_on: vec![],
                                envs: HashMap::from([
                                    ("KEY1".to_string(), "VALUE1".to_string()),
                                    ("KEY2".to_string(), "Multiline\nstring".to_string()),
                                    (
                                        "KEY_WITH_INJECTED_ENV".to_string(),
                                        // "oct-orchestrator" was the previous value because it was in that crate.
                                        // Now it's in oct-config, so CARGO_PKG_NAME will be oct-config.
                                        // Wait, the test uses {{ env.CARGO_PKG_NAME }}.
                                        // When running tests for oct-config, CARGO_PKG_NAME is oct-config.
                                        "oct-config".to_string()
                                    ),
                                    (
                                        "KEY_WITH_OTHER_TEMPLATE_VARIABLE".to_string(),
                                        "{{ other_vars.some_var }}".to_string()
                                    ),
                                ]),
                            }
                        ),
                        (
                            "app_2".to_string(),
                            Service {
                                name: "app_2".to_string(),
                                image: "nginx:latest".to_string(),
                                dockerfile_path: None,
                                command: None,
                                internal_port: None,
                                external_port: None,
                                cpus: 250,
                                memory: 64,
                                depends_on: vec!["app_1".to_string()],
                                envs: HashMap::new(),
                            }
                        ),
                    ]),
                    domain: Some("opencloudtool.com".to_string()),
                }
            }
        );
    }

    #[test]
    fn test_config_to_graph_empty() {
        // Arrange
        let config = Config {
            project: Project {
                name: "test".to_string(),
                state_backend: StateBackend::Local {
                    path: "state.json".to_string(),
                },
                user_state_backend: StateBackend::Local {
                    path: "user_state.json".to_string(),
                },
                services: HashMap::new(),
                domain: None,
            },
        };

        // Act
        let graph = config.to_graph().expect("Failed to get graph");

        // Assert
        assert_eq!(graph.node_count(), 1); // Root node
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn test_config_to_graph_single_node() {
        // Arrange
        let service = Service {
            name: "app_1".to_string(),
            image: "nginx:latest".to_string(),
            dockerfile_path: None,
            command: None,
            internal_port: None,
            external_port: None,
            cpus: 250,
            memory: 64,
            depends_on: vec![],
            envs: HashMap::new(),
        };
        let config = Config {
            project: Project {
                name: "test".to_string(),
                state_backend: StateBackend::Local {
                    path: "state.json".to_string(),
                },
                user_state_backend: StateBackend::Local {
                    path: "user_state.json".to_string(),
                },
                services: HashMap::from([("app_1".to_string(), service)]),
                domain: None,
            },
        };

        // Act
        let graph = config.to_graph().expect("Failed to get graph");

        // Assert
        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);

        let root_node_index = graph
            .node_indices()
            .find(|i| matches!(graph[*i], Node::Root))
            .expect("Root node not found");
        let service_node_index = graph
            .node_indices()
            .find(|i| matches!(graph[*i], Node::Resource(_)))
            .expect("Service node not found");

        assert!(graph.contains_edge(root_node_index, service_node_index));
    }

    #[test]
    fn test_config_to_graph_with_dependencies() {
        // Arrange
        let service1 = Service {
            name: "app_1".to_string(),
            image: "nginx:latest".to_string(),
            dockerfile_path: None,
            command: None,
            internal_port: None,
            external_port: None,
            cpus: 250,
            memory: 64,
            depends_on: vec![],
            envs: HashMap::new(),
        };
        let service2 = Service {
            name: "app_2".to_string(),
            image: "nginx:latest".to_string(),
            dockerfile_path: None,
            command: None,
            internal_port: None,
            external_port: None,
            cpus: 250,
            memory: 64,
            depends_on: vec!["app_1".to_string()],
            envs: HashMap::new(),
        };
        let config = Config {
            project: Project {
                name: "test".to_string(),
                state_backend: StateBackend::Local {
                    path: "state.json".to_string(),
                },
                user_state_backend: StateBackend::Local {
                    path: "user_state.json".to_string(),
                },
                services: HashMap::from([
                    ("app_1".to_string(), service1.clone()),
                    ("app_2".to_string(), service2.clone()),
                ]),
                domain: None,
            },
        };

        // Act
        let graph = config.to_graph().expect("Failed to get graph");

        // Assert
        assert_eq!(graph.node_count(), 3);
        assert_eq!(graph.edge_count(), 2);

        let root_node_index = graph
            .node_indices()
            .find(|i| matches!(graph[*i], Node::Root))
            .expect("Root node not found");
        let service1_node_index = graph
            .node_indices()
            .find(|i| graph[*i] == Node::Resource(service1.clone()))
            .expect("Service 1 node not found");
        let service2_node_index = graph
            .node_indices()
            .find(|i| graph[*i] == Node::Resource(service2.clone()))
            .expect("Service 2 node not found");

        assert!(graph.contains_edge(root_node_index, service1_node_index));
        assert!(graph.contains_edge(service1_node_index, service2_node_index));
    }

    #[test]
    fn test_config_to_graph_failed_to_get_dependency() {
        // Arrange
        let service = Service {
            name: "app_1".to_string(),
            image: "nginx:latest".to_string(),
            dockerfile_path: None,
            command: None,
            internal_port: None,
            external_port: None,
            cpus: 250,
            memory: 64,
            depends_on: vec!["INCORRECT_SERVICE_NAME".to_string()],
            envs: HashMap::new(),
        };
        let config = Config {
            project: Project {
                name: "test".to_string(),
                state_backend: StateBackend::Local {
                    path: "state.json".to_string(),
                },
                user_state_backend: StateBackend::Local {
                    path: "user_state.json".to_string(),
                },
                services: HashMap::from([("app_1".to_string(), service)]),
                domain: None,
            },
        };

        // Act
        let graph = config.to_graph();

        // Assert
        assert!(graph.is_err());
        assert_eq!(
            graph.expect_err("Expected error").to_string(),
            "Missed resource with name 'INCORRECT_SERVICE_NAME' referenced as dependency in 'app_1' service"
        );
    }
}
