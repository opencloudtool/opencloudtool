use crate::handlers::{
    AppState, add_env_var_to_config, add_service_to_config, create_project_action, edit_config,
    list_projects, project_dashboard, remove_env_var_from_config, remove_service_from_config,
    root_redirect, run_apply, run_destroy, run_genesis, update_config, view_state,
};
use axum::{
    Router,
    routing::{get, post, put},
};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(root_redirect))
        .route("/projects", get(list_projects).post(create_project_action))
        .route("/projects/{name}", get(project_dashboard))
        .route("/projects/{name}/edit", get(edit_config))
        .route("/projects/{name}/state", get(view_state))
        .route("/projects/{name}/config", put(update_config))
        .route(
            "/projects/{name}/config/add-service",
            post(add_service_to_config),
        )
        .route(
            "/projects/{name}/config/remove-service",
            post(remove_service_from_config),
        )
        .route(
            "/projects/{name}/config/add-env-var",
            post(add_env_var_to_config),
        )
        .route(
            "/projects/{name}/config/remove-env-var",
            post(remove_env_var_from_config),
        )
        .route("/projects/{name}/action/genesis", get(run_genesis))
        .route("/projects/{name}/action/apply", get(run_apply))
        .route("/projects/{name}/action/destroy", get(run_destroy))
        .with_state(state)
}
