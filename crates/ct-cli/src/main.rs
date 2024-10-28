use clap::{Parser, Subcommand};
use ct_cloud::aws;
use serde_derive::{Deserialize, Serialize};
use std::process::Command;

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
            // Create ECR repository
            let repository_uri = aws::create_ecr_repository("ct-app").await?;
            println!("ECR repository URI: {}", repository_uri);

            // Try to find Dockerfile in the current directory
            let dockerfile_path = std::fs::read_dir(".")?.find(|entry| {
                entry
                    .as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .ends_with("Dockerfile")
            });
            println!("Dockerfile path: {:?}", dockerfile_path);

            // Build docker image
            let image_tag = format!("{}:latest", repository_uri);
            let _build_result = Command::new("docker")
                .arg("build")
                .arg("-t")
                .arg(&image_tag)
                .arg("--platform")
                .arg("linux/amd64")
                .arg(".")
                .output()?;

            println!("Docker image {} built successfully", image_tag);

            // Push docker image to ECR repository
            let push_result = Command::new("docker")
                .arg("push")
                .arg(&image_tag)
                .status()?;

            if !push_result.success() {
                return Err(format!("Failed to push docker image: {}", image_tag).into());
            }

            println!("Docker image {} pushed successfully", image_tag);

            // Create EC2 instance
            let instance_id = aws::create_ec2_instance(&image_tag).await?;
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

            aws::destroy_ec2_instance(&state.instance_id).await?;

            // Delete ECR repository
            aws::delete_ecr_repository("ct-app").await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::State;

    use ct_cloud::aws;

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

        let mut cmd = Command::cargo_bin("ct-cli").unwrap();
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
