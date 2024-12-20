use std::fs;

use clap::{Parser, Subcommand};
use log;
use oct_cloud::aws;
use oct_cloud::aws::Resource;
use oct_cloud::state;

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

    /// Path to the Dockerfile
    #[clap(long, default_value = ".")]
    dockerfile_path: String,

    /// Context path
    #[clap(long, default_value = ".")]
    context_path: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();

    let state_file_path = "./state.json";

    match &cli.command {
        Commands::Deploy(args) => {
            // Create EC2 instance
            let mut instance = aws::Ec2Instance::new(
                "us-west-2".to_string(),
                "ami-04dd23e62ed049936".to_string(),
                aws::aws_sdk_ec2::types::InstanceType::T2Micro,
                "oct-cli".to_string(),
            )
            .await;

            instance.create().await?;

            let instance_id = instance.id.clone().ok_or("No instance id")?;

            log::info!("Instance created: {}", instance_id);

            // Save to state file
            let instance_state = state::Ec2InstanceState::new(instance).await;
            let json_data = serde_json::to_string_pretty(&instance_state)?;
            fs::write(state_file_path, json_data)?;

            log::info!("Instance: {} saved to state file", instance_id);
        }
        Commands::Destroy(args) => {
            // Load instance from state file
            let json_data = fs::read_to_string(state_file_path).expect("Unable to read file");
            let state: state::Ec2InstanceState = serde_json::from_str(&json_data)?;

            // Create EC2 instance from state
            let mut instance = state.build_from_state().await?;

            // Destroy EC2 instance
            instance.destroy().await?;

            log::info!("Instance destroyed");

            // Remove instance from state file
            fs::remove_file(state_file_path).expect("Unable to remove file");

            log::info!("Instance removed from state file");
        }
    }

    Ok(())
}

// TODO: Add tests
// #[cfg(test)]
// mod tests {
// }
