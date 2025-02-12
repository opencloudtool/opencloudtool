use std::collections::HashMap;
use std::fs;
use std::path::Path;

use oct_cloud::aws::resource::{
    Ec2Instance, InternetGateway, RouteTable, SecurityGroup, Subnet, VPC,
};
use oct_cloud::aws::types::InstanceType;
use oct_cloud::resource::Resource;
use oct_cloud::state;

mod config;
mod oct_ctl_sdk;
mod scheduler;
mod user_state;

/// Deploys and destroys user services and manages underlying cloud resources
pub struct Orchestrator {
    state_file_path: String,
    user_state_file_path: String,
}

impl Orchestrator {
    const INSTANCE_TYPE: InstanceType = InstanceType::T2_MICRO;

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
        let mut user_state = user_state::UserState::new(&self.user_state_file_path)?;

        let (services_to_create, services_to_remove) =
            Self::get_user_services_to_create_and_delete(&config, &user_state);

        log::info!("Services to create: {services_to_create:?}");
        log::info!("Services to remove: {services_to_remove:?}");

        let number_of_instances =
            Self::get_number_of_needed_instances(&config, &Self::INSTANCE_TYPE);

        log::info!("Number of instances required: {number_of_instances}");

        let state = self.prepare_infrastructure(number_of_instances).await?; // TODO(#189): pass info about required resources

        let mut instances = Vec::<Ec2Instance>::new();

        for instance in state.instances {
            instances.push(instance.new_from_state().await?);
        }

        for instance in &instances {
            let Some(public_ip) = instance.public_ip.clone() else {
                log::error!("Instance {:?} has no public IP", instance.id);

                continue;
            };

            let oct_ctl_client = oct_ctl_sdk::Client::new(public_ip.clone());

            let host_health = self.check_host_health(&oct_ctl_client).await;
            if host_health.is_err() {
                log::error!("Failed to check '{}' host health", public_ip);

                continue;
            }

            // Add missing instances to state
            // TODO: Handle removing instances
            if !user_state.instances.contains_key(&public_ip) {
                user_state.instances.insert(
                    public_ip.clone(),
                    user_state::Instance {
                        cpus: instance.instance_type.cpus,
                        memory: instance.instance_type.memory,

                        services: HashMap::new(),
                    },
                );
            }
        }

        // All instances are healthy and ready to serve user services
        self.deploy_user_services(
            &config,
            &mut user_state,
            &services_to_create,
            &services_to_remove,
        )
        .await?;

