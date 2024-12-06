use clap::{Parser, Subcommand};
use log;
use oct_cloud::aws;
use oct_cloud::aws::Resource;

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
            let mut instance = aws::Ec2Instance::new(
                "us-west-2".to_string(),
                "ami-04dd23e62ed049936".to_string(),
                aws::aws_sdk_ec2::types::InstanceType::T2Micro,
                "oct-cli".to_string(),
            )
            .await;
            instance.id = Some("".to_string()); // Put instance id here
            instance.arn = Some("".to_string());
            instance.public_ip = Some("".to_string());
            instance.public_dns = Some("".to_string());

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
