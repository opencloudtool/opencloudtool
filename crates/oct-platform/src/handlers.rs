use std::collections::HashMap;
use std::convert::Infallible;
use std::fmt::Write;
use std::sync::Arc;

use askama::Template;
use axum::extract::{Json, Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, Sse};
use axum::response::{Html, IntoResponse};
use futures::stream::Stream;
use serde::Deserialize;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

use oct_cloud::infra::state::State as InfraState;
use oct_config::{Project, Service};

use crate::config_manager::{ConfigManager, ProjectSummary};
use crate::orchestrator::Orchestrator;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
pub struct AppState {
    pub orchestrator: Arc<dyn Orchestrator>,
    pub config_manager: Arc<dyn ConfigManager>,
    pub log_sender: tokio::sync::broadcast::Sender<String>,
}

// --- Templates ---

#[derive(Template)]
#[template(path = "pages/projects.html")]
struct ProjectsTemplate {
    projects: Vec<ProjectSummary>,
    version: &'static str,
}

#[derive(Template)]
#[template(path = "pages/index.html")]
struct IndexTemplate<'a> {
    project: &'a Project,
    raw_config: String,
    version: &'static str,
}

#[derive(Template)]
#[template(path = "pages/edit.html")]
struct EditTemplate<'a> {
    project: &'a Project,
    version: &'static str,
}

#[derive(Template)]
#[template(path = "pages/edit.html", block = "content")]
struct EditContentTemplate<'a> {
    project: &'a Project,
}

#[derive(Template)]
#[template(path = "pages/state.html")]
struct StateTemplate {
    project_name: String,
    state_json: String,
    mermaid_graph: String,
    version: &'static str,
}

#[derive(Template)]
#[template(path = "pages/index.html", block = "content")]
struct IndexContentTemplate<'a> {
    project: &'a Project,
    raw_config: String,
}

// --- Handlers ---

pub async fn root_redirect() -> impl IntoResponse {
    axum::response::Redirect::to("/projects")
}

pub async fn list_projects(State(state): State<AppState>) -> impl IntoResponse {
    let projects = state.config_manager.list_projects();
    let template = ProjectsTemplate {
        projects,
        version: VERSION,
    };
    render_template(template)
}

#[derive(Deserialize)]
pub struct CreateProjectForm {
    pub name: String,
}

pub async fn create_project_action(
    State(state): State<AppState>,
    axum::Form(form): axum::Form<CreateProjectForm>,
) -> impl IntoResponse {
    if let Err(e) = state.config_manager.create_project(&form.name) {
        return (StatusCode::BAD_REQUEST, Html(format!("Error: {e}"))).into_response();
    }
    let url = format!("/projects/{}", form.name);
    ([("HX-Redirect", url)], "Redirecting...").into_response()
}

pub async fn project_dashboard(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.config_manager.load_project(&name) {
        Ok(config) => {
            let raw_config = match state.config_manager.load_project_raw(&name) {
                Ok(c) => c,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Html(format!("Failed to load raw config: {e}")),
                    );
                }
            };
            let template = IndexTemplate {
                project: &config.project,
                raw_config,
                version: VERSION,
            };
            render_template(template)
        }
        Err(e) => (
            StatusCode::NOT_FOUND,
            Html(format!("Project not found: {e}")),
        ),
    }
}