        Ok(())
    }

    pub async fn destroy(&self) -> Result<(), Box<dyn std::error::Error>> {
        if !Path::new(&self.state_file_path).exists() {
            log::info!("Nothing to destroy");

            return Ok(());
        }

        let state = state::State::new(&self.state_file_path)?;

        // Create EC2 instances from state
        let mut instances = Vec::<Ec2Instance>::new();

        for instance in state.instances {
            instances.push(instance.new_from_state().await?);
        }

        // Create VPC from state
        let mut vpc = state.vpc.new_from_state().await;

        let user_state = user_state::UserState::new(&self.user_state_file_path)?;

        for (instance_ip, instance) in user_state.instances {
            for (service_name, _service) in instance.services {
                // Remove container from instance
                log::info!("Removing container for service: {}", service_name);

                let oct_ctl_client = oct_ctl_sdk::Client::new(instance_ip.clone());

                let response = oct_ctl_client.remove_container(service_name.clone()).await;

                match response {
                    Ok(()) => {
                        log::info!("Container removed for service: {}", service_name);
                    }
                    Err(err) => {
                        log::error!(
                            "Failed to remove container for service: {}. Error: {}",
                            service_name,
                            err
                        );
                    }
                }
            }
        }

        let _ = fs::remove_file(&self.user_state_file_path);

        // Destroy EC2 instance
        for mut instance in instances {
            instance.destroy().await?;
        }

        // Destroy VPC
        vpc.destroy().await?;

        log::info!("Instance destroyed");

        // Remove instance from state file
        fs::remove_file(&self.state_file_path).expect("Unable to remove file");

        log::info!("Instance removed from state file");

        Ok(())
    }

    /// Prepares L1 infrastructure (VM instances and base networking)
    async fn prepare_infrastructure(
        &self,
        number_of_instances: u32,
    ) -> Result<state::State, Box<dyn std::error::Error>> {
        // Get state file
        let mut state = state::State::new(&self.state_file_path)?;

        let security_group = SecurityGroup::new(
            None,
            "ct-app-security-group".to_string(),
            None,
            "ct-app-security-group".to_string(),
            // TODO: Add support of multiple ports
            // TODO: Expose only the ports needed by the application
            22,
            "tcp".to_string(),
            "us-west-2".to_string(),
        )
        .await;

        let route_table = RouteTable::new(None, None, None, "us-west-2".to_string()).await;

        let internet_gateway =
            InternetGateway::new(None, None, None, None, "us-west-2".to_string()).await;

        let subnet = Subnet::new(
            None,
            "us-west-2".to_string(),
            "10.0.0.0/24".to_string(),
            None,
            "ct-app-subnet".to_string(),
        )
        .await;

        let mut vpc = VPC::new(
            None,
            "us-west-2".to_string(),
            "10.0.0.0/16".to_string(),
            "ct-app-vpc".to_string(),
            subnet,
            Some(internet_gateway),
            route_table,
            security_group,
        )
        .await;

        vpc.create().await?;

        let mut created_instances = Vec::<Ec2Instance>::new();
        for _ in 0..number_of_instances {
            let mut instance = Ec2Instance::new(
                None,
                None,
                None,
                "us-west-2".to_string(),
                "ami-04dd23e62ed049936".to_string(),
                Self::INSTANCE_TYPE,
                "oct-cli".to_string(),
                None,
            )
            .await;

            instance.create().await?;

            let instance_id = instance.id.clone().ok_or("No instance id")?;

            log::info!("Instance created: {instance_id}");

            created_instances.push(instance);
        }

        // Add instance to state
        state.vpc = state::VPCState::new(&vpc);
        state.instances = created_instances
            .into_iter()
            .map(|instance| state::Ec2InstanceState::new(&instance))
            .collect();

        state.save(&self.state_file_path)?;

        Ok(state)
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

    /// Gets list of services to remove and to create
    /// Preserves order of services from config
    fn get_user_services_to_create_and_delete(
        config: &config::Config,
        user_state: &user_state::UserState,
    ) -> (Vec<String>, Vec<String>) {
        let expected_services: Vec<String> = config.project.services.keys().cloned().collect();
        let user_state_services: Vec<String> = user_state
            .instances
            .values()
            .flat_map(|instance| instance.services.keys())
            .cloned()
            .collect();

        let services_to_create: Vec<String> = expected_services
            .iter()
            .filter(|service| !user_state_services.contains(*service))
            .cloned()
            .collect();

        let services_to_remove: Vec<String> = user_state_services
            .iter()
            .filter(|service| !expected_services.contains(service))
            .cloned()
            .collect();

        (services_to_create, services_to_remove)
    }

    /// Calculates the number of instances needed to run the services
    /// For now we expect that an individual service required resources will not exceed
    /// a single EC2 instance capacity
    fn get_number_of_needed_instances(
        config: &config::Config,
        instance_type: &InstanceType,
    ) -> u32 {
        let total_services_cpus = config
            .project
            .services
            .values()
            .map(|service| service.cpus)
            .sum::<u32>();

        let total_services_memory = config
            .project
            .services
            .values()
            .map(|service| service.memory)
            .sum::<u64>();

        let needed_instances_count_by_cpus = total_services_cpus.div_ceil(instance_type.cpus);
        let needed_instances_count_by_memory = total_services_memory.div_ceil(instance_type.memory);

        std::cmp::max(
            needed_instances_count_by_cpus,
            u32::try_from(needed_instances_count_by_memory).unwrap_or_default(),
        )
    }

    /// Deploys and destroys user services
    /// TODO: Use it in `destroy`. Needs some modifications to correctly handle state file removal
    async fn deploy_user_services(
        &self,
        config: &config::Config,
        user_state: &mut user_state::UserState,
        services_to_create: &[String],
        services_to_remove: &[String],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut scheduler = scheduler::Scheduler::new(user_state);

        for service_name in services_to_remove {
            log::info!("Stopping container for service: {service_name}");

            let _ = scheduler.stop(service_name).await;
        }

        for service_name in services_to_create {
            let service = config.project.services.get(service_name);
            let Some(service) = service else {
                log::error!("Service '{service_name}' not found in config");

                continue;
            };

            log::info!("Running service: {service_name}");

            let _ = scheduler.run(service_name, service).await;
        }

        // Write updated user state to file
        user_state.save(&self.user_state_file_path)?;

        log::info!("Services saved to user state file");

        Ok(())
    }
}

#[cfg(test)]
mod tests {}
