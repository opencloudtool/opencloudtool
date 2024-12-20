use crate::aws;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Ec2InstanceState {
    pub id: String,
    pub arn: Option<String>,
    pub public_ip: String,
    pub public_dns: String,
    pub region: String,
    pub ami: String,
    pub instance_type: String,
    pub name: String,
    pub instance_profile: Option<InstanceProfileState>,
}

impl Ec2InstanceState {
    pub async fn new(ec2_instance: aws::Ec2Instance) -> Self {
        Self {
            id: ec2_instance.id.clone().expect("Instance id is not set"),
            arn: ec2_instance.arn.clone(),
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

    pub async fn build_from_state(&self) -> Result<aws::Ec2Instance, Box<dyn std::error::Error>> {
        // Load AWS configuration
        let region_provider = aws_sdk_ec2::config::Region::new(self.region.clone());
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(region_provider)
            .load()
            .await;

        let ec2_client = aws_sdk_ec2::Client::new(&config);
        // Initialize instance profile
        let instance_type: aws_sdk_ec2::types::InstanceType =
            aws_sdk_ec2::types::InstanceType::from(self.instance_type.as_str());

        let instance_profile = if let Some(profile_state) = &self.instance_profile {
            Some(InstanceProfileState::build_from_state(profile_state).await?)
        } else {
            None
        };

        Ok(aws::Ec2Instance {
            client: aws::Ec2::new(ec2_client),
            id: Some(self.id.clone()),
            arn: self.arn.clone(),
            public_ip: Some(self.public_ip.clone()),
            public_dns: Some(self.public_dns.clone()),
            region: self.region.clone(),
            ami: self.ami.clone(),
            instance_type,
            name: self.name.clone(),
            user_data: "".to_string(),
            user_data_base64: "".to_string(),
            instance_profile,
        })
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

    pub async fn build_from_state(
        &self,
    ) -> Result<aws::InstanceProfile, Box<dyn std::error::Error>> {
        let mut instance_roles = Vec::new();
        // Load AWS configuration
        let region_provider = aws_sdk_ec2::config::Region::new(self.region.clone());
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(region_provider)
            .load()
            .await;

        let iam_client = aws_sdk_iam::Client::new(&config);

        for role_state in &self.instance_roles {
            let role: aws::InstanceRole = role_state.build_from_state().await?;
            instance_roles.push(role);
        }
        Ok(aws::InstanceProfile {
            client: aws::IAM::new(iam_client),
            name: self.name.clone(),
            region: self.region.clone(),
            instance_roles,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
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

    pub async fn build_from_state(&self) -> Result<aws::InstanceRole, Box<dyn std::error::Error>> {
        // Load AWS configuration
        let region_provider = aws_sdk_ec2::config::Region::new(self.region.clone());
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(region_provider)
            .load()
            .await;

        let iam_client = aws_sdk_iam::Client::new(&config);

        Ok(aws::InstanceRole {
            client: aws::IAM::new(iam_client),
            name: self.name.clone(),
            region: self.region.clone(),
            assume_role_policy: self.assume_role_policy.clone(),
            policy_arns: self.policy_arns.clone(),
        })
    }
}
