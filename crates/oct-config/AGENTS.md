# OCT-CONFIG Crate Context

This crate (`oct-config`) handles TOML configuration loading, Tera template rendering
for environment variables, and conversion of service definitions into dependency graphs.

## Architecture

- **Config Loading:**
  - `Config::new(path)` reads an `oct.toml` file and deserializes it via `toml`.
  - `render_system_envs()` substitutes `{{ env.* }}` placeholders using Tera before parsing.

- **Key Types:**
  - `Config` — root struct wrapping a `Project`.
  - `Project` — `name`, `domain`, `services`, `state_backend`, `user_state_backend`.
  - `Service` — `name`, `image`, `cpus`, `memory`, `depends_on`, `envs`, optional ports/dockerfile/command.
  - `StateBackend` — enum: `Local { path }` or `S3 { region, bucket, key }`.
  - `Node` — graph node enum: `Root` (synthetic) or `Resource(Service)`.

- **Graph Conversion:**
  - `Config::to_graph()` builds a `petgraph::Graph<Node, String>` DAG.
  - Adds a synthetic `Root` node connected to all services.
  - Validates: no duplicate service names, no missing dependency references.

- **Single-file crate:** all code lives in `src/lib.rs`.

## Testing

- **Run tests:**
  ```bash
  cargo test -p oct-config
  ```
- **Pattern:** `tempfile::NamedTempFile` for writing temporary TOML configs.
- **Coverage:** success path with env var injection, empty/single/multi-service graphs,
  missing dependency errors, duplicate service name errors.
- **Style:** explicit `// Arrange`, `// Act`, `// Assert` sections.

## Symlinks

- Keep `CLAUDE.md` and `GEMINI.md` in this directory as symlinks to `AGENTS.md`.

## Directory Index

- `AGENTS.md` - Local crate-specific agent instructions.
- `CLAUDE.md` - Symlink to `AGENTS.md`.
- `GEMINI.md` - Symlink to `AGENTS.md`.
- `src/` - Rust source code for `oct-config`.
  - `lib.rs` - All types, config loading, graph conversion, and tests.
