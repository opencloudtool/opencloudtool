use std::collections::HashMap;
use std::fs;

use serde::{Deserialize, Serialize};

use crate::user_state;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Config {
    pub(crate) project: Project,
}

impl Config {
    const DEFAULT_CONFIG_PATH: &'static str = "oct.toml";

    pub(crate) fn new(path: Option<&str>) -> Result<Self, Box<dyn std::error::Error>> {
        let config =
            fs::read_to_string(path.unwrap_or(Self::DEFAULT_CONFIG_PATH)).map_err(|e| {
                format!(
                    "Failed to read config file {}: {}",
                    Self::DEFAULT_CONFIG_PATH,
                    e
                )
            })?;

        let config_with_injected_envs = Self::render_system_envs(config);

        let toml_data: Config = toml::from_str(&config_with_injected_envs)?;

        Ok(toml_data)
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
pub(crate) enum StateBackend {
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
pub(crate) struct Project {
    pub(crate) name: String,

    pub(crate) state_backend: StateBackend,
    pub(crate) user_state_backend: StateBackend,

    pub(crate) services: HashMap<String, Service>,

    pub(crate) domain: Option<String>,
}

/// Configuration for a service
/// This configuration is managed by the user and used to deploy the service
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Service {
    /// Image to use for the container
    pub(crate) image: String,
    /// Path to the Dockerfile
    pub(crate) dockerfile_path: Option<String>,
    /// Command to run in the container
    pub(crate) command: Option<String>,
    /// Internal port exposed from the container
    pub(crate) internal_port: Option<u32>,
    /// External port exposed to the public internet
    pub(crate) external_port: Option<u32>,
    /// CPU millicores
    pub(crate) cpus: u32,
    /// Memory in MB
    pub(crate) memory: u64,
    /// List of services that this service depends on
    pub(crate) depends_on: Option<Vec<String>>,
    /// Raw environment variables to set in the container
    /// All values are rendered using in `render_envs` method
    #[serde(default)]
    pub(crate) envs: HashMap<String, String>,
}

impl Service {
    /// Renders environment variables using [tera](https://docs.rs/tera/latest/tera/)
    /// Available variables:
    /// - `services`: Map of all services
    pub(crate) fn render_envs(
        &self,
        services_context: &HashMap<String, user_state::ServiceContext>,
    ) -> HashMap<String, String> {
        let mut context = tera::Context::new();
        context.insert("services", services_context);

        let mut rendered_envs = HashMap::new();

        for (env_name, env_value) in &self.envs {
            let rendered = tera::Tera::one_off(env_value, &context, true);

            match rendered {
                Ok(rendered) => {
                    rendered_envs.insert(env_name.clone(), rendered);
                }
                Err(err) => {
                    log::warn!("Failed to render string: '{env_value}', error: {err}");

                    rendered_envs.insert(env_name.clone(), env_value.clone());
                }
            }
        }

        rendered_envs
    }
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
KEY2 = """Multiline
string"""
KEY_WITH_INJECTED_ENV = "{{ env.CARGO_PKG_NAME }}"
KEY_WITH_OTHER_TEMPLATE_VARIABLE = "{{ other_vars.some_var }}"

[project.services.app_2]
image = "nginx:latest"
cpus = 250
memory = 64
depends_on = ["app_1"]
"#;

        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(config_file_content.as_bytes()).unwrap();

        // Act
        let config = Config::new(file.path().to_str()).unwrap();

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
                                image: String::new(),
                                dockerfile_path: Some("Dockerfile".to_string()),
                                command: Some("echo Hello World!".to_string()),
                                internal_port: Some(80),
                                external_port: Some(80),
                                cpus: 250,
                                memory: 64,
                                depends_on: None,
                                envs: HashMap::from([
                                    ("KEY1".to_string(), "VALUE1".to_string()),
                                    ("KEY2".to_string(), "Multiline\nstring".to_string()),
                                    (
                                        "KEY_WITH_INJECTED_ENV".to_string(),
                                        "oct-orchestrator".to_string()
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
                                image: "nginx:latest".to_string(),
                                dockerfile_path: None,
                                command: None,
                                internal_port: None,
                                external_port: None,
                                cpus: 250,
                                memory: 64,
                                depends_on: Some(vec!("app_1".to_string())),
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
    fn test_service_render_envs_success() {
        // Arrange
        let service = Service {
            image: "nginx:latest".to_string(),
            dockerfile_path: None,
            command: None,
            internal_port: None,
            external_port: None,
            cpus: 250,
            memory: 64,
            depends_on: Some(vec!["app_1".to_string()]),
            envs: HashMap::from([(
                "KEY".to_string(),
                "Service public_ip={{ services.app_1.public_ip }}".to_string(),
            )]),
        };

        let services_context = HashMap::from([(
            "app_1".to_string(),
            user_state::ServiceContext {
                public_ip: "1.2.3.4".to_string(),
            },
        )]);

        // Act
        let rendered_envs = service.render_envs(&services_context);

        // Assert
        assert_eq!(
            rendered_envs,
            HashMap::from([("KEY".to_string(), "Service public_ip=1.2.3.4".to_string())])
        );
    }

    #[test]
    fn test_service_render_envs_failure() {
        // Arrange
        let service = Service {
            image: "nginx:latest".to_string(),
            dockerfile_path: None,
            command: None,
            internal_port: None,
            external_port: None,
            cpus: 250,
            memory: 64,
            depends_on: Some(vec!["app_1".to_string()]),
            envs: HashMap::from([(
                "KEY".to_string(),
                "Service public_ip={{ UNKNOWN_VAR }}".to_string(),
            )]),
        };

        let services_context = HashMap::new();

        // Act
        let rendered_envs = service.render_envs(&services_context);

        // Assert
        assert_eq!(
            rendered_envs,
            HashMap::from([(
                "KEY".to_string(),
                "Service public_ip={{ UNKNOWN_VAR }}".to_string()
            )])
        );
    }
}
