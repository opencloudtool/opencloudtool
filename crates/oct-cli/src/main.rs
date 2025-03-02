use clap::{Parser, Subcommand};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,

    /// Path to the infra state file
    #[clap(long, default_value = "./state.json")]
    state_file_path: String,

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

    let orchestrator =
        oct_orchestrator::Orchestrator::new(cli.state_file_path, cli.user_state_file_path);

    match &cli.command {
        Commands::Deploy => orchestrator.deploy().await?,
        Commands::Destroy => orchestrator.destroy().await?,
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_cmd::Command;
    use clap::Parser;
    use predicates::prelude::*;

    #[test]
    fn test_cli_default_paths() {
        // Arrange
        let cli = Cli::parse_from(["app", "deploy"]);

        // Assert
        assert_eq!(cli.state_file_path, "./state.json");
        assert_eq!(cli.user_state_file_path, "./user_state.json");
        assert_eq!(cli.dockerfile_path, ".");
        assert_eq!(cli.context_path, ".");
    }

    #[tokio::test]
    async fn test_main_deploy_no_oct_toml() {
        // Arrange
        let mut oct_cli_bin = Command::cargo_bin(assert_cmd::crate_name!()).unwrap();

        // Act
        let cmd = oct_cli_bin.arg("deploy");

        // Assert
        cmd.assert().failure().stderr(predicate::str::contains(
            "Failed to read config file oct.toml",
        ));
    }

    #[tokio::test]
    async fn test_main_destroy_no_oct_toml() {
        // Arrange
        let mut oct_cli_bin = Command::cargo_bin(assert_cmd::crate_name!()).unwrap();

        // Act
        let cmd = oct_cli_bin.arg("destroy");

        // Assert
        cmd.assert().failure().stderr(predicate::str::contains(
            "Failed to read config file oct.toml",
        ));
    }
}