pub async fn view_state(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.config_manager.load_project(&name) {
        Ok(config) => {
            let infra_state_backend = oct_orchestrator::backend::get_state_backend::<InfraState>(
                &config.project.state_backend,
            );

            let (infra_state, _) = match infra_state_backend.load().await {
                Ok(s) => s,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Html(format!("Failed to load state: {e}")),
                    );
                }
            };
            let state_json = match serde_json::to_string_pretty(&infra_state) {
                Ok(s) => s,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Html(format!("Failed to serialize state: {e}")),
                    );
                }
            };

            let mut mermaid_graph = String::from("graph TD;\n");
            let graph = infra_state.to_graph();
            let raw_nodes = graph.raw_nodes();
            let raw_edges = graph.raw_edges();

            for (i, node) in raw_nodes.iter().enumerate() {
                let label = match &node.weight {
                    oct_cloud::infra::resource::Node::Root => "Root".to_string(),
                    oct_cloud::infra::resource::Node::Resource(r) => r.name(),
                };
                let id = format!("node_{i}");
                let _ = writeln!(mermaid_graph, "    {id}[\"{label}\"];");
            }

            for edge in raw_edges {
                let source_id = format!("node_{}", edge.source().index());
                let target_id = format!("node_{}", edge.target().index());
                let _ = writeln!(mermaid_graph, "    {source_id} --> {target_id};");
            }

            let template = StateTemplate {
                project_name: config.project.name,
                state_json,
                mermaid_graph,
                version: VERSION,
            };
            render_template(template)
        }
        Err(e) => (
            StatusCode::NOT_FOUND,
            Html(format!("Project not found: {e}")),
        ),
    }
}

fn form_to_services(
    form_services: Vec<ServiceUpdate>,
    existing_services: &[Service],
) -> Vec<Service> {
    form_services
        .into_iter()
        .map(|s| {
            let existing = existing_services.iter().find(|es| es.name == s.name);
            let envs: HashMap<String, String> =
                s.envs.into_iter().map(|e| (e.key, e.value)).collect();

            Service {
                name: s.name,
                image: s.image,
                cpus: s.cpus.parse().unwrap_or(250),
                memory: s.memory.parse().unwrap_or(64),
                dockerfile_path: existing.and_then(|e| e.dockerfile_path.clone()),
                command: existing.and_then(|e| e.command.clone()),
                internal_port: existing.and_then(|e| e.internal_port),
                external_port: existing.and_then(|e| e.external_port),
                depends_on: existing.map(|e| e.depends_on.clone()).unwrap_or_default(),
                envs,
            }
        })
        .collect()
}

#[derive(Debug, Deserialize)]
pub struct UpdateProjectForm {
    pub name: String,
    pub domain: Option<String>,
    #[serde(default)]
    pub services: Vec<ServiceUpdate>,
}

#[derive(Debug, Deserialize)]
pub struct ServiceUpdate {
    pub name: String,
    pub image: String,
    pub cpus: String,
    pub memory: String,
    #[serde(default)]
    pub envs: Vec<EnvVarUpdate>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct EnvVarUpdate {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct RemoveServiceQuery {
    pub index: usize,
}

#[derive(Debug, Deserialize)]
pub struct ServiceIndexQuery {
    pub service_index: usize,
}

#[derive(Debug, Deserialize)]
pub struct RemoveEnvVarQuery {
    pub service_index: usize,
    pub env_index: usize,
}

pub async fn edit_config(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.config_manager.load_project(&name) {
        Ok(config) => {
            let template = EditTemplate {
                project: &config.project,
                version: VERSION,
            };
            render_template(template).into_response()
        }
        Err(e) => (
            StatusCode::NOT_FOUND,
            Html(format!("Project not found: {e}")),
        )
            .into_response(),
    }
}

pub async fn update_config(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(form): Json<UpdateProjectForm>,
) -> impl IntoResponse {
    let mut config = match state.config_manager.load_project(&name) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::NOT_FOUND,
                Html(format!("Project not found: {e}")),
            );
        }
    };

    config.project.name = form.name;
    config.project.domain = form.domain.filter(|s| !s.is_empty());
    config.project.services = form_to_services(form.services, &config.project.services);

    if let Err(e) = state.config_manager.save(&config) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("Failed to save config: {e}")),
        );
    }

    let raw_config = match state.config_manager.load_project_raw(&config.project.name) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("Failed to load raw config: {e}")),
            );
        }
    };

    let template = IndexContentTemplate {
        project: &config.project,
        raw_config,
    };
    render_template(template)
}

