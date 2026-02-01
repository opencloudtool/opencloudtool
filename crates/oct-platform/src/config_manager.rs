use oct_config::{Config, Project, StateBackend};
use serde::Serialize;
use std::{fs, path::Path};
use tracing::error;

#[derive(Debug, Clone, Serialize)]
pub struct ProjectSummary {
    pub name: String,
    pub domain: Option<String>,
    pub services_count: usize,
}

pub trait ConfigManager: Send + Sync {
    fn load_project(&self, name: &str) -> Result<Config, Box<dyn std::error::Error + Send + Sync>>;
    fn load_project_raw(
        &self,
        name: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>>;
    fn save(&self, config: &Config) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    // Project Management
    fn list_projects(&self) -> Vec<ProjectSummary>;
    fn create_project(&self, name: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

pub struct FileConfigManager {
    path: String,
}

impl FileConfigManager {
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
        }
    }

    fn load(&self) -> Config {
        if !Path::new(&self.path).exists() {
            return Config {
                project: Project {
                    name: "New Project".to_string(),
                    state_backend: StateBackend::Local {
                        path: "./state.json".to_string(),
                    },
                    user_state_backend: StateBackend::Local {
                        path: "./user_state.json".to_string(),
                    },
                    services: vec![],
                    domain: None,
                },
            };
        }

        match Config::new(Some(&self.path)) {
            Ok(c) => c,
            Err(e) => {
                error!("Error loading config: {e}");
                Config {
                    project: Project {
                        name: "Error Loading Config".to_string(),
                        state_backend: StateBackend::Local {
                            path: "./state.json".to_string(),
                        },
                        user_state_backend: StateBackend::Local {
                            path: "./user_state.json".to_string(),
                        },
                        services: vec![],
                        domain: None,
                    },
                }
            }
        }
    }
}

impl ConfigManager for FileConfigManager {
    fn load_project(
        &self,
        _name: &str,
    ) -> Result<Config, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.load())
    }

    fn load_project_raw(
        &self,
        _name: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let config = self.load();
        let toml_str = toml::to_string(&config)?;
        Ok(toml_str)
    }

    fn save(&self, config: &Config) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let toml_str = toml::to_string(config)?;
        fs::write(&self.path, toml_str)?;
        Ok(())
    }

    fn list_projects(&self) -> Vec<ProjectSummary> {
        let config = self.load();
        vec![ProjectSummary {
            name: config.project.name,
            domain: config.project.domain,
            services_count: config.project.services.len(),
        }]
    }

    fn create_project(&self, _name: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Err("Project creation not supported in single-file mode".into())
    }
}

pub struct WorkspaceConfigManager {
    root_path: std::path::PathBuf,
}

impl Default for WorkspaceConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkspaceConfigManager {
    pub fn new() -> Self {
        let root = if let Ok(root_env) = std::env::var("OCT_WORKSPACE_ROOT") {
            Path::new(&root_env).to_path_buf()
        } else {
            let home = dirs::home_dir().expect("Could not find home directory");
            home.join(".oct")
        };

        Self::with_root(root)
    }

    pub fn with_root(root: std::path::PathBuf) -> Self {
        if !root.exists() {
            fs::create_dir_all(&root).expect("Could not create workspace directory");
        }
        Self { root_path: root }
    }

    fn project_path(&self, name: &str) -> std::path::PathBuf {
        self.root_path.join(name)
    }
}

impl ConfigManager for WorkspaceConfigManager {
    fn list_projects(&self) -> Vec<ProjectSummary> {
        if let Ok(entries) = fs::read_dir(&self.root_path) {
            let mut projects: Vec<ProjectSummary> = entries
                .filter_map(Result::ok)
                .filter(|e| e.path().is_dir())
                .filter_map(|e| {
                    let name = e.file_name().into_string().ok()?;
                    let config_path = e.path().join("oct.toml");
                    let config = if config_path.exists() {
                        Config::new(config_path.to_str()).ok()
                    } else {
                        None
                    };

                    Some(ProjectSummary {
                        name,
                        domain: config.as_ref().and_then(|c| c.project.domain.clone()),
                        services_count: config
                            .as_ref()
                            .map(|c| c.project.services.len())
                            .unwrap_or(0),
                    })
                })
                .collect();
            projects.sort_by(|a, b| a.name.cmp(&b.name));
            projects
        } else {
            vec![]
        }
    }

