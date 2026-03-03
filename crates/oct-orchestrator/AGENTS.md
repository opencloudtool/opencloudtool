# OCT-ORCHESTRATOR Crate Context

This crate (`oct-orchestrator`) is the core orchestration engine that drives
the genesis → apply → destroy lifecycle for cloud infrastructure deployments.

## Architecture

- **Orchestrator** (`lib.rs`):
  - `OrchestratorWithGraph` — main entry point with three async methods:
    - `genesis()` — bootstraps infra: creates state backend, builds spec graph, deploys resources.
    - `apply()` — loads deployed state, connects to leader VM via `oct-ctl-sdk`, forwards config.
    - `destroy()` — tears down infrastructure and removes state.
  - `get_instance_type()` — sums service CPU/memory requirements to pick the smallest EC2 instance.
  - `get_state_backend()` — factory returning a boxed `StateBackend` based on config.

- **State Backends** (`backend.rs`):
  - `StateBackend<T>` — async trait: `save()`, `load()`, `remove()`.
  - `LocalStateBackend<T>` — JSON file on disk.
  - `S3StateBackend<T>` — JSON object in S3.

- **User State** (`user_state.rs`):
  - `UserState` — maps public IPs to `Instance` structs (CPU, memory, services).
  - Used to track what is running on each deployed VM.

## Testing

- **Run tests:**
  ```bash
  cargo test -p oct-orchestrator
  ```
- **Test locations:** inline `#[cfg(test)] mod tests` in `backend.rs`.
- **Patterns:**
  - `tempfile` for local backend tests.
  - S3 backend tests marked `#[ignore]` (require live AWS credentials).
  - `lib.rs` has an empty test module (orchestration tested via integration/E2E).
- **Style:** explicit `// Arrange`, `// Act`, `// Assert` sections.

## Symlinks

- Keep `CLAUDE.md` and `GEMINI.md` in this directory as symlinks to `AGENTS.md`.

## Directory Index

- `AGENTS.md` - Local crate-specific agent instructions.
- `CLAUDE.md` - Symlink to `AGENTS.md`.
- `GEMINI.md` - Symlink to `AGENTS.md`.
- `src/` - Rust source code for `oct-orchestrator`.
  - `lib.rs` - `OrchestratorWithGraph`, instance-type helper, backend factory.
  - `backend.rs` - `StateBackend` trait and Local/S3 implementations.
  - `user_state.rs` - `UserState` and `Instance` data types.