pub async fn add_service_to_config(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(form): Json<UpdateProjectForm>,
) -> impl IntoResponse {
    let mut config = match state.config_manager.load_project(&name) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::NOT_FOUND,
                Html(format!("Project not found: {e}")),
            )
                .into_response();
        }
    };

    config.project.name = form.name;
    config.project.domain = form.domain.filter(|s| !s.is_empty());

    let mut new_services = form_to_services(form.services, &config.project.services);

    let new_service_index = new_services.len() + 1;
    new_services.push(Service {
        name: format!("service_{new_service_index}"),
        image: "nginx:latest".to_string(),
        cpus: 250,
        memory: 64,
        dockerfile_path: None,
        command: None,
        internal_port: None,
        external_port: None,
        depends_on: vec![],
        envs: HashMap::new(),
    });

    config.project.services = new_services;

    if let Err(e) = state.config_manager.save(&config) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("Failed to save config: {e}")),
        )
            .into_response();
    }

    let template = EditContentTemplate {
        project: &config.project,
    };
    render_template(template).into_response()
}

pub async fn remove_service_from_config(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(query): Query<RemoveServiceQuery>,
    Json(form): Json<UpdateProjectForm>,
) -> impl IntoResponse {
    let mut config = match state.config_manager.load_project(&name) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::NOT_FOUND,
                Html(format!("Project not found: {e}")),
            )
                .into_response();
        }
    };

    config.project.name = form.name;
    config.project.domain = form.domain.filter(|s| !s.is_empty());

    let mut new_services = form_to_services(form.services, &config.project.services);

    if query.index < new_services.len() {
        new_services.remove(query.index);
    }

    config.project.services = new_services;

    if let Err(e) = state.config_manager.save(&config) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("Failed to save config: {e}")),
        )
            .into_response();
    }

    let template = EditContentTemplate {
        project: &config.project,
    };
    render_template(template).into_response()
}

pub async fn add_env_var_to_config(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(query): Query<ServiceIndexQuery>,
    Json(mut form): Json<UpdateProjectForm>,
) -> impl IntoResponse {
    let config = match state.config_manager.load_project(&name) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::NOT_FOUND,
                Html(format!("Project not found: {e}")),
            )
                .into_response();
        }
    };

    if query.service_index < form.services.len() {
        form.services[query.service_index].envs.push(EnvVarUpdate {
            key: "NEW_VAR".to_string(),
            value: "value".to_string(),
        });
    }

    let mut new_config = config;
    new_config.project.name = form.name;
    new_config.project.domain = form.domain.filter(|s| !s.is_empty());
    new_config.project.services = form_to_services(form.services, &new_config.project.services);

    if let Err(e) = state.config_manager.save(&new_config) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("Failed to save config: {e}")),
        )
            .into_response();
    }

    let template = EditContentTemplate {
        project: &new_config.project,
    };
    render_template(template).into_response()
}

pub async fn remove_env_var_from_config(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(query): Query<RemoveEnvVarQuery>,
    Json(mut form): Json<UpdateProjectForm>,
) -> impl IntoResponse {
    let config = match state.config_manager.load_project(&name) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::NOT_FOUND,
                Html(format!("Project not found: {e}")),
            )
                .into_response();
        }
    };

    if query.service_index < form.services.len() {
        if query.env_index < form.services[query.service_index].envs.len() {
            form.services[query.service_index]
                .envs
                .remove(query.env_index);
        }
    }

    let mut new_config = config;
    new_config.project.name = form.name;
    new_config.project.domain = form.domain.filter(|s| !s.is_empty());
    new_config.project.services = form_to_services(form.services, &new_config.project.services);

    if let Err(e) = state.config_manager.save(&new_config) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("Failed to save config: {e}")),
        )
            .into_response();
    }

    let template = EditContentTemplate {
        project: &new_config.project,
    };
    render_template(template).into_response()
}

