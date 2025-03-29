use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use oct_cloud::aws::resource::{
    Ec2Instance, EcrRepository, HostedZone, InboundRule, InstanceProfile, InstanceRole,
    InternetGateway, RouteTable, SecurityGroup, Subnet, VPC,
};
use oct_cloud::aws::types::InstanceType;
use oct_cloud::resource::Resource;
use oct_cloud::state;

mod backend;
mod config;
mod oct_ctl_sdk;
mod scheduler;
mod user_state;

/// Orchestrates the deployment and destruction of user services while managing the underlying
/// cloud infrastructure resources such as instances, networking, and container repositories
pub struct Orchestrator;

impl Orchestrator {
    const INSTANCE_TYPE: InstanceType = InstanceType::T2_MICRO;

    pub async fn deploy(&self) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: Put into Orchestrator struct field
        let mut config = config::Config::new(None)?;

        // Get user state file data
        let user_state_backend =
            backend::get_state_backend::<user_state::UserState>(&config.project.user_state_backend);
        let (mut user_state, _loaded) = user_state_backend.load().await?;

        let (services_to_create, services_to_remove, services_to_update) =
            Self::get_user_services_to_create_and_delete(&config, &user_state);

        log::info!("Services to create: {services_to_create:?}");
        log::info!("Services to remove: {services_to_remove:?}");
        log::info!("Services to update: {services_to_update:?}");

        let number_of_instances =
            Self::get_number_of_needed_instances(&config, &Self::INSTANCE_TYPE);

        log::info!("Number of instances required: {number_of_instances}");

        let state = self
            .prepare_infrastructure(&config, number_of_instances)
            .await?; // TODO(#189): pass info about required resources

        let ecr_repository = state.ecr.new_from_state().await;

        let Some(repository_url) = ecr_repository.url else {
            return Err("ECR repository url not found".into());
        };

        let repository_url = format!("{repository_url}:latest");

        config.project.services.iter_mut().for_each(|(_, service)| {
            match &service.dockerfile_path {
                Some(dockerfile_path) => match build_image(dockerfile_path, &repository_url) {
                    Ok(()) => {
                        let result = push_image(repository_url.clone());

                        match result {
                            Ok(()) => {
                                // Save image uri to state
                                service.image.clone_from(&repository_url);
                            }
                            Err(e) => log::error!("Failed to push image to ECR repository: {e}"),
                        }
                    }

                    Err(e) => log::error!("Failed to build an image: {e}"),
                },
                None => {
                    log::info!("Dockerfile path not specified");
                }
            }
        });

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
                log::error!("Failed to check '{public_ip}' host health");

