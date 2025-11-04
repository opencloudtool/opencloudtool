use assert_cmd::Command;
use predicates::prelude::*;

#[tokio::test]
async fn test_deploy_no_oct_toml() {
    // Arrange
    let mut oct_cli_bin = Command::new(assert_cmd::cargo::cargo_bin!());

    // Act
    let cmd = oct_cli_bin.arg("deploy");

    // Assert
    cmd.assert().failure().stderr(predicate::str::contains(
        "Failed to read config file oct.toml",
    ));
}

#[tokio::test]
async fn test_destroy_no_oct_toml() {
    // Arrange
    let mut oct_cli_bin = Command::new(assert_cmd::cargo::cargo_bin!());

    // Act
    let cmd = oct_cli_bin.arg("destroy");

    // Assert
    cmd.assert().failure().stderr(predicate::str::contains(
        "Failed to read config file oct.toml",
    ));
}
