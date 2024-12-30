use crate::aws::{self, InstanceProfile};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Ec2InstanceState {
    pub id: String,
    pub public_ip: String,
    pub public_dns: String,
    pub region: String,
    pub ami: String,
    pub instance_type: String,
    pub name: String,
    // TODO: Make instance_profile required
    pub instance_profile: Option<InstanceProfileState>,
}

impl Ec2InstanceState {
    pub async fn new(ec2_instance: &aws::Ec2Instance) -> Self {
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
            instance_type: ec2_instance.instance_type.clone().to_string(),
            name: ec2_instance.name.clone(),
            instance_profile: ec2_instance
                .instance_profile
                .as_ref()
                .map(|profile| InstanceProfileState::new(profile)),
        }
    }

    pub async fn new_from_state(&self) -> Result<aws::Ec2Instance, Box<dyn std::error::Error>> {
        let instance_profile = match &self.instance_profile {
            Some(profile) => profile.new_from_state().await,
            None => return Err("Instance profile is not set".into()),
        };

        Ok(aws::Ec2Instance::new(
            Some(self.id.clone()),
            Some(self.public_ip.clone()),
            Some(self.public_dns.clone()),
            self.region.clone(),
            self.ami.clone(),
            aws_sdk_ec2::types::InstanceType::from(self.instance_type.as_str()),
            self.name.clone(),
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
    pub fn new(instance_profile: &aws::InstanceProfile) -> Self {
        Self {
            name: instance_profile.name.clone(),
            region: instance_profile.region.clone(),
            instance_roles: instance_profile
                .instance_roles
                .iter()
                .map(|role| InstanceRoleState::new(role))
                .collect(),
        }
    }

    pub async fn new_from_state(&self) -> InstanceProfile {
        let mut roles = vec![];
        for role in &self.instance_roles {
            roles.push(role.new_from_state().await);
        }

        // TODO: Get all fields from the state
        aws::InstanceProfile::new(self.region.clone(), roles).await
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
    pub fn new(instance_role: &aws::InstanceRole) -> Self {
        Self {
            name: instance_role.name.clone(),
            region: instance_role.region.clone(),
            assume_role_policy: instance_role.assume_role_policy.clone(),
            policy_arns: instance_role.policy_arns.clone(),
        }
    }

    pub async fn new_from_state(&self) -> aws::InstanceRole {
        // TODO: Get all fields from the state
        aws::InstanceRole::new(self.region.clone()).await
    }
}
