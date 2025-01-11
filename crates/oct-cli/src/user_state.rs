use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct UserState {
    pub service_name: String,
    pub public_ip: String,
}

impl UserState {
    pub(crate) fn new(name: String, public_ip: String) -> Self {
        Self {
            service_name: name,
            public_ip: public_ip,
        }
    }
}
