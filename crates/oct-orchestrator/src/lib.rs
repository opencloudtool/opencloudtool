use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use oct_cloud::aws::resource::{
    DNSRecord, Ec2Instance, EcrRepository, HostedZone, InboundRule, InstanceProfile, InstanceRole,
    InternetGateway, RouteTable, SecurityGroup, Subnet, VPC,
};
use oct_cloud::aws::types::{InstanceType, RecordType};
use oct_cloud::infra;
use oct_cloud::resource::Resource;
use oct_cloud::state;

mod backend;
mod config;
mod oct_ctl_sdk;
mod scheduler;
mod user_state;

pub struct OrchestratorWithGraph;

impl OrchestratorWithGraph {
    const INSTANCE_TYPE: InstanceType = InstanceType::T3Medium;

    pub async fn deploy(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut config = config::Config::new(None)?;

        let infra_state_backend =
            backend::get_state_backend::<infra::state::State>(&config.project.state_backend);
        // let (mut infra_state, _loaded) = state_backend.load().await?;

        let user_state_backend =
            backend::get_state_backend::<user_state::UserState>(&config.project.user_state_backend);
        let (mut user_state, _loaded) = user_state_backend.load().await?;

        let (services_to_create, services_to_remove, services_to_update) =
            get_user_services_to_create_and_delete(&config, &user_state);

        log::info!("Services to create: {services_to_create:?}");
        log::info!("Services to remove: {services_to_remove:?}");
        log::info!("Services to update: {services_to_update:?}");

        let number_of_instances = get_number_of_needed_instances(&config, &Self::INSTANCE_TYPE);

        let spec_graph = infra::graph::GraphManager::get_spec_graph(
            number_of_instances,
            &Self::INSTANCE_TYPE,
            config.project.domain.clone(),
        );

        let graph_manager = infra::graph::GraphManager::new().await;
        let (resource_graph, vms, ecr) = graph_manager.deploy(&spec_graph).await;

        let state = infra::state::State::from_graph(&resource_graph);
        let () = infra_state_backend.save(&state).await?;

        for vm in &vms {
            let oct_ctl_client = oct_ctl_sdk::Client::new(vm.public_ip.clone());

            let host_health = check_host_health(&oct_ctl_client).await;
            if host_health.is_err() {
                return Err("Failed to check host health".into());
            }

            // Add missing instances to state
            // TODO: Handle removing instances
            if user_state.instances.contains_key(&vm.public_ip) {
                continue;
            }

            let instance_info = vm.instance_type.get_info();

            user_state.instances.insert(
                vm.public_ip.clone(),
                user_state::Instance {
                    cpus: instance_info.cpus,
                    memory: instance_info.memory,
                    services: HashMap::new(),
                },
            );
        }

        // All instances are healthy and ready to serve user services
        let mut scheduler = scheduler::Scheduler::new(&mut user_state, &*user_state_backend);

        if let Some(ecr) = ecr {
            let known_base_ecr_url = ecr.get_base_uri();

            container_manager_login(known_base_ecr_url)?;

            log::info!("Logged in to ECR {known_base_ecr_url}");

            for (service_name, service) in &mut config.project.services {
                let Some(dockerfile_path) = &service.dockerfile_path else {
                    log::debug!("Dockerfile path not specified for service '{service_name}'");

                    continue;
                };

                let ecr_url = ecr.uri.clone();
                let image_tag = format!("{ecr_url}:{service_name}-latest");

                build_image(dockerfile_path, &image_tag)?;
                push_image(&image_tag)?;

                service.image.clone_from(&image_tag);
            }
        }

        deploy_user_services(
            &config,
            &mut scheduler,
            &services_to_create,
            &services_to_remove,
            &services_to_update,
        )
        .await?;

        Ok(())
    }

    pub async fn destroy(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config = config::Config::new(None)?;

        let infra_state_backend =
            backend::get_state_backend::<infra::state::State>(&config.project.state_backend);
        let (infra_state, _loaded) = infra_state_backend.load().await?;

        let user_state_backend =
            backend::get_state_backend::<user_state::UserState>(&config.project.user_state_backend);
        let (_user_state, _loaded) = user_state_backend.load().await?;

        let resource_graph = infra_state.to_graph();

        let graph_manager = infra::graph::GraphManager::new().await;
        graph_manager.destroy(&resource_graph).await;

        infra_state_backend.remove().await?;
        user_state_backend.remove().await?;

        Ok(())
    }
}

