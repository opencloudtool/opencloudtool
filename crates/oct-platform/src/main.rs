use std::sync::Arc;
use tokio::sync::broadcast;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use oct_platform::config_manager::{ConfigManager, FileConfigManager, WorkspaceConfigManager};
use oct_platform::handlers::AppState;
use oct_platform::logging::LogLayer;
use oct_platform::orchestrator::{MockOrchestrator, Orchestrator, RealOrchestrator};
use oct_platform::routes::router;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (log_sender, _rx) = broadcast::channel(1000);

    let log_layer = LogLayer {
        sender: log_sender.clone(),
    };

    let args: Vec<String> = std::env::args().collect();
    let verbose = args.iter().any(|arg| arg == "--verbose" || arg == "-v");

    let default_filter = if verbose {
        "debug,tower_http=debug,oct_platform=debug,oct_config=info,oct_cloud=debug,oct_orchestrator=debug,oct_ctl_sdk=debug"
    } else {
        "warn,oct_platform=info,oct_cloud=info,oct_orchestrator=info,oct_ctl_sdk=info"
    };

    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(env_filter)
        .with(log_layer)
        .init();

    let orchestrator: Arc<dyn Orchestrator> = if std::env::var("OCT_PLATFORM_MOCK").is_ok() {
        tracing::info!("Starting in MOCK mode");
        Arc::new(MockOrchestrator::default())
    } else {
        Arc::new(RealOrchestrator)
    };

    let config_path_env = std::env::var("OCT_CONFIG_PATH")
        .ok()
        .filter(|s| !s.is_empty());

    let config_manager: Arc<dyn ConfigManager> = if let Some(path) = config_path_env {
        tracing::info!("Using FileConfigManager with path: {path}");
        Arc::new(FileConfigManager::new(&path))
    } else {
        tracing::info!("Using WorkspaceConfigManager");
        Arc::new(WorkspaceConfigManager::new()?)
    };

    let port = std::env::var("OCT_PLATFORM_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(3000);

    let state = AppState {
        orchestrator,
        config_manager,
        log_sender,
    };

    let app = router(state).layer(TraceLayer::new_for_http());

    #[cfg(debug_assertions)]
    let app = app.layer(tower_livereload::LiveReloadLayer::new().request_predicate(
        |req: &axum::http::Request<_>| !req.headers().contains_key("hx-request"),
    ));

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .map_err(|e| format!("Failed to bind listener to 0.0.0.0:{port}: {e}"))?;
    tracing::info!("Listening on http://0.0.0.0:{port}");
    axum::serve(listener, app)
        .await
        .map_err(|e| format!("Server error: {e}").into())
}
