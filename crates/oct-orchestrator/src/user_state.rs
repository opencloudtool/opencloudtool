use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::config;

#[derive(Serialize, Deserialize, Debug, Default, Eq, PartialEq)]
pub(crate) struct UserState {
    /// Key - public IP, Value - instance
    pub(crate) instances: HashMap<String, Instance>,
}

impl UserState {
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
    pub(crate) services: HashMap<String, config::Service>,
}

impl Instance {
    /// Gets cpus and memory available on instance
    pub(crate) fn get_available_resources(&self) -> (u32, u64) {
        let available_cpus = self.cpus - self.services.values().map(|s| s.cpus).sum::<u32>();
        let available_memory = self.memory - self.services.values().map(|s| s.memory).sum::<u64>();

        (available_cpus, available_memory)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_state_get_services_context() {
        let user_state = UserState {
            instances: HashMap::from([
                (
                    "1.2.3.4".to_string(),
                    Instance {
                        cpus: 1000,
                        memory: 1024,
                        services: HashMap::from([(
                            "app_1".to_string(),
                            config::Service {
                                name: "app_1".to_string(),
                                image: "nginx:latest".to_string(),
                                dockerfile_path: None,
                                command: None,
                                internal_port: None,
                                external_port: None,
                                cpus: 1000,
                                memory: 1024,
                                depends_on: vec![],
                                envs: HashMap::new(),
                            },
                        )]),
                    },
                ),
                (
                    "2.3.4.5".to_string(),
                    Instance {
                        cpus: 1000,
                        memory: 1024,
                        services: HashMap::from([(
                            "app_2".to_string(),
                            config::Service {
                                name: "app_2".to_string(),
                                image: "nginx:latest".to_string(),
                                dockerfile_path: None,
                                command: None,
                                internal_port: None,
                                external_port: None,
                                cpus: 1000,
                                memory: 1024,
                                depends_on: vec![],
                                envs: HashMap::new(),
                            },
                        )]),
                    },
                ),
            ]),
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
                        public_ip: "2.3.4.5".to_string()
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
                    "app_1".to_string(),
                    config::Service {
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
                    config::Service {
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
