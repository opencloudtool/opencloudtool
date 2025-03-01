use std::fs;

use crate::state;

pub trait StateBackend {
    /// Saves state to a backend
    fn save(&self, state: &state::State) -> Result<(), Box<dyn std::error::Error>>;

    /// Loads state from a backend or initialize a new one
    /// Also returns whether the state was loaded as a boolean
    fn load(&self) -> Result<(state::State, bool), Box<dyn std::error::Error>>;
}

pub struct LocalStateBackend {
    file_path: String,
}

impl LocalStateBackend {
    pub fn new(file_path: &str) -> Self {
        LocalStateBackend {
            file_path: file_path.to_string(),
        }
    }
}

impl StateBackend for LocalStateBackend {
    fn save(&self, state: &state::State) -> Result<(), Box<dyn std::error::Error>> {
        fs::write(&self.file_path, serde_json::to_string_pretty(state)?)?;

        Ok(())
    }

    fn load(&self) -> Result<(state::State, bool), Box<dyn std::error::Error>> {
        if std::path::Path::new(&self.file_path).exists() {
            let existing_data = fs::read_to_string(&self.file_path)?;
            Ok((serde_json::from_str::<state::State>(&existing_data)?, true))
        } else {
            Ok((state::State::default(), false))
        }
    }
}

#[allow(dead_code)]
pub struct S3StateBackend {
    region: String,
    bucket: String,
}

impl S3StateBackend {
    pub fn new(region: &str, bucket: &str) -> Self {
        S3StateBackend {
            region: region.to_string(),
            bucket: bucket.to_string(),
        }
    }
}

impl StateBackend for S3StateBackend {
    fn save(&self, _state: &state::State) -> Result<(), Box<dyn std::error::Error>> {
        todo!()
    }

    fn load(&self) -> Result<(state::State, bool), Box<dyn std::error::Error>> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Write;

    #[test]
    fn test_state_new_exists() {
        // Arrange
        let state_file_content = r#"
{
    "vpc": {    
        "id": "id",
        "region": "region",
        "cidr_block": "test_cidr_block",
        "name": "name",
        "subnet": {
            "id": "id",
            "region": "region",
            "cidr_block": "test_cidr_block",
            "availability_zone": "availability_zone",
            "vpc_id": "vpc_id",
            "name": "name"
        },
        "internet_gateway": null,
        "route_table": {
            "id": "id",
            "vpc_id": "vpc_id",
            "subnet_id": "subnet_id",
            "region": "region"
        },
        "security_group": {
            "id": "id",
            "vpc_id": "vpc_id",
            "name": "name",
            "description": "description",
            "region": "region",
            "inbound_rules": [
            {
                "protocol": "tcp",
                "port": 0,
                "cidr_block": "cidr_block"
            }
            ]
        }
    },
    "ecr": {
        "name": "name",
        "region": "region",
        "id": "id"
    },
    "instance_profile": {
        "name": "instance_profile_name",
        "region": "region",
        "instance_roles": [
        {
            "name": "instance_role_name",
            "region": "region",
            "assume_role_policy": "assume_role_policy",
            "policy_arns": [
                "policy_arn"
            ]
        }
        ]
    },
    "instances": [
    {
        "id": "id",
        "public_ip": "public_ip",
        "public_dns": "public_dns",
        "region": "region",
        "ami": "ami",
        "instance_type": "t2.micro",
        "name": "name",
        "instance_profile_name": "instance_profile_name",
        "subnet_id": "subnet_id",
        "security_group_id": "security_group_id",
        "user_data": "user_data"
    }]
}"#;

        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(state_file_content.as_bytes()).unwrap();

        let state_backend = LocalStateBackend::new(file.path().to_str().unwrap());

        // Act
        let (state, loaded) = state_backend.load().unwrap();

        // Assert
        assert!(loaded);
        assert_eq!(
            state,
            state::State {
                vpc: state::VPCState {
                    id: "id".to_string(),
                    region: "region".to_string(),
                    cidr_block: "test_cidr_block".to_string(),
                    name: "name".to_string(),
                    subnet: state::SubnetState {
                        id: "id".to_string(),
                        region: "region".to_string(),
                        cidr_block: "test_cidr_block".to_string(),
                        availability_zone: "availability_zone".to_string(),
                        vpc_id: "vpc_id".to_string(),
                        name: "name".to_string(),
                    },
                    internet_gateway: None,
                    route_table: state::RouteTableState {
                        id: "id".to_string(),
                        vpc_id: "vpc_id".to_string(),
                        subnet_id: "subnet_id".to_string(),
                        region: "region".to_string(),
                    },
                    security_group: state::SecurityGroupState {
                        id: "id".to_string(),
                        vpc_id: "vpc_id".to_string(),
                        name: "name".to_string(),
                        description: "description".to_string(),
                        region: "region".to_string(),
                        inbound_rules: vec![state::InboundRuleState {
                            protocol: "tcp".to_string(),
                            port: 0,
                            cidr_block: "cidr_block".to_string(),
                        }],
                    },
                },
                ecr: state::ECRState {
                    id: "id".to_string(),
                    name: "name".to_string(),
                    region: "region".to_string(),
                },
                instance_profile: state::InstanceProfileState {
                    name: "instance_profile_name".to_string(),
                    region: "region".to_string(),
                    instance_roles: vec![state::InstanceRoleState {
                        name: "instance_role_name".to_string(),
                        region: "region".to_string(),
                        assume_role_policy: "assume_role_policy".to_string(),
                        policy_arns: vec!["policy_arn".to_string()],
                    }],
                },
                instances: vec![state::Ec2InstanceState {
                    id: "id".to_string(),
                    public_ip: "public_ip".to_string(),
                    public_dns: "public_dns".to_string(),
                    region: "region".to_string(),
                    ami: "ami".to_string(),
                    instance_type: "t2.micro".to_string(),
                    name: "name".to_string(),
                    instance_profile_name: "instance_profile_name".to_string(),
                    subnet_id: "subnet_id".to_string(),
                    security_group_id: "security_group_id".to_string(),
                    user_data: "user_data".to_string(),
                }],
            }
        )
    }