/// Orchestrates the deployment and destruction of user services while managing the underlying
/// cloud infrastructure resources such as instances, networking, and container repositories
pub struct Orchestrator;

impl Orchestrator {
    const INSTANCE_TYPE: InstanceType = InstanceType::T3Medium;

    pub async fn deploy(&self) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: Put into Orchestrator struct field
        let mut config = config::Config::new(None)?;

        // Get user state file data
        let user_state_backend =
            backend::get_state_backend::<user_state::UserState>(&config.project.user_state_backend);
        let (mut user_state, _loaded) = user_state_backend.load().await?;

        let (services_to_create, services_to_remove, services_to_update) =
            get_user_services_to_create_and_delete(&config, &user_state);

        log::info!("Services to create: {services_to_create:?}");
        log::info!("Services to remove: {services_to_remove:?}");
        log::info!("Services to update: {services_to_update:?}");

        let number_of_instances = get_number_of_needed_instances(&config, &Self::INSTANCE_TYPE);

        log::info!("Number of instances required: {number_of_instances}");

        let base_ecr_url = self.prepare_ecr_repos(&mut config).await?;

        let state = self
            .prepare_infrastructure(&mut config, number_of_instances, base_ecr_url)
            .await?; // TODO(#189): pass info about required resources

        let mut instances = Vec::<Ec2Instance>::new();
        for instance in state.instances {
            instances.push(instance.new_from_state().await?);
        }

        for instance in &instances {
            let Some(public_ip) = instance.public_ip.clone() else {
                log::error!("Instance {:?} has no public IP", instance.id);

                return Err("Instance has no public IP".into());
            };

            let oct_ctl_client = oct_ctl_sdk::Client::new(public_ip.clone());

            let host_health = check_host_health(&oct_ctl_client).await;
            if host_health.is_err() {
                log::error!("Failed to check '{public_ip}' host health");

                return Err("Failed to check host health".into());
            }

            // Add missing instances to state
            // TODO: Handle removing instances
            if user_state.instances.contains_key(&public_ip) {
                continue;
            }

            let instance_info = instance.instance_type.get_info();

            user_state.instances.insert(
                public_ip.clone(),
                user_state::Instance {
                    cpus: instance_info.cpus,
                    memory: instance_info.memory,
                    services: HashMap::new(),
                },
            );
        }

        // All instances are healthy and ready to serve user services
        let mut scheduler = scheduler::Scheduler::new(&mut user_state, &*user_state_backend);

        deploy_user_services(
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
        while let Some(instance) = state.instances.pop() {
            let mut instance = instance.new_from_state().await?;

            instance.destroy().await?;
            state_backend.save(&state).await?;
        }

        log::info!("Instances destroyed");

        if let Some(vpc) = state.vpc {
            let mut vpc = vpc.new_from_state().await;

            vpc.destroy().await?;
            state.vpc = None;
            state_backend.save(&state).await?;

            log::info!("VPC destroyed");
        }

        if let Some(instance_profile) = state.instance_profile {
            let mut instance_profile = instance_profile.new_from_state().await;

            instance_profile.destroy().await?;
            state.instance_profile = None;
            state_backend.save(&state).await?;

            log::info!("Instance profile destroyed");
        }

        if let Some(hosted_zone) = state.hosted_zone {
            let mut hosted_zone = hosted_zone.new_from_state().await;

            hosted_zone.destroy().await?;
            state.hosted_zone = None;
            state_backend.save(&state).await?;

            log::info!("Hosted zone destroyed");
        }

        while let Some(ecr) = state.ecr_repos.pop() {
            let mut ecr = ecr.new_from_state().await;

            ecr.destroy().await?;
            state_backend.save(&state).await?;
        }

        state_backend.remove().await?;

        Ok(())
    }

