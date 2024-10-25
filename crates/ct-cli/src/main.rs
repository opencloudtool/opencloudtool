use clap::{Parser, Subcommand};

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

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Deploy => {
            println!("Deploying the application...");
            // Add deployment logic here
        }
        Commands::Destroy => {
            println!("Destroying the application...");
            // Add destruction logic here
        }
    }
}

#[cfg(test)]
mod tests {
    use assert_cmd::Command;

    #[test]
    fn test_deploy_command() {
        let mut cmd = Command::cargo_bin("ct-cli").unwrap();
        cmd.arg("deploy")
            .assert()
            .success()
            .stdout("Deploying the application...\n");
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
