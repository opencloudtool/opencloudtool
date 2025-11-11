use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use petgraph::Graph;
use petgraph::dot::Dot;

use oct_cloud::aws::types::InstanceType;
use oct_cloud::infra;

mod backend;
mod config;
mod scheduler;
mod user_state;

pub struct OrchestratorWithGraph;

impl OrchestratorWithGraph {
    const INSTANCE_TYPE: InstanceType = InstanceType::T2Micro;

    pub async fn deploy(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut config = config::Config::new(None)?;

        let services_graph = config.to_graph()?;

        log::info!("User services graph: {}", Dot::new(&services_graph));

        let infra_state_backend =
            backend::get_state_backend::<infra::state::State>(&config.project.state_backend);
        // let (mut infra_state, _loaded) = state_backend.load().await?;

        let user_state_backend =
            backend::get_state_backend::<user_state::UserState>(&config.project.user_state_backend);
        let (mut user_state, _loaded) = user_state_backend.load().await?;

        let number_of_instances =
            get_number_of_needed_instances(&services_graph, &Self::INSTANCE_TYPE)?;

        log::info!("Instances to be created: {number_of_instances}");

        let spec_graph = infra::graph::GraphManager::get_spec_graph(
            number_of_instances,
            &Self::INSTANCE_TYPE,
            config.project.domain.clone(),
        );

        let infra_graph_manager = infra::graph::GraphManager::new().await;
        let (resource_graph, vms, ecr) = infra_graph_manager.deploy(&spec_graph).await?;

        let state = infra::state::State::from_graph(&resource_graph);
        let () = infra_state_backend.save(&state).await?;

        for vm in &vms {
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

        let mut scheduler = scheduler::Scheduler::new(&mut user_state, &*user_state_backend);

        deploy_user_services(&services_graph, &mut scheduler).await?;

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

        let mut resource_graph = infra_state.to_graph();

        let graph_manager = infra::graph::GraphManager::new().await;
        let destroy_result = graph_manager.destroy(&mut resource_graph).await;

        match destroy_result {
            Ok(()) => {
                infra_state_backend.remove().await?;
                user_state_backend.remove().await?;

                Ok(())
            }
            Err(e) => {
                log::error!("Failed to destroy: {e}");

                let current_infra_state = infra::state::State::from_graph(&resource_graph);

                if let Err(save_err) = infra_state_backend.save(&current_infra_state).await {
                    return Err(format!(
                        "Destruction failed: {e}. Additionally, failed to save state: {save_err}"
                    )
                    .into());
                }

                Err(format!("Partial destruction: {e}. Remaining resources saved to state.").into())
            }
        }
    }
}

/// Calculates the number of instances needed to run the services
/// For now we expect that an individual service required resources will not exceed
/// a single EC2 instance capacity
fn get_number_of_needed_instances(
    services_graph: &Graph<config::Node, String>,
    instance_type: &InstanceType,
) -> Result<u32, Box<dyn std::error::Error>> {
    let sorted_graph = infra::graph::kahn_traverse(services_graph)?;

    let total_services_cpus = sorted_graph
        .iter()
        .filter_map(|node_index| {
            if let config::Node::Resource(service) = &services_graph[*node_index] {
                return Some(service);
            }

            None
        })
        .map(|service| service.cpus)
        .sum::<u32>();

    let total_services_memory = sorted_graph
        .iter()
        .filter_map(|node_index| {
            if let config::Node::Resource(service) = &services_graph[*node_index] {
                return Some(service);
            }

            None
        })
        .map(|service| service.memory)
        .sum::<u64>();

    let instance_info = instance_type.get_info();

    let needed_instances_count_by_cpus = total_services_cpus.div_ceil(instance_info.cpus);
    let needed_instances_count_by_memory = total_services_memory.div_ceil(instance_info.memory);

    Ok(std::cmp::max(
        needed_instances_count_by_cpus,
        u32::try_from(needed_instances_count_by_memory).unwrap_or_default(),
    ))
}

/// Deploys user services
async fn deploy_user_services(
    services_graph: &Graph<config::Node, String>,
    scheduler: &mut scheduler::Scheduler<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    let sorted_graph = infra::graph::kahn_traverse(services_graph)?;

    for node_index in &sorted_graph {
        if let config::Node::Resource(service) = &services_graph[*node_index] {
            log::info!("Running service: {}", service.name);

            let _ = scheduler.run(&service.name, service).await;
        }
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
