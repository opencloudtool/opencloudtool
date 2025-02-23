use std::collections::HashMap;
use std::process::Command;

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
}

#[cfg_attr(test, allow(dead_code))]
impl ContainerEngine {
    const NETWORK_NAME: &str = "oct";

    /// Runs container using `podman`
    pub(crate) fn run(
        &self,
        name: &str,
        image: &str,
        external_port: Option<u32>,
        internal_port: Option<u32>,
        cpus: u32,
        memory: u64,
        envs: &HashMap<String, String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let network_create_cmd = Command::new(self.manager.as_str())
            .args(["network", "create", Self::NETWORK_NAME])
            .output()?;

        log::info!(
            "Network create command output: status={:?}, stdout={:?}, stderr={:?}",
            network_create_cmd.status,
            network_create_cmd.stdout,
            network_create_cmd.stderr
        );

        let run_container_args = Self::build_run_container_args(
            name,
            image,
            external_port,
            internal_port,
            cpus,
            memory,
            envs,
        );

        let run_container_cmd = Command::new(self.manager.as_str())
            .args(&run_container_args)
            .output()?;

        log::info!(
            "Run container command output: status={:?}, stdout={:?}, stderr={:?}",
            run_container_cmd.status,
            run_container_cmd.stdout,
            run_container_cmd.stderr
        );

        Ok(())
    }

    /// Removes container
    pub(crate) fn remove(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let command = Command::new(self.manager.as_str())
            .args(["rm", "-f", name])
            .output();

        match command {
            Ok(_) => Ok(()),
            Err(err) => Err(Box::new(err)),
        }
    }

    fn build_run_container_args(
        name: &str,
        image: &str,
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
            name.to_string(),
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

        run_container_args.push(image.to_string());

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
                name: &str,
                image: &str,
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
