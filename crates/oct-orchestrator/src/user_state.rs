use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Service {
    pub public_ip: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub(crate) struct UserState {
    pub services: HashMap<String, Service>,
}

#[cfg(test)]
mod tests {
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
}