                continue;
            }

            // Add missing instances to state
            // TODO: Handle removing instances
            if user_state.instances.contains_key(&public_ip) {
                continue;
            }

            user_state.instances.insert(
                public_ip.clone(),
                user_state::Instance {
                    cpus: instance.instance_type.cpus,
                    memory: instance.instance_type.memory,
                    services: HashMap::new(),
                },
            );
        }

        // All instances are healthy and ready to serve user services
        let mut scheduler = scheduler::Scheduler::new(&mut user_state, &*user_state_backend);

        self.deploy_user_services(
            &config,
            &mut scheduler,
            &services_to_create,
            &services_to_remove,
            &services_to_update,
        )
        .await?;

        // TODO: Map public IP to domain name in Route 53

        Ok(())
    }

    pub async fn destroy(&self) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: Put into Orchestrator struct field
        let config = config::Config::new(None)?;

        let state_backend =
            backend::get_state_backend::<state::State>(&config.project.state_backend);
        let (mut state, loaded) = state_backend.load().await?;

        if !loaded {
            log::info!("Nothing to destroy");

            return Ok(());
        }

        // Destroy user services
        let user_state_backend =
            backend::get_state_backend::<user_state::UserState>(&config.project.user_state_backend);
        let (mut user_state, user_state_loaded) = user_state_backend.load().await?;

        if user_state_loaded {
            // TODO: Simplify
            let all_service_names = user_state
                .instances
                .values()
                .map(|instance| instance.services.keys().cloned())
                .collect::<Vec<_>>()
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();

            let mut scheduler = scheduler::Scheduler::new(&mut user_state, &*user_state_backend);

            for service_name in all_service_names {
                let _ = scheduler.stop(&service_name).await;
            }

            user_state_backend.remove().await?;
        }

        // Destroy infrastructure
        for i in (0..state.instances.len()).rev() {
            let mut instance = state.instances[i].new_from_state().await?;

            instance.destroy().await?;
            state.instances.remove(i);
            state_backend.save(&state).await?;
        }

        log::info!("Instances destroyed");

        let mut vpc = state.vpc.new_from_state().await;

        vpc.destroy().await?;
        state.vpc = state::VPCState::default();
        state_backend.save(&state).await?;

        log::info!("VPC destroyed");

        let mut instance_profile = state.instance_profile.new_from_state().await;

        instance_profile.destroy().await?;
        state.instance_profile = state::InstanceProfileState::default();
        state_backend.save(&state).await?;

        log::info!("Instance profile destroyed");

        if let Some(hosted_zone) = state.hosted_zone {
            let mut hosted_zone = hosted_zone.new_from_state().await;

            hosted_zone.destroy().await?;
            state.hosted_zone = None;
            state_backend.save(&state).await?;

            log::info!("Hosted zone destroyed");
        }

        let mut ecr = state.ecr.new_from_state().await;

        ecr.destroy().await?;
        state.ecr = state::ECRState::default();
        state_backend.save(&state).await?;

        log::info!("ECR destroyed");

        state_backend.remove().await?;

        Ok(())
    }

    /// Prepares L1 infrastructure (VM instances and base networking)
    async fn prepare_infrastructure(
        &self,
        config: &config::Config,
        number_of_instances: u32,
    ) -> Result<state::State, Box<dyn std::error::Error>> {
        let state_backend =
            backend::get_state_backend::<state::State>(&config.project.state_backend);
        let (mut state, loaded) = state_backend.load().await?;

        if loaded {
            log::info!("State file already exists");

            return Ok(state);
        }

        if let Some(domain_name) = config.project.domain.clone() {
            let mut hosted_zone =
                HostedZone::new(None, None, domain_name, "us-west-2".to_string()).await;

            hosted_zone.create().await?;
            state.hosted_zone = Some(state::HostedZoneState::new(&hosted_zone));
            state_backend.save(&state).await?;
        }

        let inbound_rules = vec![
            InboundRule {
                cidr_block: "0.0.0.0/0".to_string(),
                protocol: "tcp".to_string(),
                port: 80,
            },
            InboundRule {
                cidr_block: "0.0.0.0/0".to_string(),
                protocol: "tcp".to_string(),
                port: 31888,
            },
            InboundRule {
                cidr_block: "0.0.0.0/0".to_string(),
                protocol: "tcp".to_string(),
                port: 22,
            },
        ];

        let security_group = SecurityGroup::new(
            None,
            "ct-app-security-group".to_string(),
            None,
            "ct-app-security-group".to_string(),
            "us-west-2".to_string(),
            inbound_rules,
        )
        .await;

        let route_table = RouteTable::new(None, None, None, "us-west-2".to_string()).await;

        let internet_gateway =
            InternetGateway::new(None, None, None, None, "us-west-2".to_string()).await;

        let subnet = Subnet::new(
            None,
            "us-west-2".to_string(),
            "10.0.0.0/24".to_string(),
            "us-west-2a".to_string(),
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
        state.vpc = state::VPCState::new(&vpc);
        state_backend.save(&state).await?;

        let subnet_id = vpc.subnet.id.clone().ok_or("No subnet id")?;
        let security_group_id = vpc
            .security_group
            .id
            .clone()
            .ok_or("No security group id")?;

        let mut instance_profile = InstanceProfile::new(
            "oct-instance-profile".to_string(),
            "us-west-2".to_string(),
            vec![InstanceRole::new("oct-instance-role".to_string(), "us-west-2".to_string()).await],
        )
        .await;

        instance_profile.create().await?;
        state.instance_profile = state::InstanceProfileState::new(&instance_profile);
        state_backend.save(&state).await?;

        let mut ecr =
            EcrRepository::new(None, None, "oct-ecr".to_string(), "us-west-2".to_string()).await;

        ecr.create().await?;
        state.ecr = state::ECRState::new(&ecr);
        state_backend.save(&state).await?;

        let user_data = format!(
            r#"#!/bin/bash
        set -e
        sudo apt update
        sudo apt -y install podman
        sudo systemctl start podman
        sudo snap install aws-cli --classic
        aws ecr get-login-password --region {ecr_region} | podman login --username AWS --password-stdin {ecr_url}
        curl \
            --output /home/ubuntu/oct-ctl \
            -L \
            https://github.com/opencloudtool/opencloudtool/releases/download/tip/oct-ctl \
            && sudo chmod +x /home/ubuntu/oct-ctl \
            && /home/ubuntu/oct-ctl &
        "#,
            ecr_region = ecr.region,
            ecr_url = ecr.url.clone().ok_or("No ecr url")?,
        );

        for _ in 0..number_of_instances {
            let mut instance = Ec2Instance::new(
                None,
                None,
                None,
                "us-west-2".to_string(),
                "ami-04dd23e62ed049936".to_string(),
                Self::INSTANCE_TYPE,
                "oct-cli".to_string(),
                instance_profile.name.clone(),
                subnet_id.clone(),
                security_group_id.clone(),
                user_data.clone(),
            )
            .await;

            instance.create().await?;

            let instance_id = instance.id.clone().ok_or("No instance id")?;

            log::info!("Instance created: {instance_id}");

            state
                .instances
                .push(state::Ec2InstanceState::new(&instance));

            state_backend.save(&state).await?;
        }

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

    /// Gets list of services to remove/create/update
    /// The order of created services depends on `depends_on` field in the config,
    /// dependencies are created first
    fn get_user_services_to_create_and_delete(
        config: &config::Config,
        user_state: &user_state::UserState,
    ) -> (Vec<String>, Vec<String>, Vec<String>) {
        let expected_services: Vec<String> = config.project.services.keys().cloned().collect();

        let user_state_services: Vec<String> = user_state
            .instances
            .values()
            .flat_map(|instance| instance.services.keys())
            .cloned()
            .collect();

        let expected_services_dependencies: Vec<String> = expected_services
            .iter()
            .filter_map(|service| config.project.services[service].depends_on.clone())
            .flatten()
            .filter(|service| !user_state_services.contains(service))
            .collect();

        let services_to_create: Vec<String> = expected_services
            .iter()
            .filter(|service| {
                !user_state_services.contains(service)
                    && !expected_services_dependencies.contains(service)
            })
            .cloned()
            .collect();

        let services_to_remove: Vec<String> = user_state_services
            .iter()
            .filter(|service| !expected_services.contains(service))
            .cloned()
            .collect();

        let services_to_update_dependencies: Vec<String> = expected_services
            .iter()
            .filter(|service| user_state_services.contains(service))
            .filter_map(|service| config.project.services[service].depends_on.clone())
            .flatten()
            .collect();

        let services_to_update: Vec<String> = expected_services
            .iter()
            .filter(|service| {
                user_state_services.contains(service)
                    && !services_to_update_dependencies.contains(service)
            })
            .cloned()
            .collect();

        (
            expected_services_dependencies
                .iter()
                .chain(services_to_create.iter())
                .cloned()
                .collect(),
            services_to_remove,
            services_to_update_dependencies
                .iter()
                .chain(services_to_update.iter())
                .cloned()
                .collect(),
        )
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
        scheduler: &mut scheduler::Scheduler<'_>, // TODO: Figure out why lifetime is needed
        services_to_create: &[String],
        services_to_remove: &[String],
        services_to_update: &[String],
    ) -> Result<(), Box<dyn std::error::Error>> {
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

        for service_name in services_to_update {
            log::info!("Updating service: {service_name}");

            let service = config.project.services.get(service_name);
            let Some(service) = service else {
                log::error!("Service '{service_name}' not found in config");

                continue;
            };

            log::info!("Recreating container for service: {service_name}");

            let _ = scheduler.stop(service_name).await;
            let _ = scheduler.run(service_name, service).await;
        }

        Ok(())
    }
}

fn build_image(dockerfile_path: &String, tag: &str) -> Result<(), Box<dyn std::error::Error>> {
    if !Path::new(&dockerfile_path).exists() {
        return Err("Dockerfile not found".into());
    }

    // TODO move to ContainerManager struct like in oct_ctl/src/main.rs
    let container_manager = get_container_manager()?;

    log::info!("Container manager: {container_manager}");

    let run_container_args = Command::new(container_manager)
        .args([
            "build",
            "-t",
            tag,
            "--platform",
            "linux/amd64",
            "-f",
            dockerfile_path.as_str(),
            ".",
        ])
        .output()?;

    log::info!("Build command output: {run_container_args:?}");

    if !run_container_args.status.success() {
        return Err("Failed to build an image".into());
    }

    log::info!("Successfully built an image");

    Ok(())
}

/// Return podman or docker string depends on what is installed
fn get_container_manager() -> Result<String, Box<dyn std::error::Error>> {
    let podman_exists = Command::new("podman")
        .args(["--version"])
        .output()?
        .status
        .success();

    if podman_exists {
        return Ok("podman".to_string());
    }

    let docker_exists = Command::new("docker")
        .args(["--version"])
        .output()?
        .status
        .success();

    if docker_exists {
        return Ok("docker".to_string());
    }

    Err("Docker and Podman not installed".into())
}

fn push_image(repository_url: String) -> Result<(), Box<dyn std::error::Error>> {
    log::info!("Pushing image to ECR repository");

    let push_args = vec!["push".to_string(), repository_url];

    let output = Command::new("podman").args(push_args).output()?;

    if !output.status.success() {
        return Err(format!("Failed to push image to ECR repository. {output:?}").into());
    }

    log::info!("Pushed image to ECR repository");

    Ok(())
}

#[cfg(test)]
mod tests {}
