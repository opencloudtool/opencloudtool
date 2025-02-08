use serde::{Deserialize, Serialize};

use crate::aws::types::InstanceType;

#[derive(Debug, Serialize, Deserialize)]
pub struct Ec2InstanceState {
    pub id: String,
    pub public_ip: String,
    pub public_dns: String,
    pub region: String,
    pub ami: String,
    pub instance_type: String,
    pub name: String,
    pub vpc: VPCState,
    // TODO: Make instance_profile required
    pub instance_profile: Option<InstanceProfileState>,
}

#[cfg(test)]
mod mocks {
    use crate::aws::types::InstanceType;

    pub struct MockEc2Instance {
        pub id: Option<String>,
        pub public_ip: Option<String>,
        pub public_dns: Option<String>,
        pub region: String,
        pub ami: String,
        pub instance_type: InstanceType,
        pub name: String,
        pub vpc: MockVPC,
        pub instance_profile: Option<MockInstanceProfile>,
    }

    impl MockEc2Instance {
        pub async fn new(
            id: Option<String>,
            public_ip: Option<String>,
            public_dns: Option<String>,
            region: String,
            ami: String,
            instance_type: InstanceType,
            name: String,
            vpc: MockVPC,
            instance_profile: Option<MockInstanceProfile>,
        ) -> Self {
            Self {
                id,
                public_ip,
                public_dns,
                region,
                ami,
                instance_type,
                name,
                vpc,
                instance_profile,
            }
        }
    }

    pub struct MockInstanceProfile {
        pub name: String,
        pub region: String,
        pub instance_roles: Vec<MockInstanceRole>,
    }

    impl MockInstanceProfile {
        pub async fn new(region: String, instance_roles: Vec<MockInstanceRole>) -> Self {
            Self {
                name: "test_name".to_string(),
                region,
                instance_roles,
            }
        }
    }

    pub struct MockInstanceRole {
        pub name: String,
        pub region: String,
        pub assume_role_policy: String,
        pub policy_arns: Vec<String>,
    }

    impl MockInstanceRole {
        pub async fn new(region: String) -> Self {
            Self {
                name: "test_name".to_string(),
                region,
                assume_role_policy: "test_assume_role_policy".to_string(),
                policy_arns: vec!["test_policy_arn".to_string()],
            }
        }
    }

    pub struct MockVPC {
        pub id: Option<String>,
        pub region: String,
        pub cidr_block: String,
        pub name: String,
        pub subnet: MockSubnet,
    }

    impl MockVPC {
        pub async fn new(
            id: Option<String>,
            region: String,
            name: String,
            subnet: MockSubnet,
        ) -> Self {
            Self {
                id,
                region,
                cidr_block: "test_cidr_block".to_string(),
                name,
                subnet,
            }
        }
    }

    pub struct MockSubnet {
        pub id: Option<String>,
        pub region: String,
        pub cidr_block: String,
        pub vpc_id: Option<String>,
        pub name: String,
    }

    impl MockSubnet {
        pub async fn new(
            id: Option<String>,
            region: String,
            cidr_block: String,
            vpc_id: Option<String>,
            name: String,
        ) -> Self {
            Self {
                id,
                region,
                cidr_block,
                name,
                vpc_id,
            }
        }
    }
}

#[cfg(not(test))]
use crate::aws::resource::{Ec2Instance, InstanceProfile, InstanceRole, Subnet, VPC};

#[cfg(test)]
use mocks::{
    MockEc2Instance as Ec2Instance, MockInstanceProfile as InstanceProfile,
    MockInstanceRole as InstanceRole, MockSubnet as Subnet, MockVPC as VPC,
};

impl Ec2InstanceState {
    pub fn new(ec2_instance: &Ec2Instance) -> Self {
        Self {
            id: ec2_instance.id.clone().expect("Instance id is not set"),
            public_ip: ec2_instance
                .public_ip
                .clone()
                .expect("Public ip is not set"),
            public_dns: ec2_instance
                .public_dns
                .clone()
                .expect("Public dns is not set"),
            region: ec2_instance.region.clone(),
            ami: ec2_instance.ami.clone(),
            instance_type: ec2_instance.instance_type.name.to_string(),
            name: ec2_instance.name.clone(),
            vpc: VPCState::new(&ec2_instance.vpc),
            instance_profile: ec2_instance
                .instance_profile
                .as_ref()
                .map(InstanceProfileState::new),
        }
    }

