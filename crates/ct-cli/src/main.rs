use clap::{Parser, Subcommand};
use ct_cloud::create_ec2_instance;
use serde_derive::{Deserialize, Serialize};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Deploy the application
    Deploy(CommandArgs),
    /// Destroy the application
    Destroy(CommandArgs),
}

#[derive(Parser)]
struct CommandArgs {
    /// Path to the state file
    #[clap(long, default_value = "./state.json")]
    state_file_path: String,
}

#[derive(Serialize, Deserialize)]
struct State {
    instance_id: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Deploy(args) => {
            let instance_id = create_ec2_instance().await?;
            println!("Instance ID: {}", instance_id);

            let state = State { instance_id };
            let state_json = serde_json::to_string(&state)?;
            std::fs::write(&args.state_file_path, state_json)?;

            println!("State saved to {}", args.state_file_path);
        }
        Commands::Destroy(args) => {
            let state = std::fs::read_to_string(&args.state_file_path)?;
            let state: State = serde_json::from_str(&state)?;

            println!("Destroying instance: {}", state.instance_id);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::State;

    use assert_cmd::Command;
    use predicates::prelude::*;
    use std::env;

    #[test]
    fn test_deploy_command() {
        if env::var("AWS_ENDPOINT_URL").is_err() {
            env::set_var("AWS_ENDPOINT_URL", "http://localhost:4566");
        }

        let temp_dir = tempfile::tempdir().unwrap();
        let state_file = temp_dir.path().join("state.json");

        let mut cmd = Command::cargo_bin("ct-cli").unwrap();
        cmd.arg("deploy")
            .arg("--state-file-path")
            .arg(state_file.to_str().unwrap())
            .assert()
            .success()
            .stdout(predicate::str::contains("Instance ID:"));
    }

    #[test]
    fn test_destroy_command() {
        let instance_id = "i-0102f03dc608749c3";

        let temp_dir = tempfile::tempdir().unwrap();
        let state_file = temp_dir.path().join("state.json");

        let state = State {
            instance_id: String::from(instance_id),
        };
        let state_json = serde_json::to_string(&state).unwrap();

        std::fs::write(&state_file, state_json).unwrap();

        let mut cmd = Command::cargo_bin("ct-cli").unwrap();
        cmd.arg("destroy")
            .arg("--state-file-path")
            .arg(state_file.to_str().unwrap())
            .assert()
            .success()
            .stdout(format!("Destroying instance: {}\n", instance_id));
    }

    #[test]
    fn test_invalid_command() {
        let mut cmd = Command::cargo_bin("ct-cli").unwrap();
        cmd.arg("invalid").assert().failure();
    }
}
