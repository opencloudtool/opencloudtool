use std::collections::HashMap;
use std::process::Command;

#[cfg(not(test))]
use crate::executor::CommandExecutor;
#[cfg(test)]
use crate::executor::mocks::MockCommandExecutor as CommandExecutor;

/// Container manager options
#[derive(Clone, Default)]
enum ContainerManager {
    #[default]
    Podman,
}

impl ContainerManager {
    fn as_str(&self) -> &'static str {
        match self {
            ContainerManager::Podman => "podman",
        }
    }
}

/// Container engine implementation
#[derive(Clone, Default)]
pub(crate) struct ContainerEngine {
    manager: ContainerManager,
    executor: CommandExecutor,
}

#[cfg_attr(test, allow(dead_code))]
impl ContainerEngine {
    const NETWORK_NAME: &str = "oct";

    /// Runs container using `podman`
    pub(crate) fn run(
        &self,
        name: String,
        image: String,
        command: Option<String>,
        external_port: Option<u32>,
        internal_port: Option<u32>,
        cpus: u32,
        memory: u64,
        envs: &HashMap<String, String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // We accept errors here, as network might already exist
        let network_create_output =
            self.executor
                .execute(Command::new(self.manager.as_str()).args([
                    "network",
                    "create",
                    Self::NETWORK_NAME,
                ]))?;

        log::info!("Network create command output: {network_create_output:?}");

        let run_container_args = Self::build_run_container_args(
            name,
            image,
            command,
            external_port,
            internal_port,
            cpus,
            memory,
            envs,
        );

        let run_container_cmd = self
            .executor
            .execute(Command::new(self.manager.as_str()).args(&run_container_args))?;

        log::info!("Run container command output: {run_container_cmd:?}");

        if run_container_cmd.status.success() {
            Ok(())
        } else {
            Err("Failed to run container".into())
        }
    }

    /// Removes container
    pub(crate) fn remove(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let output = self
            .executor
            .execute(Command::new(self.manager.as_str()).args(["rm", "-f", name]))?;

        if output.status.success() {
            Ok(())
        } else {
            Err("Failed to remove container".into())
        }
    }

    fn build_run_container_args(
        name: String,
        image: String,
        command: Option<String>,
        external_port: Option<u32>,
        internal_port: Option<u32>,
        cpus: u32,
        memory: u64,
        envs: &HashMap<String, String>,
    ) -> Vec<String> {
        let cpus = f64::from(cpus) / 1000.0; // Convert millicores to cores
        let cpus_str = format!("{cpus:.2}");
        let memory_str = format!("{memory}m");

        let mut run_container_args = vec![
            "run".to_string(),
            "--restart".to_string(),
            "always".to_string(),
            "-d".to_string(),
            "--name".to_string(),
            name,
            "--cpus".to_string(),
            cpus_str,
            "--memory".to_string(),
            memory_str,
            "--network".to_string(),
            Self::NETWORK_NAME.to_string(),
        ];

        if let (Some(external_port), Some(internal_port)) = (external_port, internal_port) {
            let port_mapping = format!("{external_port}:{internal_port}");

            run_container_args.push("-p".to_string());
            run_container_args.push(port_mapping);
        }

        for (key, value) in envs {
            let env_str = format!("{key}={value}");
            run_container_args.push("-e".to_string());
            run_container_args.push(env_str);
        }

        run_container_args.push(image);

        if let Some(command) = command {
            run_container_args.push(command);
        }

        run_container_args
    }
}

#[cfg(test)]
pub(crate) mod mocks {
    use std::collections::HashMap;

    use mockall::mock;

    // As long as ContainerEngine implemnts Clone, we mock it using
    // mockall::mock macro, more info here:
    // https://docs.rs/mockall/latest/mockall/macro.mock.html#examples
    mock! {
        pub(crate) ContainerEngine {
            pub(crate) fn run(
                &self,
                name: String,
                image: String,
                command: Option<String>,
                external_port: Option<u32>,
                internal_port: Option<u32>,
                cpus: u32,
                memory: u64,
                envs: &HashMap<String, String>,
            ) -> Result<(), Box<dyn std::error::Error>>;

            pub(crate) fn remove(&self, name: &str) -> Result<(), Box<dyn std::error::Error>>;
        }

        impl Clone for ContainerEngine {
            fn clone(&self) -> Self;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::process::ExitStatusExt;
    use std::process::{ExitStatus, Output};

    use super::*;

    fn get_command_executor_mock(exit_code: i32) -> CommandExecutor {
        let mut mock_command_executor = CommandExecutor::default();
        mock_command_executor.expect_execute().returning(move |_| {
            Ok(Output {
                status: ExitStatus::from_raw(exit_code),
                stdout: Vec::new(),
                stderr: Vec::new(),
            })
        });

        mock_command_executor
    }

    #[test]
    fn test_container_engine_run_success() {
        // Arrange
        let mock_command_executor = get_command_executor_mock(0);

        let container_engine = ContainerEngine {
            manager: ContainerManager::Podman,
            executor: mock_command_executor,
        };

        // Act
        let run_result = container_engine.run(
            "test".to_string(),
            "ubuntu:latest".to_string(),
            Some("echo hello".to_string()),
            Some(80),
            Some(8080),
            1,
            512,
            &HashMap::from([("KEY".to_string(), "VALUE".to_string())]),
        );

        // Assert
        assert!(run_result.is_ok());
    }

    #[test]
    fn test_container_engine_run_failure() {
        // Arrange
        let mock_command_executor = get_command_executor_mock(1);

        let container_engine = ContainerEngine {
            manager: ContainerManager::Podman,
            executor: mock_command_executor,
        };

        // Act
        let run_result = container_engine.run(
            "test".to_string(),
            "ubuntu:latest".to_string(),
            Some("echo hello".to_string()),
            Some(80),
            Some(8080),
            1,
            512,
            &HashMap::new(),
        );

        // Assert
        assert!(run_result.is_err());
    }

    #[test]
    fn test_container_engine_remove_success() {
        // Arrange
        let mock_command_executor = get_command_executor_mock(0);

        let container_engine = ContainerEngine {
            manager: ContainerManager::Podman,
            executor: mock_command_executor,
        };

        // Act
        let remove_result = container_engine.remove("test");

        // Assert
        assert!(remove_result.is_ok());
    }

    #[test]
    fn test_container_engine_remove_failure() {
        // Arrange
        let mock_command_executor = get_command_executor_mock(1);

        let container_engine = ContainerEngine {
            manager: ContainerManager::Podman,
            executor: mock_command_executor,
        };

        // Act
        let remove_result = container_engine.remove("test");

        // Assert
        assert!(remove_result.is_err());
    }

    #[test]
    fn test_container_manager_as_str() {
        let container_manager = ContainerManager::Podman;

        assert_eq!(container_manager.as_str(), "podman");
    }
}
