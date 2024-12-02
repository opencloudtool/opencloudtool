use clap::{Parser, Subcommand};
use log;
use oct_cloud::aws;
use oct_cloud::aws::Resource;
use oct_cloud::aws::State;

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

            log::info!("Instance created: {}", instance.id.ok_or("No instance id")?);
        }
        Commands::Destroy(args) => {
            // Load the state file
            let mut state = State::load("state.json");

            // Retrieve instance info from the state file
            let instance_info = state
                .get_instance("instance_id")
                .expect("Instance not found in the state file");

            let mut instance = aws::Ec2Instance::new(
                instance_info.region.clone(),
                instance_info.ami.clone(),
                instance_info.instance_type.clone().into(),
                instance_info.name.clone(),
            )
            .await;

            instance.id = Some(instance_info.id.clone());
            instance.arn = Some(instance_info.arn.clone());
            instance.public_ip = Some(instance_info.public_ip.clone());
            instance.public_dns = Some(instance_info.public_dns.clone());

            instance.destroy().await?;

            log::info!("Instance destroyed");
        }
    }

    Ok(())
}

// TODO: Add tests
// #[cfg(test)]
// mod tests {
// }
