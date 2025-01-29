use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Service {
    pub name: String,
    pub public_ip: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub(crate) struct UserState {
    pub services: Vec<Service>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_state() {
        let user_state = UserState {
            services: vec![
                Service {
                    name: "test".to_string(),
                    public_ip: "test".to_string(),
                },
                Service {
                    name: "test2".to_string(),
                    public_ip: "test2".to_string(),
                },
            ],
        };
        assert_eq!(user_state.services.len(), 2);
        assert_eq!(user_state.services[0].name, "test");
        assert_eq!(user_state.services[0].public_ip, "test");
        assert_eq!(user_state.services[1].name, "test2");
        assert_eq!(user_state.services[1].public_ip, "test2");
    }
}
