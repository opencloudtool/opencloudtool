use std::collections::HashMap;
use std::fs;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Service {
    pub public_ip: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub(crate) struct UserState {
    pub services: HashMap<String, Service>,
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

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile;

    use super::*;

    #[test]
    fn test_user_state() {
        let user_state = UserState {
            services: HashMap::from([
                (
                    "test".to_string(),
                    Service {
                        public_ip: "test".to_string(),
                    },
                ),
                (
                    "test2".to_string(),
                    Service {
                        public_ip: "test2".to_string(),
                    },
                ),
            ]),
        };
        assert_eq!(user_state.services.len(), 2);
        assert_eq!(user_state.services["test"].public_ip, "test");
        assert_eq!(user_state.services["test2"].public_ip, "test2");
    }

    #[test]
    fn test_user_state_new_exists() {
        // Arrange
        let state_file_content = r#"
{
    "services": {
        "test": {
            "public_ip": "test"
        },
        "test2": {
            "public_ip": "test2"
        }
    }
}
"#;
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(state_file_content.as_bytes()).unwrap();

        // Act
        let user_state = UserState::new(file.path().to_str().unwrap()).unwrap();

        // Assert
        assert_eq!(user_state.services.len(), 2);
        assert_eq!(user_state.services["test"].public_ip, "test");
        assert_eq!(user_state.services["test2"].public_ip, "test2");
    }

    #[test]
    fn test_user_state_new_not_exists() {
        // Act
        let user_state = UserState::new("NO_FILE").unwrap();

        // Assert
        assert_eq!(user_state.services.len(), 0);
    }

    #[test]
    fn test_user_state_save() {
        // Arrange
        let user_state = UserState {
            services: HashMap::from([(
                "test".to_string(),
                Service {
                    public_ip: "test".to_string(),
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
  "services": {
    "test": {
      "public_ip": "test"
    }
  }
}"#
        );
    }
}
