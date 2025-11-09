use crate::user_state;
use crate::{backend, config};

/// Schedules services on EC2 instances
/// TODO:
/// - Implement custom errors (Not enough capacity)
/// - Move state saving logic from Scheduler?
pub(crate) struct Scheduler<'a> {
    user_state: &'a mut user_state::UserState,
    state_backend: &'a dyn backend::StateBackend<user_state::UserState>,
}

impl<'a> Scheduler<'a> {
    pub(crate) fn new(
        user_state: &'a mut user_state::UserState,
        state_backend: &'a dyn backend::StateBackend<user_state::UserState>,
    ) -> Self {
        Self {
            user_state,
            state_backend,
        }
    }

    /// Runs a service on a first available instance and adds it to the state
    pub(crate) async fn run(
        &mut self,
        service_name: &str,
        service: &config::Service,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let services_context = self.user_state.get_services_context();

        for (public_ip, instance) in &mut self.user_state.instances {
            let (available_cpus, available_memory) = instance.get_available_resources();

            if available_cpus < service.cpus || available_memory < service.memory {
                log::info!(
                    "Not enough capacity to run '{service_name}' service on instance {public_ip}"
                );
                continue;
            }

            let oct_ctl_client = oct_ctl_sdk::Client::new(public_ip.clone());

            let response = oct_ctl_client
                .run_container(
                    service_name.to_string(),
                    service.image.clone(),
                    service.command.clone(),
                    service.external_port,
                    service.internal_port,
                    service.cpus,
                    service.memory,
                    service.render_envs(&services_context),
                )
                .await;

            match response {
                Ok(()) => {
                    match service.external_port {
                        Some(port) => {
                            log::info!(
                                "Service {} is available at http://{}:{port}",
                                service_name,
                                oct_ctl_client.public_ip()
                            );
                        }
                        None => {
                            log::info!("Service '{service_name}' is running");
                        }
                    }

                    instance
                        .services
                        .insert(service_name.to_string(), service.clone());

                    break;
                }
                Err(err) => {
                    log::error!("Failed to run '{service_name}' service. Error: {err}");
                }
            }
        }

        self.save_state().await;

        Ok(())
    }

    /// Stops a running container and removes it from the state
    #[allow(dead_code)]
    pub(crate) async fn stop(
        &mut self,
        service_name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for (public_ip, instance) in &mut self.user_state.instances {
            if !instance.services.contains_key(service_name) {
                continue;
            }

            let oct_ctl_client = oct_ctl_sdk::Client::new(public_ip.clone());

            let response = oct_ctl_client
                .remove_container(service_name.to_string())
                .await;

            match response {
                Ok(()) => {
                    instance.services.remove(service_name);

                    break;
                }
                Err(err) => {
                    log::error!("Failed to stop container for service '{service_name}': {err}");
                }
            }
        }

        self.save_state().await;

        Ok(())
    }

    async fn save_state(&self) {
        match self.state_backend.save(self.user_state).await {
            Ok(()) => {
                log::info!("User state saved using state backend");
            }
            Err(err) => {
                log::error!("Failed to save user state: {err}");
            }
        }
    }
}
