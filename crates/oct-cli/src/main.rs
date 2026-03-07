use std::collections::HashMap;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,

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
    /// Run Genesis step
    Genesis,
    /// Apply the application
    Apply,
    /// Destroy the application
    Destroy {
        /// Path to the state file (skips oct.toml when provided)
        #[clap(long)]
        state_path: Option<String>,
    },
    /// Deploy a single container in one step (genesis + apply)
    Run {
        /// Container image to deploy
        #[clap(long)]
        image: String,

        /// Project and service name
        #[clap(long, default_value = "oct-run")]
        name: String,

        /// CPU millicores
        #[clap(long, default_value_t = 2000)]
        cpus: u32,

        /// Memory in MB
        #[clap(long, default_value_t = 4096)]
        memory: u64,

        /// External port on the host
        #[clap(long, default_value_t = 80)]
        external_port: u32,

        /// Internal port inside the container
        #[clap(long, default_value_t = 22)]
        internal_port: u32,

        /// Environment variable in KEY=VALUE format (repeatable)
        #[clap(short = 'e', long = "env")]
        envs: Vec<String>,

        /// Path to save infrastructure state
        #[clap(long, default_value = "./oct-run-state.json")]
        state_path: String,
    },
}

/// Builds a [`oct_config::Config`] from inline CLI arguments instead of reading
/// `oct.toml`.
fn build_inline_config(
    name: &str,
    image: &str,
    cpus: u32,
    memory: u64,
    external_port: u32,
    internal_port: u32,
    envs: &[String],
    state_path: &str,
) -> Result<oct_config::Config, Box<dyn std::error::Error + Send + Sync>> {
    let env_map: HashMap<String, String> = envs
        .iter()
        .map(|entry| {
            let (key, value) = entry
                .split_once('=')
                .ok_or_else(|| format!("Invalid env format '{entry}', expected KEY=VALUE"))?;
            Ok((key.to_string(), value.to_string()))
        })
        .collect::<Result<_, Box<dyn std::error::Error + Send + Sync>>>()?;

    let service = oct_config::Service {
        name: name.to_string(),
        image: image.to_string(),
        dockerfile_path: None,
        command: None,
        internal_port: Some(internal_port),
        external_port: Some(external_port),
        cpus,
        memory,
        depends_on: vec![],
        envs: env_map,
    };

    let user_state_path = state_path.replace(".json", "-user.json");

    Ok(oct_config::Config {
        project: oct_config::Project {
            name: name.to_string(),
            state_backend: oct_config::StateBackend::Local {
                path: state_path.to_string(),
            },
            user_state_backend: oct_config::StateBackend::Local {
                path: user_state_path,
            },
            services: vec![service],
            domain: None,
        },
    })
}