pub async fn run_genesis(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let log_rx = state.log_sender.subscribe();
    let orchestrator = state.orchestrator.clone();
    let config_manager = state.config_manager.clone();

    tokio::spawn(async move {
        match config_manager.load_project(&name) {
            Ok(config) => match orchestrator.genesis(&config).await {
                Ok(()) => tracing::info!("Genesis completed successfully!"),
                Err(e) => tracing::error!("Genesis failed: {e}"),
            },
            Err(e) => tracing::error!("Failed to load project {name} for genesis: {e}"),
        }
    });

    let stream = BroadcastStream::new(log_rx).filter_map(|msg| match msg {
        Ok(s) => Some(Ok(Event::default().data(s))),
        Err(_) => None,
    });

    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}

pub async fn run_apply(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let log_rx = state.log_sender.subscribe();
    let orchestrator = state.orchestrator.clone();
    let config_manager = state.config_manager.clone();

    tokio::spawn(async move {
        match config_manager.load_project(&name) {
            Ok(config) => match orchestrator.apply(&config).await {
                Ok(()) => tracing::info!("Apply completed successfully!"),
                Err(e) => tracing::error!("Apply failed: {e}"),
            },
            Err(e) => tracing::error!("Failed to load project {name} for apply: {e}"),
        }
    });

    let stream = BroadcastStream::new(log_rx).filter_map(|msg| match msg {
        Ok(s) => Some(Ok(Event::default().data(s))),
        Err(_) => None,
    });

    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}

pub async fn run_destroy(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let log_rx = state.log_sender.subscribe();
    let orchestrator = state.orchestrator.clone();
    let config_manager = state.config_manager.clone();

    tokio::spawn(async move {
        match config_manager.load_project(&name) {
            Ok(config) => match orchestrator.destroy(&config).await {
                Ok(()) => tracing::info!("Destroy completed successfully!"),
                Err(e) => tracing::error!("Destroy failed: {e}"),
            },
            Err(e) => tracing::error!("Failed to load project {name} for destroy: {e}"),
        }
    });

    let stream = BroadcastStream::new(log_rx).filter_map(|msg| match msg {
        Ok(s) => Some(Ok(Event::default().data(s))),
        Err(_) => None,
    });

    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}

fn render_template<T: Template>(template: T) -> (StatusCode, Html<String>) {
    match template.render() {
        Ok(html) => (StatusCode::OK, Html(html)),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("Template error: {err}")),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_form_to_services_update() {
        // Arrange
        let existing = vec![Service {
            name: "web".to_string(),
            image: "nginx:1.0".to_string(),
            cpus: 100,
            memory: 128,
            dockerfile_path: Some("Dockerfile".to_string()),
            command: None,
            internal_port: Some(80),
            external_port: None,
            depends_on: vec![],
            envs: HashMap::new(),
        }];

        let updates = vec![ServiceUpdate {
            name: "web".to_string(),
            image: "nginx:latest".to_string(),
            cpus: "200".to_string(),
            memory: "256".to_string(),
            envs: vec![],
        }];

        // Act
        let result = form_to_services(updates, &existing);

        // Assert
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "web");
        assert_eq!(result[0].image, "nginx:latest"); // Updated
        assert_eq!(result[0].cpus, 200); // Updated
        assert_eq!(result[0].memory, 256); // Updated
        assert_eq!(result[0].dockerfile_path, Some("Dockerfile".to_string())); // Preserved
        assert_eq!(result[0].internal_port, Some(80)); // Preserved
    }

    #[test]
    fn test_form_to_services_new() {
        // Arrange
        let existing = vec![];
        let updates = vec![ServiceUpdate {
            name: "db".to_string(),
            image: "postgres".to_string(),
            cpus: "500".to_string(),
            memory: "1024".to_string(),
            envs: vec![EnvVarUpdate {
                key: "POSTGRES_PASSWORD".to_string(),
                value: "secret".to_string(),
            }],
        }];

        // Act
        let result = form_to_services(updates, &existing);

        // Assert
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "db");
        assert_eq!(result[0].image, "postgres");
        assert_eq!(result[0].cpus, 500);
        assert_eq!(
            result[0].envs.get("POSTGRES_PASSWORD"),
            Some(&"secret".to_string())
        );
        assert_eq!(result[0].dockerfile_path, None); // Default
    }
}