    #[test]
    fn test_state_new_not_exists() {
        // Arrange
        let state_backend = LocalStateBackend::new("NO_FILE");

        // Act
        let (state, loaded) = state_backend.load().unwrap();

        // Assert
        assert_eq!(state.instances.len(), 0);
        assert!(!loaded);
    }

    #[test]
    fn test_local_state_backend_save() {
        // Arrange
        let state = state::State {
            vpc: state::VPCState {
                id: "id".to_string(),
                region: "region".to_string(),
                cidr_block: "test_cidr_block".to_string(),
                name: "name".to_string(),
                subnet: state::SubnetState {
                    id: "id".to_string(),
                    region: "region".to_string(),
                    cidr_block: "test_cidr_block".to_string(),
                    availability_zone: "availability_zone".to_string(),
                    vpc_id: "vpc_id".to_string(),
                    name: "name".to_string(),
                },
                internet_gateway: None,
                route_table: state::RouteTableState {
                    id: "id".to_string(),
                    vpc_id: "vpc_id".to_string(),
                    subnet_id: "subnet_id".to_string(),
                    region: "region".to_string(),
                },
                security_group: state::SecurityGroupState {
                    id: "id".to_string(),
                    vpc_id: "vpc_id".to_string(),
                    name: "name".to_string(),
                    description: "description".to_string(),
                    region: "region".to_string(),
                    inbound_rules: vec![state::InboundRuleState {
                        protocol: "tcp".to_string(),
                        port: 0,
                        cidr_block: "cidr_block".to_string(),
                    }],
                },
            },
            ecr: state::ECRState {
                id: "id".to_string(),
                name: "name".to_string(),
                region: "region".to_string(),
            },
            instance_profile: state::InstanceProfileState {
                name: "instance_profile_name".to_string(),
                region: "region".to_string(),
                instance_roles: vec![state::InstanceRoleState {
                    name: "instance_role_name".to_string(),
                    region: "region".to_string(),
                    assume_role_policy: "assume_role_policy".to_string(),
                    policy_arns: vec!["policy_arn".to_string()],
                }],
            },
            instances: vec![state::Ec2InstanceState {
                id: "id".to_string(),
                public_ip: "public_ip".to_string(),
                public_dns: "public_dns".to_string(),
                region: "region".to_string(),
                ami: "ami".to_string(),
                instance_type: "t2.micro".to_string(),
                name: "name".to_string(),
                instance_profile_name: "instance_profile_name".to_string(),
                subnet_id: "subnet_id".to_string(),
                security_group_id: "security_group_id".to_string(),
                user_data: "user_data".to_string(),
            }],
        };

        let state_file = tempfile::NamedTempFile::new().unwrap();
        let state_file_path = state_file.path().to_str().unwrap();

        let state_backend = LocalStateBackend::new(state_file_path);

        // Act
        state_backend.save(&state).unwrap();

        // Assert
        let file_content = fs::read_to_string(state_file_path).unwrap();

        assert_eq!(
            file_content,
            r#"{
  "vpc": {
    "id": "id",
    "region": "region",
    "cidr_block": "test_cidr_block",
    "name": "name",
    "subnet": {
      "id": "id",
      "region": "region",
      "cidr_block": "test_cidr_block",
      "availability_zone": "availability_zone",
      "vpc_id": "vpc_id",
      "name": "name"
    },
    "internet_gateway": null,
    "route_table": {
      "id": "id",
      "vpc_id": "vpc_id",
      "subnet_id": "subnet_id",
      "region": "region"
    },
    "security_group": {
      "id": "id",
      "vpc_id": "vpc_id",
      "name": "name",
      "description": "description",
      "region": "region",
      "inbound_rules": [
        {
          "protocol": "tcp",
          "port": 0,
          "cidr_block": "cidr_block"
        }
      ]
    }
  },
  "ecr": {
    "id": "id",
    "name": "name",
    "region": "region"
  },
  "instance_profile": {
    "name": "instance_profile_name",
    "region": "region",
    "instance_roles": [
      {
        "name": "instance_role_name",
        "region": "region",
        "assume_role_policy": "assume_role_policy",
        "policy_arns": [
          "policy_arn"
        ]
      }
    ]
  },
  "instances": [
    {
      "id": "id",
      "public_ip": "public_ip",
      "public_dns": "public_dns",
      "region": "region",
      "ami": "ami",
      "instance_type": "t2.micro",
      "name": "name",
      "instance_profile_name": "instance_profile_name",
      "subnet_id": "subnet_id",
      "security_group_id": "security_group_id",
      "user_data": "user_data"
    }
  ]
}"#
        );
    }

    #[test]
    fn test_s3_backend_new() {
        let state_backend = S3StateBackend::new("region", "bucket");

        assert_eq!(state_backend.region, "region");
        assert_eq!(state_backend.bucket, "bucket");
    }

    #[test]
    #[should_panic(expected = "not yet implemented")]
    fn test_s3_backend_save() {
        let state_backend = S3StateBackend::new("region", "bucket");

        let state = state::State::default();

        state_backend.save(&state).unwrap();
    }

    #[test]
    #[should_panic(expected = "not yet implemented")]
    fn test_s3_backend_load() {
        let state_backend = S3StateBackend::new("region", "bucket");

        state_backend.load().unwrap();
    }
}
