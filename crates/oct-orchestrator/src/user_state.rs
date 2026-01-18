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
