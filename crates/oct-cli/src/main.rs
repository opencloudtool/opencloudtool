use clap::{Parser, Subcommand};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,

    /// Path to the user state file
    #[clap(long, default_value = "./user_state.json")]
    user_state_file_path: String,

    /// Path to the Dockerfile
    #[clap(long, default_value = ".")]
    dockerfile_path: String,

    /// Context path
    #[clap(long, default_value = ".")]
    context_path: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Deploy the application
    Deploy,
    /// Destroy the application
    Destroy,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();

    let orchestrator_with_graph = oct_orchestrator::OrchestratorWithGraph;

    match &cli.command {
        Commands::Deploy => orchestrator_with_graph.deploy().await?,
        Commands::Destroy => orchestrator_with_graph.destroy().await?,
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_cli_default_paths() {
        // Arrange
        let cli = Cli::parse_from(["app", "deploy"]);

        // Assert
        assert_eq!(cli.user_state_file_path, "./user_state.json");
        assert_eq!(cli.dockerfile_path, ".");
        assert_eq!(cli.context_path, ".");
    }
}