/// Builds a minimal [`oct_config::Config`] with only a local state backend for
/// destroy.
fn build_destroy_config(state_path: &str) -> oct_config::Config {
    let user_state_path = state_path.replace(".json", "-user.json");

    oct_config::Config {
        project: oct_config::Project {
            name: "oct-run".to_string(),
            state_backend: oct_config::StateBackend::Local {
                path: state_path.to_string(),
            },
            user_state_backend: oct_config::StateBackend::Local {
                path: user_state_path,
            },
            services: vec![],
            domain: None,
        },
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();

    let orchestrator = oct_orchestrator::OrchestratorWithGraph;

    match cli.command {
        Commands::Genesis => {
            let config = oct_config::Config::new(None)?;
            orchestrator.genesis(&config).await?;
        }
        Commands::Apply => {
            let config = oct_config::Config::new(None)?;
            orchestrator.apply(&config).await?;
        }
        Commands::Destroy { state_path } => {
            let config = match state_path {
                Some(path) => build_destroy_config(&path),
                None => oct_config::Config::new(None)?,
            };
            orchestrator.destroy(&config).await?;
        }
        Commands::Run {
            image,
            name,
            cpus,
            memory,
            external_port,
            internal_port,
            envs,
            state_path,
        } => {
            let config = build_inline_config(
                &name,
                &image,
                cpus,
                memory,
                external_port,
                internal_port,
                &envs,
                &state_path,
            )?;

            orchestrator.genesis(&config).await?;
            orchestrator.apply(&config).await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[test]
    fn test_cli_default_paths() {
        // Arrange
        let cli = Cli::parse_from(["app", "apply"]);

        // Assert
        assert_eq!(cli.user_state_file_path, "./user_state.json");
        assert_eq!(cli.dockerfile_path, ".");
        assert_eq!(cli.context_path, ".");
    }

    #[test]
    fn test_run_command_parses() {
        // Arrange
        let cli = Cli::parse_from(["app", "run", "--image", "nginx:latest"]);

        // Assert
        match cli.command {
            Commands::Run {
                image,
                name,
                cpus,
                memory,
                external_port,
                internal_port,
                envs,
                state_path,
            } => {
                assert_eq!(image, "nginx:latest");
                assert_eq!(name, "oct-run");
                assert_eq!(cpus, 2000);
                assert_eq!(memory, 4096);
                assert_eq!(external_port, 80);
                assert_eq!(internal_port, 22);
                assert!(envs.is_empty());
                assert_eq!(state_path, "./oct-run-state.json");
            }
            _ => panic!("Expected Commands::Run"),
        }
    }

    #[test]
    fn test_run_command_all_flags() {
        // Arrange
        let cli = Cli::parse_from([
            "app",
            "run",
            "--image",
            "ghcr.io/org/img:v1",
            "--name",
            "my-svc",
            "--cpus",
            "4000",
            "--memory",
            "8192",
            "--external-port",
            "8080",
            "--internal-port",
            "80",
            "-e",
            "KEY1=val1",
            "-e",
            "KEY2=val2",
            "--state-path",
            "/tmp/state.json",
        ]);

        // Assert
        match cli.command {
            Commands::Run {
                image,
                name,
                cpus,
                memory,
                external_port,
                internal_port,
                envs,
                state_path,
            } => {
                assert_eq!(image, "ghcr.io/org/img:v1");
                assert_eq!(name, "my-svc");
                assert_eq!(cpus, 4000);
                assert_eq!(memory, 8192);
                assert_eq!(external_port, 8080);
                assert_eq!(internal_port, 80);
                assert_eq!(envs, vec!["KEY1=val1", "KEY2=val2"]);
                assert_eq!(state_path, "/tmp/state.json");
            }
            _ => panic!("Expected Commands::Run"),
        }
    }

    #[test]
    fn test_destroy_with_state_path() {
        // Arrange
        let cli = Cli::parse_from(["app", "destroy", "--state-path", "s.json"]);

        // Assert
        match cli.command {
            Commands::Destroy { state_path } => {
                assert_eq!(state_path, Some("s.json".to_string()));
            }
            _ => panic!("Expected Commands::Destroy"),
        }
    }

    #[test]
    fn test_destroy_without_state_path() {
        // Arrange
        let cli = Cli::parse_from(["app", "destroy"]);

        // Assert
        match cli.command {
            Commands::Destroy { state_path } => {
                assert_eq!(state_path, None);
            }
            _ => panic!("Expected Commands::Destroy"),
        }
    }

    #[test]
    fn test_build_inline_config_defaults() {
        // Arrange / Act
        let config = build_inline_config(
            "oct-run",
            "nginx:latest",
            2000,
            4096,
            80,
            22,
            &[],
            "./oct-run-state.json",
        )
        .expect("should build config");

        // Assert
        assert_eq!(config.project.name, "oct-run");
        assert_eq!(config.project.services.len(), 1);

        let svc = &config.project.services[0];
        assert_eq!(svc.name, "oct-run");
        assert_eq!(svc.image, "nginx:latest");
        assert_eq!(svc.cpus, 2000);
        assert_eq!(svc.memory, 4096);
        assert_eq!(svc.internal_port, Some(22));
        assert_eq!(svc.external_port, Some(80));
        assert!(svc.envs.is_empty());
        assert!(config.project.domain.is_none());

        assert_eq!(
            config.project.state_backend,
            oct_config::StateBackend::Local {
                path: "./oct-run-state.json".to_string()
            }
        );
        assert_eq!(
            config.project.user_state_backend,
            oct_config::StateBackend::Local {
                path: "./oct-run-state-user.json".to_string()
            }
        );
    }

    #[test]
    fn test_build_inline_config_with_port_and_envs() {
        // Arrange / Act
        let config = build_inline_config(
            "my-app",
            "ubuntu:24.04",
            4000,
            8192,
            80,
            22,
            &["FOO=bar".to_string(), "BAZ=qux".to_string()],
            "./state.json",
        )
        .expect("should build config");

        // Assert
        let svc = &config.project.services[0];
        assert_eq!(svc.internal_port, Some(22));
        assert_eq!(svc.external_port, Some(80));
        assert_eq!(svc.envs.get("FOO"), Some(&"bar".to_string()));
        assert_eq!(svc.envs.get("BAZ"), Some(&"qux".to_string()));
    }

    #[test]
    fn test_build_inline_config_invalid_env() {
        // Arrange / Act
        let result = build_inline_config(
            "oct-run",
            "nginx",
            2000,
            4096,
            80,
            22,
            &["INVALID_NO_EQUALS".to_string()],
            "./state.json",
        );

        // Assert
        let err = result.expect_err("should fail for invalid env format");
        assert!(
            err.to_string().contains("KEY=VALUE"),
            "Error should mention KEY=VALUE format, got: {err}"
        );
    }

    #[test]
    fn test_build_destroy_config() {
        // Arrange / Act
        let config = build_destroy_config("/tmp/my-state.json");

        // Assert
        assert_eq!(
            config.project.state_backend,
            oct_config::StateBackend::Local {
                path: "/tmp/my-state.json".to_string()
            }
        );
        assert!(config.project.services.is_empty());
    }

    #[test]
    fn test_run_command_missing_image_fails() {
        // Assert
        let result = Cli::try_parse_from(["app", "run"]);
        assert!(result.is_err(), "run without --image should fail");
    }
}
