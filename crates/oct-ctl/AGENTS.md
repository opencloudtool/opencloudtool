# OCT-CTL Crate Context

This crate (`oct-ctl`) provides the REST API controller that runs on leader VMs
to manage container-based service deployments via Podman.

## Architecture

- **HTTP Server** (`service.rs`):
  - Axum router on port 31888.
  - `POST /apply` — accepts `Config`, builds dependency graph, deploys services in topological order
    (Kahn traversal from `oct-cloud`).
  - `POST /destroy` — cleanup endpoint (currently stubbed).
  - `GET /health-check` — simple liveness probe.
  - `ServerConfig` holds shared `ContainerEngine` via Axum state.

- **Container Engine** (`container.rs`):
  - `ContainerEngine` — wraps Podman CLI for container lifecycle:
    `run()`, `remove()`, `login()`, `pull()`.
  - `ContainerManager` enum defaults to Podman.
  - Uses `CommandExecutor` for shell invocation.

- **Command Executor** (`executor.rs`):
  - `CommandExecutor` — wraps `std::process::Command` with stdout/stderr capture.

- **Mock Pattern:**
  - `container.rs` and `executor.rs` use `mockall::mock!` to define mock types.
  - `#[cfg(test)]` import switching swaps real types for mocks in tests.
  - This two-layer pattern (mock definition + conditional import) is the standard
    approach for this crate.

- **State:** persists deployment state to `/var/log/oct-state.json`.

## Testing

- **Run tests:**
  ```bash
  cargo test -p oct-ctl
  ```
- **Mock utilities:**
  - `get_command_executor_mock()` — fixture factory for `CommandExecutor`.
  - `get_container_engine_mock()` — fixture factory for `ContainerEngine`.
  - Exit status mocking via `ExitStatus::from_raw()`.
- **Test locations:** inline `#[cfg(test)] mod tests` in `service.rs`, `container.rs`, `executor.rs`.
- **Server tests:** use `tower::Service::oneshot()` for Axum endpoint testing.
- **Style:** explicit `// Arrange`, `// Act`, `// Assert` sections.

## Symlinks

- Keep `CLAUDE.md` and `GEMINI.md` in this directory as symlinks to `AGENTS.md`.

## Directory Index

- `AGENTS.md` - Local crate-specific agent instructions.
- `CLAUDE.md` - Symlink to `AGENTS.md`.
- `GEMINI.md` - Symlink to `AGENTS.md`.
- `src/` - Rust source code for `oct-ctl`.
  - `main.rs` - Entry point; calls `service::run()`.
  - `service.rs` - Axum router, endpoints, and `ServerConfig`.
  - `container.rs` - `ContainerEngine` Podman wrapper and mock.
  - `executor.rs` - `CommandExecutor` shell wrapper and mock.