    /// Prepares ECR repositories
    /// Clears existing ECR repositories in state
    /// Returns base ECR URL if successful
    async fn prepare_ecr_repos(
        &self,
        config: &mut config::Config,
    ) -> Result<Option<String>, Box<dyn std::error::Error>> {
        let state_backend =
            backend::get_state_backend::<state::State>(&config.project.state_backend);
        let (mut state, _loaded) = state_backend.load().await?;

        log::info!(
            "Clearing ECR repositories. It's not optimal and the repos can be re-used, \
            but it's ok for now. In the future, we should implement a more efficient \
            approach that reuses existing repositories."
        );

        while let Some(ecr) = state.ecr_repos.pop() {
            let mut ecr = ecr.new_from_state().await;

            ecr.destroy().await?;
            state_backend.save(&state).await?;
        }

        log::info!("ECR repositories cleared");

        log::info!("Starting container manager");

        let mut base_ecr_url = None; // TODO: Make prettier
        let mut ecr_repos_cache = HashMap::new();
        for (service_name, service) in &mut config.project.services {
            let Some(dockerfile_path) = &service.dockerfile_path else {
                log::debug!("Dockerfile path not specified");

                continue;
            };

            if ecr_repos_cache.contains_key(dockerfile_path) {
                service.image.clone_from(&ecr_repos_cache[dockerfile_path]);

                continue;
            }

            log::info!("Creating ECR repository for service '{service_name}'");

            // TODO: Change service_name to the name connected to Dockerfile
            let ecr_repo_name = format!("{}/{}", config.project.name, service_name);
            let mut ecr =
                EcrRepository::new(None, None, ecr_repo_name, "us-west-2".to_string()).await;

            ecr.create().await?;
            state.ecr_repos.push(state::ECRState::new(&ecr));
            state_backend.save(&state).await?;

            let known_base_ecr_url = format!(
                "{}.dkr.ecr.{}.amazonaws.com",
                ecr.id.clone().ok_or("No ECR id")?,
                ecr.region
            );

            container_manager_login(&known_base_ecr_url)?;

            log::info!("Logged in to ECR {known_base_ecr_url}");

            base_ecr_url = Some(known_base_ecr_url);

            let ecr_url = ecr.url.ok_or("No ECR url")?;
            let image_tag = format!("{ecr_url}:latest");

            build_image(dockerfile_path, &image_tag)?;
            push_image(&image_tag)?;

            service.image.clone_from(&image_tag);
            ecr_repos_cache.insert(dockerfile_path.clone(), image_tag.clone());
        }

        Ok(base_ecr_url)
    }

