use std::fs;

use oct_cloud::aws;
use oct_cloud::aws::Resource;
use oct_cloud::state;

mod config;
mod oct_ctl_sdk;
mod user_state;

/// Deploys and destroys user services and manages underlying cloud resources
pub struct Orchestrator {
    state_file_path: String,
    user_state_file_path: String,
}

impl Orchestrator {
    pub fn new(state_file_path: String, user_state_file_path: String) -> Self {
        Orchestrator {
            state_file_path,
            user_state_file_path,
        }
    }

    pub async fn deploy(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Get project config
        let config = config::Config::new(None)?;

        // Create EC2 instance
        let mut instance = aws::Ec2Instance::new(
            None,
            None,
            None,
            "us-west-2".to_string(),
            "ami-04dd23e62ed049936".to_string(),
            aws::aws_sdk_ec2::types::InstanceType::T2Micro,
            "oct-cli".to_string(),
            None,
        )
        .await;

        instance.create().await?;

        let instance_id = instance.id.clone().ok_or("No instance id")?;
        let public_ip = instance.public_ip.clone().ok_or("Public IP not found")?;

        log::info!("Instance created: {}", instance_id);

        // Save to state file
        let instance_state = state::Ec2InstanceState::new(&instance);
        let json_data = serde_json::to_string_pretty(&instance_state)?;
        fs::write(&self.state_file_path, json_data)?;

        let oct_ctl_client = oct_ctl_sdk::Client::new(public_ip.clone(), None);

        let _ = self.check_host_health(&oct_ctl_client).await?;

        for service in config.project.services {
            log::info!("Running container for service: {}", service.name);

            let response = oct_ctl_client
                .run_container(
                    service.name.to_string(),
                    service.image.to_string(),
                    service.external_port.to_string(),
                    service.internal_port.to_string(),
                )
                .await;

            if response.is_err() {
                log::error!("Failed to run '{}' service", service.name);
                continue;
            }

            log::info!(
                "Service is available at http://{}:{}",
                public_ip,
                service.external_port
            );

            // Save service to user state file
            let deployed_service =
                user_state::UserState::new(service.name.to_string(), public_ip.to_string());
            fs::write(
                &self.user_state_file_path,
                serde_json::to_string_pretty(&deployed_service)?,
            )?;
            log::info!(
                "Service: {} - saved to user state file",
                service.name.to_string()
            );
        }

        Ok(())
    }

    pub async fn destroy(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Load instance from state file
        let json_data =
            fs::read_to_string(&self.state_file_path).expect("Unable to read state file");
        let state: state::Ec2InstanceState = serde_json::from_str(&json_data)?;

        // Create EC2 instance from state
        let mut instance = state.new_from_state().await?;

        // Check if user state file exists
        if std::path::Path::new(&self.user_state_file_path).exists() {
            // Load service from user state file
            let service_json_data = fs::read_to_string(&self.user_state_file_path)
                .expect("Unable to read user state file");
            let user_state: user_state::UserState = serde_json::from_str(&service_json_data)?;

            // Remove container from instance
            log::info!(
                "Removing container for service: {}",
                user_state.service_name
            );

            let oct_ctl_client = oct_ctl_sdk::Client::new(user_state.public_ip, None);

            let response = oct_ctl_client
                .remove_container(user_state.service_name)
                .await;

            match response {
                Ok(()) => {
                    fs::remove_file(&self.user_state_file_path).expect("Unable to remove file");
                    log::info!("Service removed from user state file");
                }
                Err(err) => log::error!("Failed to remove service: {}", err),
            }
        } else {
            log::warn!("User state file not found or no containers are running");
        }

        // Destroy EC2 instance
        instance.destroy().await?;

        log::info!("Instance destroyed");

        // Remove instance from state file
        fs::remove_file(&self.state_file_path).expect("Unable to remove file");

        log::info!("Instance removed from state file");

        Ok(())
    }

    /// Waits for a host to be healthy
    async fn check_host_health(
        &self,
        oct_ctl_client: &oct_ctl_sdk::Client,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let public_ip = &oct_ctl_client.public_ip;

        let max_tries = 24;
        let sleep_duration_s = 5;

        log::info!("Waiting for host '{public_ip}' to be ready");

        for _ in 0..max_tries {
            match oct_ctl_client.health_check().await {
                Ok(_) => {
                    log::info!("Host '{public_ip}' is ready");

                    return Ok(());
                }
                Err(err) => {
                    log::info!(
                        "Host '{public_ip}' responded with error: {err}. \
                        Retrying in {sleep_duration_s} sec..."
                    );

                    tokio::time::sleep(std::time::Duration::from_secs(sleep_duration_s)).await;
                }
            }
        }

        Err(format!("Host '{public_ip}' failed to become ready after max retries").into())
    }
}

#[cfg(test)]
mod tests {}
