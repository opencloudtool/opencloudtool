use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Ec2InstanceState {
    pub id: String,
    pub arn: String,
    pub public_ip: String,
    pub public_dns: String,
    pub region: String,
    pub ami: String,
    pub instance_type: String,
    pub name: String,
    pub instance_profile: Option<InstanceProfileState>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InstanceProfileState {
    pub name: String,
    pub region: String,
    pub instance_roles: Vec<InstanceRoleState>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InstanceRoleState {
    pub name: String,
    pub region: String,
    pub assume_role_policy: String,
    pub policy_arns: Vec<String>,
}
