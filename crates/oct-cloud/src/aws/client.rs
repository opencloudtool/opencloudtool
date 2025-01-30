/// AWS service clients implementation
use aws_sdk_ec2::operation::run_instances::RunInstancesOutput;

#[allow(unused_imports)]
use mockall::automock;

/// AWS EC2 client implementation
#[derive(Debug)]
pub(super) struct Ec2Impl {
    inner: aws_sdk_ec2::Client,
}

/// TODO: Add tests using static replay
#[cfg_attr(test, allow(dead_code))]
#[cfg_attr(test, automock)]
impl Ec2Impl {
    pub(super) fn new(inner: aws_sdk_ec2::Client) -> Self {
        Self { inner }
    }

    // Retrieve metadata about specific EC2 instance
    pub(super) async fn describe_instances(
        &self,
        instance_id: String,
    ) -> Result<aws_sdk_ec2::types::Instance, Box<dyn std::error::Error>> {
        let response = self
            .inner
            .describe_instances()
            .instance_ids(instance_id)
            .send()
            .await?;

        let instance = response
            .reservations()
            .first()
            .ok_or("No reservations")?
            .instances()
            .first()
            .ok_or("No instances")?;

        Ok(instance.clone())
    }

    // TODO: Return Instance instead of response
    pub(super) async fn run_instances(
        &self,
        instance_type: aws_sdk_ec2::types::InstanceType,
        ami: String,
        user_data_base64: String,
        instance_profile_name: Option<String>,
    ) -> Result<RunInstancesOutput, Box<dyn std::error::Error>> {
        log::info!("Starting EC2 instance");

        let mut request = self
            .inner
            .run_instances()
            .instance_type(instance_type.clone())
            .image_id(ami.clone())
            .user_data(user_data_base64.clone())
            .min_count(1)
            .max_count(1);

        if let Some(instance_profile_name) = instance_profile_name {
            request = request.iam_instance_profile(
                aws_sdk_ec2::types::IamInstanceProfileSpecification::builder()
                    .name(instance_profile_name)
                    .build(),
            );
        }

        let response = request.send().await?;

        log::info!("Created EC2 instance");

        Ok(response)
    }

    pub(super) async fn terminate_instance(
        &self,
        instance_id: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.inner
            .terminate_instances()
            .instance_ids(instance_id)
            .send()
            .await?;

        Ok(())
    }
}

/// AWS IAM client implementation
#[derive(Debug)]
pub(super) struct IAMImpl {
    inner: aws_sdk_iam::Client,
}

/// TODO: Add tests using static replay
#[cfg_attr(test, allow(dead_code))]
#[cfg_attr(test, automock)]
impl IAMImpl {
    pub(super) fn new(inner: aws_sdk_iam::Client) -> Self {
        Self { inner }
    }

    pub(super) async fn create_instance_iam_role(
        &self,
        name: String,
        assume_role_policy: String,
        policy_arns: Vec<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Create IAM role for EC2 instance
        log::info!("Creating IAM role for EC2 instance");

        self.inner
            .create_role()
            .role_name(name.clone())
            .assume_role_policy_document(assume_role_policy)
            .send()
            .await?;

        log::info!("Created IAM role for EC2 instance");

        for policy_arn in &policy_arns {
            log::info!("Attaching '{policy_arn}' policy to the role");

            self.inner
                .attach_role_policy()
                .role_name(name.clone())
                .policy_arn(policy_arn)
                .send()
                .await?;

            log::info!("Attached '{policy_arn}' policy to the role");
        }

        Ok(())
    }

    pub(super) async fn delete_instance_iam_role(
        &self,
        name: String,
        policy_arns: Vec<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for policy_arn in &policy_arns {
            log::info!("Detaching '{policy_arn}' IAM role from EC2 instance");

            self.inner
                .detach_role_policy()
                .role_name(name.clone())
                .policy_arn(policy_arn)
                .send()
                .await?;

            log::info!("Detached '{policy_arn}' IAM role from EC2 instance");
        }

        log::info!("Deleting IAM role for EC2 instance");

        self.inner
            .delete_role()
            .role_name(name.clone())
            .send()
            .await?;

        log::info!("Deleted IAM role for EC2 instance");

        Ok(())
    }

    pub(super) async fn create_instance_profile(
        &self,
        name: String,
        role_names: Vec<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Creating IAM instance profile for EC2 instance");

        self.inner
            .create_instance_profile()
            .instance_profile_name(name.clone())
            .send()
            .await?;

        log::info!("Created IAM instance profile for EC2 instance");

        for role_name in role_names {
            log::info!("Adding '{role_name}' IAM role to instance profile");

            self.inner
                .add_role_to_instance_profile()
                .instance_profile_name(name.clone())
                .role_name(role_name.clone())
                .send()
                .await?;

            log::info!("Added '{role_name}' IAM role to instance profile");
        }

        log::info!("Waiting for instance profile to be ready");
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;

        Ok(())
    }

    pub(super) async fn delete_instance_profile(
        &self,
        name: String,
        role_names: Vec<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for role_name in role_names {
            log::info!("Removing {role_name} IAM role from instance profile");

            self.inner
                .remove_role_from_instance_profile()
                .instance_profile_name(name.clone())
                .role_name(role_name.clone())
                .send()
                .await?;

            log::info!("Removed {role_name} IAM role from instance profile");
        }

        log::info!("Deleting IAM instance profile");

        self.inner
            .delete_instance_profile()
            .instance_profile_name(name.clone())
            .send()
            .await?;

        log::info!("Deleted IAM instance profile");

        Ok(())
    }
}

// TODO: Is there a better way to expose mocked structs?
#[cfg(not(test))]
pub(super) use Ec2Impl as Ec2;
#[cfg(test)]
pub(super) use MockEc2Impl as Ec2;

#[cfg(not(test))]
pub(super) use IAMImpl as IAM;
#[cfg(test)]
pub(super) use MockIAMImpl as IAM;