    fn create_project(&self, name: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let path = self.project_path(name);
        if path.exists() {
            return Err(format!("Project '{name}' already exists").into());
        }
        fs::create_dir_all(&path)?;

        let state_path = path.join("state.json");
        let config = Config {
            project: Project {
                name: name.to_string(),
                state_backend: StateBackend::Local {
                    path: state_path.to_string_lossy().to_string(),
                },
                user_state_backend: StateBackend::Local {
                    path: path.join("user_state.json").to_string_lossy().to_string(),
                },
                services: vec![],
                domain: None,
            },
        };

        let toml_str = toml::to_string(&config)?;
        fs::write(path.join("oct.toml"), toml_str)?;

        Ok(())
    }

    fn load_project(&self, name: &str) -> Result<Config, Box<dyn std::error::Error + Send + Sync>> {
        let config_path = self.project_path(name).join("oct.toml");

        match Config::new(Some(config_path.to_str().unwrap_or("oct.toml"))) {
            Ok(c) => Ok(c),
            Err(e) => Err(format!("Error loading config for {name}: {e}").into()),
        }
    }

    fn load_project_raw(
        &self,
        name: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let config_path = self.project_path(name).join("oct.toml");
        if config_path.exists() {
            let toml_str = fs::read_to_string(config_path)?;
            Ok(toml_str)
        } else {
            Ok(String::new())
        }
    }

    fn save(&self, config: &Config) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let name = &config.project.name;
        let config_path = self.project_path(name).join("oct.toml");

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let toml_str = toml::to_string(config)?;
        fs::write(config_path, toml_str)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_file_config_manager_load_existing() {
        // Arrange
        let mut file = NamedTempFile::new().expect("Failed to create temp file");
        let config_content = r#"
[project]
name = "Test Project"
domain = "example.com"
services = []

[project.state_backend.local]
path = "./state.json"

[project.user_state_backend.local]
path = "./user_state.json"
"#;
        file.write_all(config_content.as_bytes())
            .expect("Failed to write config content");
        file.flush().expect("Failed to flush file"); // Ensure content is written to disk

        // Act
        let manager = FileConfigManager::new(file.path().to_str().expect("Path to string failed"));
        let config = manager.load_project("any").expect("Failed to load project");

        // Assert
        assert_eq!(config.project.name, "Test Project");
        assert_eq!(config.project.domain, Some("example.com".to_string()));
    }

    #[test]
    fn test_file_config_manager_load_non_existent() {
        // Arrange
        let manager = FileConfigManager::new("/non/existent/path.toml");

        // Act
        let config = manager
            .load_project("any")
            .expect("Failed to load project (should return default on failure)");

        // Assert
        assert_eq!(config.project.name, "New Project");
    }

    #[test]
    fn test_file_config_manager_save() {
        // Arrange
        let file = NamedTempFile::new().expect("Failed to create temp file");
        let manager = FileConfigManager::new(file.path().to_str().expect("Path to string failed"));

        let config = Config {
            project: Project {
                name: "Saved Project".to_string(),
                state_backend: StateBackend::Local {
                    path: "state.json".to_string(),
                },
                user_state_backend: StateBackend::Local {
                    path: "user_state.json".to_string(),
                },
                services: vec![],
                domain: None,
            },
        };

        // Act
        manager.save(&config).expect("Failed to save config");

        // Assert
        let loaded = manager
            .load_project("any")
            .expect("Failed to reload project");
        assert_eq!(loaded.project.name, "Saved Project");
    }

    #[test]
    fn test_workspace_config_manager_create_and_load() {
        // Arrange
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        // Use with_root to inject the temporary directory without unsafe env var manipulation
        let manager = WorkspaceConfigManager::with_root(temp_dir.path().to_path_buf());

        // Act
        manager
            .create_project("test-proj")
            .expect("Failed to create project");

        // Assert
        let projects = manager.list_projects();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "test-proj");

        let config = manager
            .load_project("test-proj")
            .expect("Failed to load project");
        assert_eq!(config.project.name, "test-proj");
    }
}
