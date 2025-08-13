use std::process::{Command, Output};

/// CLI command executor
#[derive(Clone, Default)]
pub(crate) struct CommandExecutor;

impl CommandExecutor {
    #[allow(clippy::unused_self)]
    pub(crate) fn execute(&self, command: &mut Command) -> Result<Output, std::io::Error> {
        command.output()
    }
}

#[cfg(test)]
pub(crate) mod mocks {
    use super::*;

    use mockall::mock;

    mock! {
        pub(crate) CommandExecutor {
            pub(crate) fn execute(&self, command: &mut Command) -> Result<Output, std::io::Error>;
        }

        impl Clone for CommandExecutor {
            fn clone(&self) -> Self;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_executor_execute() {
        // Arrange
        let command_executor = CommandExecutor;

        let mut command = Command::new("echo");
        command.args(["hello"]);

        // Act
        let output = command_executor.execute(&mut command);

        // Assert
        assert!(output.is_ok());

        let unwrapped_output = output.expect("Failed to get output");

        assert!(unwrapped_output.status.success());

        assert_eq!(unwrapped_output.stdout, b"hello\n");
        assert_eq!(unwrapped_output.stderr, b"");
    }
}
