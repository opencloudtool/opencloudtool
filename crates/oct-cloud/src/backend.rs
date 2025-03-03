use std::fs;

use crate::aws::resource::S3Bucket;
use crate::resource::Resource;
use crate::state;

#[async_trait::async_trait]
pub trait StateBackend {
    /// Saves state to a backend
    async fn save(&self, state: &state::State) -> Result<(), Box<dyn std::error::Error>>;

    /// Loads state from a backend or initialize a new one
    /// Also returns whether the state was loaded as a boolean
    async fn load(&self) -> Result<(state::State, bool), Box<dyn std::error::Error>>;

    /// Removes state file from a backend
    async fn remove(&self) -> Result<(), Box<dyn std::error::Error>>;
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

#[async_trait::async_trait]
impl StateBackend for LocalStateBackend {
    async fn save(&self, state: &state::State) -> Result<(), Box<dyn std::error::Error>> {
        fs::write(&self.file_path, serde_json::to_string_pretty(state)?)?;

        Ok(())
    }

    async fn load(&self) -> Result<(state::State, bool), Box<dyn std::error::Error>> {
        if std::path::Path::new(&self.file_path).exists() {
            let existing_data = fs::read_to_string(&self.file_path)?;
            Ok((serde_json::from_str::<state::State>(&existing_data)?, true))
        } else {
            Ok((state::State::default(), false))
        }
    }

    async fn remove(&self) -> Result<(), Box<dyn std::error::Error>> {
        fs::remove_file(&self.file_path)?;

        Ok(())
    }
}

#[allow(dead_code)]
pub struct S3StateBackend {
    region: String,
    bucket: String,
    key: String,
}

impl S3StateBackend {
    pub fn new(region: &str, bucket: &str, key: &str) -> Self {
        S3StateBackend {
            region: region.to_string(),
            bucket: bucket.to_string(),
            key: key.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl StateBackend for S3StateBackend {
    async fn save(&self, state: &state::State) -> Result<(), Box<dyn std::error::Error>> {
        let mut s3_bucket = S3Bucket::new(self.region.clone(), self.bucket.clone()).await;
        s3_bucket.create().await?;

        s3_bucket
            .put_object(&self.key, serde_json::to_vec(state)?)
            .await?;

        Ok(())
    }

    async fn load(&self) -> Result<(state::State, bool), Box<dyn std::error::Error>> {
        let s3_bucket = S3Bucket::new(self.region.clone(), self.bucket.clone()).await;

        let data = s3_bucket.get_object(&self.key).await;

        match data {
            Ok(data) => Ok((serde_json::from_slice(&data)?, true)),
            Err(_) => Ok((state::State::default(), false)),
        }
    }

    async fn remove(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut s3_bucket = S3Bucket::new(self.region.clone(), self.bucket.clone()).await;

        // For now we expect to have only one file in the bucket
        // If there are multiple files, the state is corrupted and bucket
        // will not be deleted
        s3_bucket.delete_object(&self.key).await?;

        s3_bucket.destroy().await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Write;

    use crate::aws::types::RecordType;

    #[tokio::test]
    async fn test_state_new_exists() {
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
        "url": "url",
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
    }
      ],
  "hosted_zone": {
    "id": "id",
    "dns_record_sets": [
      {
        "name": "name",
        "record_type": "A",
        "records": [
          "records"
        ],
        "ttl": 300
      }
    ],
    "name": "name",
    "region": "region"
  }
}"#;

        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(state_file_content.as_bytes()).unwrap();

        let state_backend = LocalStateBackend::new(file.path().to_str().unwrap());

        // Act
        let (state, loaded) = state_backend.load().await.unwrap();

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
                    url: "url".to_string(),
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
                hosted_zone: Some(state::HostedZoneState {
                    id: "id".to_string(),
                    dns_record_sets: vec![state::DNSRecordSetState {
                        name: "name".to_string(),
                        record_type: RecordType::A.as_str().to_string(),
                        records: Some(vec!["records".to_string()]),
                        ttl: Some(300),
                    }],
                    name: "name".to_string(),
                    region: "region".to_string(),
                }),
            }
        )
    }

    #[tokio::test]
    async fn test_state_new_not_exists() {
        // Arrange
        let state_backend = LocalStateBackend::new("NO_FILE");

        // Act
        let (state, loaded) = state_backend.load().await.unwrap();

        // Assert
        assert_eq!(state.instances.len(), 0);
        assert!(!loaded);
    }

    #[tokio::test]
    async fn test_local_state_backend_save() {
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
                url: "url".to_string(),
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
            hosted_zone: Some(state::HostedZoneState {
                id: "id".to_string(),
                dns_record_sets: vec![state::DNSRecordSetState {
                    name: "name".to_string(),
                    record_type: RecordType::A.as_str().to_string(),
                    records: Some(vec!["records".to_string()]),
                    ttl: Some(300),
                }],
                name: "name".to_string(),
                region: "region".to_string(),
            }),
        };

        let state_file = tempfile::NamedTempFile::new().unwrap();
        let state_file_path = state_file.path().to_str().unwrap();

        let state_backend = LocalStateBackend::new(state_file_path);

        // Act
        state_backend.save(&state).await.unwrap();

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
    "url": "url",
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
  ],
  "hosted_zone": {
    "id": "id",
    "dns_record_sets": [
      {
        "name": "name",
        "record_type": "A",
        "records": [
          "records"
        ],
        "ttl": 300
      }
    ],
    "name": "name",
    "region": "region"
  }
}"#
        );
    }

    #[test]
    fn test_s3_backend_new() {
        let state_backend = S3StateBackend::new("region", "bucket", "key");

        assert_eq!(state_backend.region, "region");
        assert_eq!(state_backend.bucket, "bucket");
    }

    #[tokio::test]
    #[ignore = "Requires AWS setup"]
    async fn test_s3_backend_save() {
        let state_backend = S3StateBackend::new("region", "bucket", "key");

        let state = state::State::default();

        state_backend.save(&state).await.unwrap();
    }

    #[tokio::test]
    #[ignore = "Requires AWS setup"]
    async fn test_s3_backend_load() {
        let state_backend = S3StateBackend::new("region", "bucket", "key");

        state_backend.load().await.unwrap();
    }
}