    pub async fn new_from_state(&self) -> Result<Ec2Instance, Box<dyn std::error::Error>> {
        let instance_profile = match &self.instance_profile {
            Some(profile) => profile.new_from_state().await,
            None => return Err("Instance profile is not set".into()),
        };

        let vpc = self.vpc.new_from_state().await;

        Ok(Ec2Instance::new(
            Some(self.id.clone()),
            Some(self.public_ip.clone()),
            Some(self.public_dns.clone()),
            self.region.clone(),
            self.ami.clone(),
            InstanceType::from(self.instance_type.as_str()),
            self.name.clone(),
            vpc,
            Some(instance_profile),
        )
        .await)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InstanceProfileState {
    pub name: String,
    pub region: String,
    pub instance_roles: Vec<InstanceRoleState>,
}

impl InstanceProfileState {
    pub fn new(instance_profile: &InstanceProfile) -> Self {
        Self {
            name: instance_profile.name.clone(),
            region: instance_profile.region.clone(),
            instance_roles: instance_profile
                .instance_roles
                .iter()
                .map(InstanceRoleState::new)
                .collect(),
        }
    }

    pub async fn new_from_state(&self) -> InstanceProfile {
        let mut roles = vec![];
        for role in &self.instance_roles {
            roles.push(role.new_from_state().await);
        }

        InstanceProfile::new(self.region.clone(), roles).await
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InstanceRoleState {
    pub name: String,
    pub region: String,
    pub assume_role_policy: String,
    pub policy_arns: Vec<String>,
}

impl InstanceRoleState {
    pub fn new(instance_role: &InstanceRole) -> Self {
        Self {
            name: instance_role.name.clone(),
            region: instance_role.region.clone(),
            assume_role_policy: instance_role.assume_role_policy.clone(),
            policy_arns: instance_role.policy_arns.clone(),
        }
    }

    pub async fn new_from_state(&self) -> InstanceRole {
        InstanceRole::new(self.region.clone()).await
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VPCState {
    pub id: String,
    pub region: String,
    pub cidr_block: String,
    pub name: String,
    pub subnet: SubnetState,
}

impl VPCState {
    pub fn new(vpc: &VPC) -> Self {
        Self {
            id: vpc.id.clone().expect("VPC id not set"),
            region: vpc.region.clone(),
            cidr_block: vpc.cidr_block.clone(),
            name: vpc.name.clone(),
            subnet: SubnetState::new(&vpc.subnet),
        }
    }

    pub async fn new_from_state(&self) -> VPC {
        let subnet = self.subnet.new_from_state().await;

        VPC::new(
            Some(self.id.clone()),
            self.region.clone(),
            self.name.clone(),
            subnet,
        )
        .await
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SubnetState {
    pub id: String,
    pub region: String,
    pub cidr_block: String,
    pub vpc_id: String,
    pub name: String,
}

impl SubnetState {
    pub fn new(subnet: &Subnet) -> Self {
        Self {
            id: subnet.id.clone().expect("Subnet id not set"),
            region: subnet.region.clone(),
            cidr_block: subnet.cidr_block.clone(),
            vpc_id: subnet.vpc_id.clone().expect("vpc id not set"),
            name: subnet.name.clone(),
        }
    }

    pub async fn new_from_state(&self) -> Subnet {
        Subnet::new(
            Some(self.id.clone()),
            self.region.clone(),
            self.cidr_block.clone(),
            Some(self.vpc_id.clone()),
            self.name.clone(),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ec2_instance_state() {
        let ec2_instance = Ec2Instance::new(
            Some("id".to_string()),
            Some("public_ip".to_string()),
            Some("public_dns".to_string()),
            "region".to_string(),
            "ami".to_string(),
            InstanceType::T2_MICRO,
            "name".to_string(),
            VPC {
                id: Some("id".to_string()),
                region: "region".to_string(),
                cidr_block: "cidr_block".to_string(),
                name: "name".to_string(),
                subnet: Subnet {
                    id: Some("id".to_string()),
                    region: "region".to_string(),
                    cidr_block: "cidr_block".to_string(),
                    vpc_id: Some("vpc_id".to_string()),
                    name: "name".to_string(),
                },
            },
            None,
        )
        .await;

        let ec2_instance_state = Ec2InstanceState::new(&ec2_instance);

        assert_eq!(ec2_instance_state.id, "id");
        assert_eq!(ec2_instance_state.public_ip, "public_ip");
        assert_eq!(ec2_instance_state.public_dns, "public_dns");
        assert_eq!(ec2_instance_state.region, "region");
        assert_eq!(ec2_instance_state.ami, "ami");
        assert_eq!(ec2_instance_state.instance_type, "t2.micro");
        assert_eq!(ec2_instance_state.name, "name");
    }

    #[tokio::test]
    async fn test_ec2_instance_state_new_from_state() {
        // Arrange
        let mock_instance_role = InstanceRole::new("region".to_string()).await;

        let mock_instance_profile =
            InstanceProfile::new("region".to_string(), vec![mock_instance_role]).await;

        let instance_profile_state = InstanceProfileState::new(&mock_instance_profile);

        let ec2_instance_state = Ec2InstanceState {
            id: "id".to_string(),
            public_ip: "public_ip".to_string(),
            public_dns: "public_dns".to_string(),
            region: "region".to_string(),
            ami: "ami".to_string(),
            instance_type: "t2.micro".to_string(),
            name: "name".to_string(),
            vpc: VPCState {
                id: "id".to_string(),
                region: "region".to_string(),
                cidr_block: "test".to_string(),
                name: "name".to_string(),
                subnet: SubnetState {
                    id: "id".to_string(),
                    region: "region".to_string(),
                    cidr_block: "test".to_string(),
                    vpc_id: "vpc_id".to_string(),
                    name: "name".to_string(),
                },
            },
            instance_profile: Some(instance_profile_state),
        };

        // Act
        let ec2_instance = ec2_instance_state.new_from_state().await.unwrap();

        // Assert
        assert_eq!(ec2_instance.id, Some("id".to_string()));
        assert_eq!(ec2_instance.public_ip, Some("public_ip".to_string()));
        assert_eq!(ec2_instance.public_dns, Some("public_dns".to_string()));
        assert_eq!(ec2_instance.region, "region".to_string());
        assert_eq!(ec2_instance.ami, "ami".to_string());
        assert_eq!(ec2_instance.instance_type, InstanceType::T2_MICRO);
        assert_eq!(ec2_instance.name, "name".to_string());
        assert!(ec2_instance.instance_profile.is_some());
        assert_eq!(
            ec2_instance.instance_profile.unwrap().instance_roles.len(),
            1
        );
    }

    #[tokio::test]
    async fn test_ec2_instance_state_new_from_state_no_instance_profile() {
        // Arrange
        let ec2_instance_state = Ec2InstanceState {
            id: "id".to_string(),
            public_ip: "public_ip".to_string(),
            public_dns: "public_dns".to_string(),
            region: "region".to_string(),
            ami: "ami".to_string(),
            instance_type: "t2.micro".to_string(),
            name: "name".to_string(),
            vpc: VPCState {
                id: "id".to_string(),
                region: "region".to_string(),
                cidr_block: "test".to_string(),
                name: "name".to_string(),
                subnet: SubnetState {
                    id: "id".to_string(),
                    region: "region".to_string(),
                    cidr_block: "test".to_string(),
                    vpc_id: "vpc_id".to_string(),
                    name: "name".to_string(),
                },
            },
            instance_profile: None,
        };

        // Act
        let ec2_instance = ec2_instance_state.new_from_state().await;

        // Assert
        assert!(ec2_instance.is_err());
    }

    #[tokio::test]
    async fn test_instance_profile_state() {
        let instance_profile = InstanceProfile::new(
            "test_region".to_string(),
            vec![InstanceRole::new("test_region".to_string()).await],
        )
        .await;

        let instance_profile_state = InstanceProfileState::new(&instance_profile);

        assert_eq!(instance_profile_state.name, "test_name");
        assert_eq!(instance_profile_state.region, "test_region");
        assert_eq!(instance_profile_state.instance_roles.len(), 1);
    }

    #[tokio::test]
    async fn test_instance_profile_state_new_from_state() {
        // Arrange
        let instance_profile_state = InstanceProfileState {
            name: "test_name".to_string(),
            region: "test_region".to_string(),
            instance_roles: vec![InstanceRoleState {
                name: "test_name".to_string(),
                region: "test_region".to_string(),
                assume_role_policy: "test_assume_role_policy".to_string(),
                policy_arns: vec!["test_policy_arn".to_string()],
            }],
        };

        // Act
        let instance_profile = instance_profile_state.new_from_state().await;

        // Assert
        assert_eq!(instance_profile.name, "test_name".to_string());
        assert_eq!(instance_profile.region, "test_region".to_string());
        assert_eq!(instance_profile.instance_roles.len(), 1);
        assert_eq!(
            instance_profile.instance_roles[0].name,
            "test_name".to_string()
        );
        assert_eq!(
            instance_profile.instance_roles[0].assume_role_policy,
            "test_assume_role_policy".to_string()
        );
        assert_eq!(
            instance_profile.instance_roles[0].policy_arns,
            vec!["test_policy_arn".to_string()]
        );
    }

    #[tokio::test]
    async fn test_instance_role_state() {
        let instance_role = InstanceRole::new("test_region".to_string()).await;

        let instance_role_state = InstanceRoleState::new(&instance_role);

        assert_eq!(instance_role_state.name, "test_name");
        assert_eq!(instance_role_state.region, "test_region");
        assert_eq!(
            instance_role_state.assume_role_policy,
            "test_assume_role_policy"
        );
        assert_eq!(instance_role_state.policy_arns, vec!["test_policy_arn"]);
    }

    #[tokio::test]
    async fn test_instance_role_state_new_from_state() {
        // Arrange
        let instance_role_state = InstanceRoleState {
            name: "test_name".to_string(),
            region: "test_region".to_string(),
            assume_role_policy: "test_assume_role_policy".to_string(),
            policy_arns: vec!["test_policy_arn".to_string()],
        };

        // Act
        let instance_role = instance_role_state.new_from_state().await;

        // Assert
        assert_eq!(instance_role.name, "test_name".to_string());
        assert_eq!(instance_role.region, "test_region".to_string());
        assert_eq!(
            instance_role.assume_role_policy,
            "test_assume_role_policy".to_string()
        );
        assert_eq!(
            instance_role.policy_arns,
            vec!["test_policy_arn".to_string()]
        );
    }

    #[tokio::test]
    async fn test_vpc_state() {
        // Arrange
        let vpc = VPC::new(
            Some("id".to_string()),
            "region".to_string(),
            "name".to_string(),
            Subnet {
                id: Some("id".to_string()),
                region: "region".to_string(),
                cidr_block: "test_cidr_block".to_string(),
                vpc_id: Some("vpc_id".to_string()),
                name: "name".to_string(),
            },
        )
        .await;

        // Act
        let vpc_state = VPCState::new(&vpc);

        // Assert
        assert_eq!(vpc_state.id, "id".to_string());
        assert_eq!(vpc_state.region, "region".to_string());
        assert_eq!(vpc_state.cidr_block, "test_cidr_block".to_string());
        assert_eq!(vpc_state.name, "name".to_string());
    }

    #[tokio::test]
    async fn test_vpc_state_new_from_state() {
        // Arrange
        let vpc_state = VPCState {
            id: "id".to_string(),
            region: "region".to_string(),
            cidr_block: "test_cidr_block".to_string(),
            name: "name".to_string(),
            subnet: SubnetState {
                id: "id".to_string(),
                region: "region".to_string(),
                cidr_block: "test_cidr_block".to_string(),
                vpc_id: "vpc_id".to_string(),
                name: "name".to_string(),
            },
        };

        // Act
        let vpc = vpc_state.new_from_state().await;

        // Assert
        assert_eq!(vpc.id, Some("id".to_string()));
        assert_eq!(vpc.region, "region".to_string());
        assert_eq!(vpc.cidr_block, "test_cidr_block".to_string());
        assert_eq!(vpc.name, "name".to_string());
    }

    #[tokio::test]
    async fn test_subnet_state() {
        // Arrange
        let subnet = Subnet::new(
            Some("id".to_string()),
            "region".to_string(),
            "test_cidr_block".to_string(),
            Some("vpc_id".to_string()),
            "test_name".to_string(),
        )
        .await;

        // Act
        let subnet_state = SubnetState::new(&subnet);

        // Assert
        assert_eq!(subnet_state.id, "id".to_string());
        assert_eq!(subnet_state.region, "region".to_string());
        assert_eq!(subnet_state.cidr_block, "test_cidr_block".to_string());
        assert_eq!(subnet_state.vpc_id, "vpc_id".to_string());
        assert_eq!(subnet_state.name, "test_name".to_string());
    }

    #[tokio::test]
    async fn test_subnet_state_new_from_state() {
        // Arrange
        let subnet_state = SubnetState {
            id: "id".to_string(),
            region: "region".to_string(),
            cidr_block: "test_cidr_block".to_string(),
            vpc_id: "vpc_id".to_string(),
            name: "test_name".to_string(),
        };

        // Act
        let subnet = subnet_state.new_from_state().await;

        // Assert
        assert_eq!(subnet.id, Some("id".to_string()));
        assert_eq!(subnet.region, "region".to_string());
        assert_eq!(subnet.cidr_block, "test_cidr_block".to_string());
        assert_eq!(subnet.vpc_id, Some("vpc_id".to_string()));
        assert_eq!(subnet.name, "test_name".to_string());
    }
}
