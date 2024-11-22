use clap::{Parser, Subcommand};
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
    let cli = Cli::parse();

    match &cli.command {
        Commands::Deploy(args) => {
            // Create EC2 instance
            let mut instance = aws::Ec2Instance::new(
                "us-west-2".to_string(),
                "ami-0c65adc9a5c1b5d7c".to_string(),
                aws::aws_sdk_ec2::types::InstanceType::T2Micro,
                "oct-cli".to_string(),
                "Hello, World!".to_string(),
            ).await;

            instance.create().await?;

            println!("Instance created: {instance:?}");
        }
        Commands::Destroy(args) => {
            let mut instance = aws::Ec2Instance::new(
                "us-west-2".to_string(),
                "ami-0c65adc9a5c1b5d7c".to_string(),
                aws::aws_sdk_ec2::types::InstanceType::T2Micro,
                "oct-cli".to_string(),
                "Hello, World!".to_string(),
            ).await;
            instance.id = Some("".to_string()); // Put instance id here
            instance.arn = Some("".to_string());
            instance.public_ip = Some("".to_string());
            instance.public_dns = Some("".to_string());

            instance.destroy().await?;

            println!("Instance destroyed: {instance:?}");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::State;

    use oct_cloud::aws;

    use assert_cmd::Command;
    use predicates::prelude::*;
    use std::env;

    use std::sync::Once;

    static SETUP: Once = Once::new();

    // TODO: Move to ct-test-utils crate
    pub fn setup() {
        SETUP.call_once(|| {
            if env::var("AWS_ENDPOINT_URL").is_err() {
                env::set_var("AWS_ENDPOINT_URL", "http://localhost:4566");
            }
            if env::var("AWS_REGION").is_err() {
                env::set_var("AWS_REGION", "eu-central-1");
            }
            if env::var("AWS_ACCESS_KEY_ID").is_err() {
                env::set_var("AWS_ACCESS_KEY_ID", "test");
            }
            if env::var("AWS_SECRET_ACCESS_KEY").is_err() {
                env::set_var("AWS_SECRET_ACCESS_KEY", "test");
            }
        });
    }

    #[test]
    fn test_deploy_command() {
        setup();

        let temp_dir = tempfile::tempdir().unwrap();
        let state_file = temp_dir.path().join("state.json");

        let mut cmd = Command::cargo_bin("oct-cli").unwrap();
        cmd.arg("deploy")
            .arg("--state-file-path")
            .arg(state_file.to_str().unwrap())
            .assert()
            .success()
            .stdout(predicate::str::contains("Instance ID:"));
    }

    #[tokio::test]
    async fn test_destroy_command() {
        setup();

        let instance_id = aws::create_ec2_instance("1234567890").await.unwrap();

        let temp_dir = tempfile::tempdir().unwrap();
        let state_file = temp_dir.path().join("state.json");

        let state = State {
            instance_id: String::from(&instance_id),
        };
        let state_json = serde_json::to_string(&state).unwrap();

        std::fs::write(&state_file, state_json).unwrap();

        let mut cmd = Command::cargo_bin("oct-cli").unwrap();
        cmd.arg("destroy")
            .arg("--state-file-path")
            .arg(state_file.to_str().unwrap())
            .assert()
            .success()
            .stdout(format!("Destroying instance: {}\n", instance_id));
    }

    #[test]
    fn test_invalid_command() {
        let mut cmd = Command::cargo_bin("oct-cli").unwrap();
        cmd.arg("invalid").assert().failure();
    }
}
