use std::fs;

use serde::{Deserialize, Serialize};
use toml;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    pub project: Project,
}

impl Config {
    const DEFAULT_CONFIG_PATH: &'static str = "oct.toml";

    pub fn new(path: Option<&str>) -> Result<Self, Box<dyn std::error::Error>> {
        let data = fs::read_to_string(path.unwrap_or(Self::DEFAULT_CONFIG_PATH))?;
        let toml_data: Config = toml::from_str(&data)?;

        Ok(toml_data)
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Project {
    pub name: String,
    pub services: Vec<Service>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Service {
    pub name: String,
    pub image: String,
    pub internal_port: u32,
    pub external_port: u32,
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile;

    use super::*;

    #[test]
    fn test_config_new_success_path_privided() {
        // Arrange
        let config_file_content = r#"
[project]
name = "example"

[[project.services]]
name = "app_1"
image = "nginx:latest"
internal_port = 80
external_port = 80

[[project.services]]
name = "app_2"
image = "nginx:latest"
internal_port = 80
external_port = 80
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
                    services: vec![
                        Service {
                            name: "app_1".to_string(),
                            image: "nginx:latest".to_string(),
                            internal_port: 80,
                            external_port: 80,
                        },
                        Service {
                            name: "app_2".to_string(),
                            image: "nginx:latest".to_string(),
                            internal_port: 80,
                            external_port: 80,
                        }
                    ]
                }
            }
        );
    }
}
