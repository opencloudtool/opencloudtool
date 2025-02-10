use std::collections::HashMap;
use std::fs;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Default, Eq, PartialEq)]
pub(crate) struct UserState {
    /// Key - public IP, Value - instance
    pub(crate) instances: HashMap<String, Instance>,
}

impl UserState {
    /// Load state from file or create a new one
    pub(crate) fn new(file_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        if std::path::Path::new(file_path).exists() {
            let existing_data = fs::read_to_string(file_path)?;
            Ok(serde_json::from_str::<UserState>(&existing_data)?)
        } else {
            Ok(UserState::default())
        }
    }

    /// Save state to file
    pub(crate) fn save(&self, file_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        fs::write(file_path, serde_json::to_string_pretty(self)?)?;

        Ok(())
    }
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
        let user_state = UserState {
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

        let state_file = tempfile::NamedTempFile::new().unwrap();

        // Act
        user_state
            .save(state_file.path().to_str().unwrap())
            .unwrap();

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
}
