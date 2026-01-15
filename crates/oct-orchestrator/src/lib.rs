use petgraph::Graph;

use oct_cloud::aws::types::InstanceType;
use oct_cloud::infra;

pub mod backend;
pub mod user_state;

pub struct OrchestratorWithGraph;

impl OrchestratorWithGraph {
    /// Initial step of the `oct`-managed system deployment
    pub async fn genesis(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config = oct_config::Config::new(None)?;

        let infra_state_backend =
            backend::get_state_backend::<infra::state::State>(&config.project.state_backend);

        // In the current version there is only one Leader node which serves
        // all user services, so it's okay to get instance type from the user services graph
        let user_services_graph = config.to_graph()?;
        let instance_type = get_instance_type(&user_services_graph)?;

        let genesis_spec_graph = infra::graph::GraphManager::get_genesis_graph(instance_type);

        let infra_graph_manager = infra::graph::GraphManager::new().await;
        let (resource_graph, _vm) = infra_graph_manager
            .deploy_genesis_graph(&genesis_spec_graph)
            .await?;

        let state = infra::state::State::from_graph(&resource_graph);
        let () = infra_state_backend.save(&state).await?;

        Ok(())
    }

    pub async fn apply(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config = oct_config::Config::new(None)?;

        let infra_state_backend =
            backend::get_state_backend::<infra::state::State>(&config.project.state_backend);
        let (infra_state, _loaded) = infra_state_backend.load().await?;

        let vms = infra_state.get_vms();
        let leader_vm = vms.first().ok_or("No leader available")?;

        let oct_ctl_client = oct_ctl_sdk::Client::new(leader_vm.public_ip.clone());
        let () = oct_ctl_client.apply(config).await?;

        Ok(())
    }

    pub async fn destroy(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config = oct_config::Config::new(None)?;

        let infra_state_backend =
            backend::get_state_backend::<infra::state::State>(&config.project.state_backend);
        let (infra_state, _loaded) = infra_state_backend.load().await?;

        let vms = infra_state.get_vms();
        let leader_vm = vms.first().ok_or("No leader available")?;

        let oct_ctl_client = oct_ctl_sdk::Client::new(leader_vm.public_ip.clone());
        let () = oct_ctl_client.destroy().await?;

        let mut resource_graph = infra_state.to_graph();

        let graph_manager = infra::graph::GraphManager::new().await;
        let destroy_result = graph_manager.destroy(&mut resource_graph).await;

        match destroy_result {
            Ok(()) => {
                infra_state_backend.remove().await?;

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

/// Tries to find an instance type which can fit all user-requested services
fn get_instance_type(
    services_graph: &Graph<oct_config::Node, String>,
) -> Result<InstanceType, Box<dyn std::error::Error>> {
    let sorted_graph = infra::graph::kahn_traverse(services_graph)?;

    let total_services_cpus = sorted_graph
        .iter()
        .filter_map(|node_index| {
            if let oct_config::Node::Resource(service) = &services_graph[*node_index] {
                return Some(service);
            }

            None
        })
        .map(|service| service.cpus)
        .sum::<u32>();

    let total_services_memory = sorted_graph
        .iter()
        .filter_map(|node_index| {
            if let oct_config::Node::Resource(service) = &services_graph[*node_index] {
                return Some(service);
            }

            None
        })
        .map(|service| service.memory)
        .sum::<u64>();

    let instance_type = InstanceType::from_resources(total_services_cpus, total_services_memory);

    match instance_type {
        Some(instance_type) => Ok(instance_type),
        None => Err("Failed to get instance type to fit all services".into()),
    }
}

#[cfg(test)]
mod tests {}