    /// Prepares L1 infrastructure (VM instances and base networking)
    async fn prepare_infrastructure(
        &self,
        config: &mut config::Config,
        number_of_instances: u32,
        base_ecr_url: Option<String>,
    ) -> Result<state::State, Box<dyn std::error::Error>> {
        let state_backend =
            backend::get_state_backend::<state::State>(&config.project.state_backend);
        let (mut state, _loaded) = state_backend.load().await?;

        if let Some(domain_name) = config.project.domain.clone() {
            let mut hosted_zone =
                HostedZone::new(None, vec![], domain_name, "us-west-2".to_string()).await;

            if state.hosted_zone.is_none() {
                hosted_zone.create().await?;
                state.hosted_zone = Some(state::HostedZoneState::new(&hosted_zone));
                state_backend.save(&state).await?;
            } else {
                log::info!("Hosted zone already exists");
            }
        }

        if state.vpc.is_none() {
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
            state.vpc = Some(state::VPCState::new(&vpc));
            state_backend.save(&state).await?;
        } else {
            log::info!("VPC already exists");
        }

        let mut instance_profile = InstanceProfile::new(
            "oct-instance-profile".to_string(),
            "us-west-2".to_string(),
            vec![InstanceRole::new("oct-instance-role".to_string(), "us-west-2".to_string()).await],
        )
        .await;

        if state.instance_profile.is_none() {
            instance_profile.create().await?;
            state.instance_profile = Some(state::InstanceProfileState::new(&instance_profile));
            state_backend.save(&state).await?;
        } else {
            log::info!("Instance profile already exists");
        }

        let ecr_login_string = match base_ecr_url {
            Some(base_ecr_url) => format!(
                r"aws ecr get-login-password --region us-west-2 | podman login --username AWS --password-stdin {base_ecr_url}"
            ),
            None => String::new(),
        };

        let user_data = format!(
            r#"#!/bin/bash
        set -e
        sudo apt update
        sudo apt -y install podman
        sudo systemctl start podman
        sudo snap install aws-cli --classic
        
        {ecr_login_string}

        curl \
            --output /home/ubuntu/oct-ctl \
            -L \
            https://github.com/opencloudtool/opencloudtool/releases/download/tip/oct-ctl \
            && sudo chmod +x /home/ubuntu/oct-ctl \
            && /home/ubuntu/oct-ctl &
        "#,
        );

        if state.instances.len() == number_of_instances as usize {
            log::info!("All instances are already created");

            return Ok(state);
        }

        log::info!("Removing existing instances");

        while let Some(instance) = state.instances.pop() {
            let mut instance = instance.new_from_state().await?;

            instance.destroy().await?;
            state_backend.save(&state).await?;
        }

        let subnet_id = state.vpc.as_ref().ok_or("No VPC")?.subnet.id.clone();
        let security_group_id = state
            .vpc
            .as_ref()
            .ok_or("No VPC")?
            .security_group
            .id
            .clone();

        for _ in 0..number_of_instances {
            let mut instance = Ec2Instance::new(
                None,
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
            let instance_public_ip = instance.public_ip.clone().ok_or("No instance public ip")?;

            log::info!("Instance created: {instance_id}");

            if let Some(hosted_zone) = state.hosted_zone.as_ref() {
                let hosted_zone_id = hosted_zone.id.clone();
                let hosted_zone_name = hosted_zone.name.clone();

                let subdomain = format!("{instance_id}.{hosted_zone_name}");

                let mut dns_record = DNSRecord::new(
                    hosted_zone_id,
                    subdomain.clone(),
                    RecordType::A,
                    instance_public_ip.clone(),
                    Some(3600),
                    instance.region.clone(),
                )
                .await;

                dns_record.create().await?;

                instance.dns_record = Some(dns_record);

                log::info!(
                    "Instance {instance_id} will be available at http://{subdomain}, once host is ready."
                );
            }

            state
                .instances
                .push(state::Ec2InstanceState::new(&instance));

            state_backend.save(&state).await?;
        }

        Ok(state)
    }
}

/// Calculates the number of instances needed to run the services
/// For now we expect that an individual service required resources will not exceed
/// a single EC2 instance capacity
fn get_number_of_needed_instances(config: &config::Config, instance_type: &InstanceType) -> u32 {
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

    let instance_info = instance_type.get_info();

    let needed_instances_count_by_cpus = total_services_cpus.div_ceil(instance_info.cpus);
    let needed_instances_count_by_memory = total_services_memory.div_ceil(instance_info.memory);

    std::cmp::max(
        needed_instances_count_by_cpus,
        u32::try_from(needed_instances_count_by_memory).unwrap_or_default(),
    )
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

/// Waits for a host to be healthy
async fn check_host_health(
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

/// Deploys and destroys user services
/// TODO: Use it in `destroy`. Needs some modifications to correctly handle state file removal
async fn deploy_user_services(
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

fn build_image(dockerfile_path: &str, tag: &str) -> Result<(), Box<dyn std::error::Error>> {
    if !Path::new(dockerfile_path).exists() {
        return Err("Dockerfile not found".into());
    }

    // TODO move to ContainerManager struct like in oct_ctl/src/main.rs
    let container_manager = get_container_manager()?;

    log::info!("Building image using '{container_manager}'");

    let run_container_args = Command::new(&container_manager)
        .args([
            "build",
            "-t",
            tag,
            "--platform",
            "linux/amd64",
            "-f",
            dockerfile_path,
            ".",
        ])
        .output()?;

    if !run_container_args.status.success() {
        return Err("Failed to build an image".into());
    }

    log::info!("Successfully built an image using '{container_manager}'");

    Ok(())
}

fn container_manager_login(ecr_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let container_manager = get_container_manager()?;

    log::info!("Logging in to ECR repository using '{container_manager}'");

    // Get the AWS ECR password
    let aws_output = Command::new("aws")
        .args(["ecr", "get-login-password", "--region", "us-west-2"])
        .output()?;

    if !aws_output.status.success() {
        return Err("Failed to get ECR password".into());
    }

    // Use the password as input for the container manager login command
    let login_process = Command::new(&container_manager)
        .args([
            "login",
            "--username",
            "AWS",
            "--password",
            String::from_utf8_lossy(&aws_output.stdout).as_ref(),
            ecr_url,
        ])
        .output()?;

    if !login_process.status.success() {
        return Err("Failed to login to ECR repository".into());
    }

    log::info!("Logged in to ECR repository using '{container_manager}'");

    Ok(())
}

fn push_image(image_tag: &str) -> Result<(), Box<dyn std::error::Error>> {
    let push_args = vec!["push", image_tag];

    let container_manager = get_container_manager()?;

    log::info!("Pushing image to ECR repository using '{container_manager}'");

    let output = Command::new(&container_manager).args(push_args).output()?;

    if !output.status.success() {
        return Err(format!(
            "Failed to push image to ECR repository. Error: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    log::info!("Pushed image to ECR repository using '{container_manager}'");

    Ok(())
}

/// Return podman or docker string depends on what is installed
fn get_container_manager() -> Result<String, Box<dyn std::error::Error>> {
    // TODO: Fix OS "Not found" error when `podman` is not installed
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

#[cfg(test)]
mod tests {}
