use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct UserState {
    pub service_name: String,
    pub public_ip: String,
}

impl UserState {
    pub fn new(name: String, public_ip: String) -> Self {
        Self {
            service_name: name,
            public_ip: public_ip,
        }
    }
}
