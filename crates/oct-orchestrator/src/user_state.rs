use std::collections::HashMap;
use std::fs;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Default, Eq, PartialEq)]
pub(crate) struct UserState {
    #[serde(skip)]
    file_path: String,

    /// Key - public IP, Value - instance
    pub(crate) instances: HashMap<String, Instance>,
}

impl UserState {
    /// Load state from file or create a new one
    pub(crate) fn new(file_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let user_state = if std::path::Path::new(file_path).exists() {
            let existing_data = fs::read_to_string(file_path)?;
            serde_json::from_str::<UserState>(&existing_data)?
        } else {
            UserState::default()
        };

        Ok(UserState {
            file_path: file_path.to_string(),
            ..user_state
        })
    }

    /// Save state to file
    pub(crate) fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        fs::write(&self.file_path, serde_json::to_string_pretty(self)?)?;

        Ok(())
    }

    /// Get context of all services running on instances
    /// Key - service name, Value - service context
    pub(crate) fn get_services_context(&self) -> HashMap<String, ServiceContext> {
        let mut context = HashMap::new();

        for (public_ip, instance) in &self.instances {
            for service_name in instance.services.keys() {
                context.insert(
                    service_name.clone(),
                    ServiceContext {
                        public_ip: public_ip.clone(),
                    },
                );
            }
        }

        context
    }
}

/// Context of a service running on an instance
#[derive(Serialize, Debug, Eq, PartialEq)]
pub(crate) struct ServiceContext {
    pub(crate) public_ip: String,
}

#[derive(Serialize, Deserialize, Debug, Default, Eq, PartialEq)]
pub(crate) struct Instance {
    /// CPUs available on instance
    pub(crate) cpus: u32,
    /// Memory available on instance
    pub(crate) memory: u64,

    /// Services running on instance
    pub(crate) services: HashMap<String, Service>,
}

impl Instance {
    /// Gets cpus and memory available on instance
    pub(crate) fn get_available_resources(&self) -> (u32, u64) {
        let available_cpus = self.cpus - self.services.values().map(|s| s.cpus).sum::<u32>();
        let available_memory = self.memory - self.services.values().map(|s| s.memory).sum::<u64>();

        (available_cpus, available_memory)
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Eq, PartialEq)]
pub(crate) struct Service {
    /// CPUs required by service
    pub(crate) cpus: u32,
    /// Memory required by service
    pub(crate) memory: u64,
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile;

    use super::*;

    #[test]
    fn test_user_state_new_exists() {
        // Arrange
        let state_file_content = r#"
{
    "instances": {
        "89.0.142.86": {
            "cpus": 1000,
            "memory": 1024,
            "services": {
                "test": {
                    "cpus": 1000,
                    "memory": 1024
                },
                "test2": {
                    "cpus": 1000,
                    "memory": 1024
                }
            }
        }
    }
}
"#;
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(state_file_content.as_bytes()).unwrap();

        // Act
        let user_state = UserState::new(file.path().to_str().unwrap()).unwrap();

        // Assert
        assert_eq!(
            user_state,
            UserState {
                file_path: file.path().to_str().unwrap().to_string(),
                instances: HashMap::from([(
                    "89.0.142.86".to_string(),
                    Instance {
                        cpus: 1000,
                        memory: 1024,
                        services: HashMap::from([
                            (
                                "test".to_string(),
                                Service {
                                    cpus: 1000,
                                    memory: 1024,
                                },
                            ),
                            (
                                "test2".to_string(),
                                Service {
                                    cpus: 1000,
                                    memory: 1024,
                                },
                            ),
                        ])
                    },
                )])
            }
        );
    }

    #[test]
    fn test_user_state_new_not_exists() {
        // Act
        let user_state = UserState::new("NO_FILE").unwrap();

        // Assert
        assert_eq!(user_state.instances.len(), 0);
    }

    #[test]
    fn test_user_state_save() {
        // Arrange
        let state_file = tempfile::NamedTempFile::new().unwrap();

        let user_state = UserState {
            file_path: state_file.path().to_str().unwrap().to_string(),
            instances: HashMap::from([(
                "test".to_string(),
                Instance {
                    cpus: 1000,
                    memory: 1024,
                    services: HashMap::from([(
                        "test".to_string(),
                        Service {
                            cpus: 1000,
                            memory: 1024,
                        },
                    )]),
                },
            )]),
        };

        // Act
        user_state.save().unwrap();

        // Assert
        let file_content = fs::read_to_string(state_file.path()).unwrap();
        assert_eq!(
            file_content,
            r#"{
  "instances": {
    "test": {
      "cpus": 1000,
      "memory": 1024,
      "services": {
        "test": {
          "cpus": 1000,
          "memory": 1024
        }
      }
    }
  }
}"#
        );
    }

    #[test]
    fn test_user_state_get_services_context() {
        let user_state = UserState {
            file_path: "test".to_string(),
            instances: HashMap::from([(
                "1.2.3.4".to_string(),
                Instance {
                    cpus: 1000,
                    memory: 1024,
                    services: HashMap::from([
                        (
                            "app_1".to_string(),
                            Service {
                                cpus: 1000,
                                memory: 1024,
                            },
                        ),
                        (
                            "app_2".to_string(),
                            Service {
                                cpus: 250,
                                memory: 256,
                            },
                        ),
                    ]),
                },
            )]),
        };

        // Act
        let context = user_state.get_services_context();

        // Assert
        assert_eq!(
            context,
            HashMap::from([
                (
                    "app_1".to_string(),
                    ServiceContext {
                        public_ip: "1.2.3.4".to_string()
                    }
                ),
                (
                    "app_2".to_string(),
                    ServiceContext {
                        public_ip: "1.2.3.4".to_string()
                    }
                )
            ])
        );
    }

    #[test]
    fn test_instance_get_available_resources() {
        let instance = Instance {
            cpus: 1000,
            memory: 1024,
            services: HashMap::from([
                (
                    "test".to_string(),
                    Service {
                        cpus: 500,
                        memory: 512,
                    },
                ),
                (
                    "test2".to_string(),
                    Service {
                        cpus: 250,
                        memory: 256,
                    },
                ),
            ]),
        };

        assert_eq!(instance.get_available_resources(), (250, 256));
    }
}
