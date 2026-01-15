use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Default, Eq, PartialEq)]
pub struct UserState {
    /// Key - public IP, Value - instance
    pub instances: HashMap<String, Instance>,
}

#[derive(Serialize, Deserialize, Debug, Default, Eq, PartialEq)]
pub struct Instance {
    /// CPUs available on instance
    pub cpus: u32,
    /// Memory available on instance
    pub memory: u64,

    /// Services running on instance
    pub services: HashMap<String, oct_config::Service>,
}

impl Instance {
    /// Gets cpus and memory available on instance
    pub fn get_available_resources(&self) -> (u32, u64) {
        let available_cpus = self.cpus - self.services.values().map(|s| s.cpus).sum::<u32>();
        let available_memory = self.memory - self.services.values().map(|s| s.memory).sum::<u64>();

        (available_cpus, available_memory)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instance_get_available_resources() {
        let instance = Instance {
            cpus: 1000,
            memory: 1024,
            services: HashMap::from([
                (
                    "app_1".to_string(),
                    oct_config::Service {
                        name: "app_1".to_string(),
                        image: "nginx:latest".to_string(),
                        dockerfile_path: None,
                        command: None,
                        internal_port: None,
                        external_port: None,
                        cpus: 500,
                        memory: 512,
                        depends_on: vec![],
                        envs: HashMap::new(),
                    },
                ),
                (
                    "app_2".to_string(),
                    oct_config::Service {
                        name: "app_2".to_string(),
                        image: "nginx:latest".to_string(),
                        dockerfile_path: None,
                        command: None,
                        internal_port: None,
                        external_port: None,
                        cpus: 250,
                        memory: 256,
                        depends_on: vec![],
                        envs: HashMap::new(),
                    },
                ),
            ]),
        };

        assert_eq!(instance.get_available_resources(), (250, 256));
    }
}
