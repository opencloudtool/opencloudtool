use std::fs;

use oct_cloud::aws::resource::{Ec2Instance, InstanceType};
use oct_cloud::resource::Resource;
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

        // Get user state file data
        let mut user_state: user_state::UserState =
            if std::path::Path::new(&self.user_state_file_path).exists() {
                let existing_data = fs::read_to_string(&self.user_state_file_path)?;
                serde_json::from_str::<user_state::UserState>(&existing_data)?
            } else {
                user_state::UserState::default()
            };

        // Create EC2 instance
        let mut instance = Ec2Instance::new(
            None,
            None,
            None,
            "us-west-2".to_string(),
            "ami-04dd23e62ed049936".to_string(),
            InstanceType::T2Micro,
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

        self.check_host_health(&oct_ctl_client).await?;

        for service in config.project.services {
            log::info!("Running container for service: {}", service.name);

            let response = oct_ctl_client
                .run_container(
                    service.name.to_string(),
                    service.image.to_string(),
                    service.external_port,
                    service.internal_port,
                    service.cpus,
                    service.memory,
                    service.envs,
                )
                .await;

            match response {
                Ok(()) => match service.external_port {
                    Some(port) => {
                        log::info!(
                            "Service {} is available at http://{public_ip}:{port}",
                            service.name
                        );
                    }
                    None => {
                        log::info!("Service '{}' is running", service.name);
                    }
                },
                Err(err) => {
                    log::error!("Failed to run '{}' service. Error: {}", service.name, err);

                    continue;
                }
            }

            // Add service to deployed services Vec
            let deployed_service = user_state::Service {
                name: service.name.to_string(),
                public_ip: public_ip.clone(),
            };
            user_state.services.push(deployed_service);

            log::info!(
                "Service: {} - added to deployed services",
                service.name.to_string()
            );
        }

        // Save services to user state file
        fs::write(
            &self.user_state_file_path,
            serde_json::to_string_pretty(&user_state)?,
        )?;
        log::info!("Services saved to user state file");

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
            // Load services from user state file
            let user_state_json_data = fs::read_to_string(&self.user_state_file_path)?;
            let user_state = serde_json::from_str::<user_state::UserState>(&user_state_json_data)?;

            for service in user_state.services {
                // Remove container from instance
                log::info!("Removing container for service: {}", service.name);

                let oct_ctl_client = oct_ctl_sdk::Client::new(service.public_ip, None);

                let response = oct_ctl_client.remove_container(service.name.clone()).await;

                match response {
                    Ok(()) => {
                        log::info!("Container removed for service: {}", service.name);
                    }
                    Err(err) => {
                        log::error!(
                            "Failed to remove container for service: {}. Error: {}",
                            service.name,
                            err
                        );
                        continue;
                    }
                }
            }
            // Remove services from user state file
            let _ = fs::remove_file(&self.user_state_file_path);
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
                Ok(()) => {
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
