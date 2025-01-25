use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct UserState {
    pub service_name: String,
    pub public_ip: String,
}

impl UserState {
    pub(crate) fn new(service_name: String, public_ip: String) -> Self {
        Self {
            service_name,
            public_ip,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_state() {
        let user_state = UserState::new("test".to_string(), "127.0.0.1".to_string());
        assert_eq!(user_state.service_name, "test");
        assert_eq!(user_state.public_ip, "127.0.0.1");
    }
}
