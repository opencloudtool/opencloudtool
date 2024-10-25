use clap::{Parser, Subcommand};
use ct_cloud::create_ec2_instance;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
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
    let cli = Cli::parse();

    match &cli.command {
        Commands::Deploy => {
            let instance_id = create_ec2_instance().await?;
            println!("Instance ID: {}", instance_id);
        }
        Commands::Destroy => {
            println!("Destroying the application...");
            // Add destruction logic here
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use assert_cmd::Command;
    use predicates::prelude::*;
    use std::env;

    #[test]
    fn test_deploy_command() {
        if env::var("AWS_ENDPOINT_URL").is_err() {
            env::set_var("AWS_ENDPOINT_URL", "http://localhost:4566");
        }

        let mut cmd = Command::cargo_bin("ct-cli").unwrap();
        cmd.arg("deploy")
            .assert()
            .success()
            .stdout(predicate::str::contains("Instance ID:"));
    }

    #[test]
    fn test_destroy_command() {
        let mut cmd = Command::cargo_bin("ct-cli").unwrap();
        cmd.arg("destroy")
            .assert()
            .success()
            .stdout("Destroying the application...\n");
    }

    #[test]
    fn test_invalid_command() {
        let mut cmd = Command::cargo_bin("ct-cli").unwrap();
        cmd.arg("invalid").assert().failure();
    }
}
