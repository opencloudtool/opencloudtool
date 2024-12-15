use std::fs;

use clap::{Parser, Subcommand};
use log;
use oct_cloud::aws::Resource;
use oct_cloud::aws::{
    self, Ec2Impl, Ec2Instance, Ec2InstanceState, InstanceProfile, InstanceProfileState,
    InstanceRole, InstanceRoleState,
};

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
                aws::aws_sdk_ec2::types::InstanceType::T2Micro.to_string(),
                "oct-cli".to_string(),
            )
            .await;

            instance.create().await?;

            // Save to state file
            let instance_state = Ec2InstanceState {
                id: instance.id.clone().unwrap_or_default(),
                arn: instance.arn.clone().unwrap_or_default(),
                public_ip: instance.public_ip.clone().unwrap_or_default(),
                public_dns: instance.public_dns.clone().unwrap_or_default(),
                region: instance.region.clone(),
                ami: instance.ami.clone(),
                instance_type: instance.instance_type.clone(),
                name: instance.name.clone(),
                instance_profile: instance.instance_profile.as_ref().map(|ip| {
                    InstanceProfileState {
                        name: ip.name.clone(),
                        region: ip.region.clone(),
                        instance_roles: ip
                            .instance_roles
                            .iter()
                            .map(|ir| InstanceRoleState {
                                name: ir.name.clone(),
                                region: ir.region.clone(),
                                assume_role_policy: ir.assume_role_policy.clone(),
                                policy_arns: ir.policy_arns.clone(),
                            })
                            .collect(),
                    }
                }),
            };

            let json_data = serde_json::to_string_pretty(&instance_state)?;
            fs::write(state_file_path, json_data)?;

            log::info!("Instance created: {}", instance.id.ok_or("No instance id")?);
        }
        Commands::Destroy(args) => {
            // Load instance from state file
            let json_data = fs::read_to_string(state_file_path).expect("Unable to read file");
            let state: aws::Ec2InstanceState = serde_json::from_str(&json_data)?;

            let mut instance = aws::Ec2Instance::new_from_state(state).await?;

            instance.destroy().await?;

            log::info!("Instance destroyed");

            // Remove instance from state file
            fs::remove_file(state_file_path).expect("Unable to remove file");
        }
    }

    Ok(())
}

// TODO: Add tests
// #[cfg(test)]
// mod tests {
// }
