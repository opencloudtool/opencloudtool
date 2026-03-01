# Contributing

Thanks for contributing to opencloudtool.

## Development setup

### Install pre-commit hooks

```bash
pre-commit install
```

### Build the project

```bash
cargo build
```

## CLI command examples

### Run deploy command

```bash
cd dir/with/oct.toml
cargo run -p oct-cli deploy
```

### Run destroy command

```bash
cargo run -p oct-cli destroy
```

### Show all available commands

```bash
cargo run -p oct-cli --help
```

### Show all available parameters for a command

```bash
cargo run -p oct-cli command --help
```

Example:

```bash
cargo run -p oct-cli deploy --help
```

## Testing

### Running tests

Run all Rust unit and integration tests:

```bash
cargo test --workspace
```

Run E2E tests for the platform:

```bash
# 1. Pre-build the platform
cargo build -p oct-platform

# 2. Run E2E tests (requires Deno)
cd crates/oct-platform/e2e
deno task test
```

### Writing tests

Main principles:

- Each module provides its own mocks in a public `mocks` module.
- Each module's tests cover only the functionality in that module.
- If a module uses external modules, mock them using the `mocks` provided by the imported module.
- In tests, prefer `expect("message")` over `unwrap()` to provide failure context.
- When testing specific error messages, use `unwrap_err()` and assert on the error.

Example structure:

```rust
...main code...

pub mod mocks {
    ...mocks...
}

#[cfg(test)]
mod tests {
    ...tests...
}
```

Example mocking:

```rust
...other imports...

#[cfg(test)]
use module::mocks::MockModule as Module;
#[cfg(not(test))]
use module::Module;

...main code...
```

### Imports ordering

Use this order for imports:

- Standard library imports
- Third-party imports
- Local crate imports

```rust
use std::fs;

use serde::{Deserialize, Serialize};

use crate::aws::types::InstanceType;
```

## Developer tools

### Machete

Removes unused dependencies:

```bash
cargo install cargo-machete
cargo machete
```

### Cargo Features Manager

Removes unused features:

```bash
cargo install cargo-features-manager
cargo features prune
```

### Profile build time

Produces an HTML build time report at `target/cargo-timings.html`:

```bash
cargo build -p PACKAGE_NAME --release --timings
```
