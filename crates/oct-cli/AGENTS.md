# OCT-CLI Crate Context

This crate (`oct-cli`) is the command-line entry point for OpenCloudTool. It is a
thin Clap wrapper that parses arguments and delegates to `oct-orchestrator`.

## Architecture

- **Commands** (Clap derive):
  - `Genesis` — initialize application infrastructure.
  - `Apply` — deploy/apply configuration changes.
  - `Destroy` — tear down infrastructure. Accepts optional `--state-path` to skip `oct.toml`.
  - `Run` — inline single-container deploy (genesis + apply in one step). Constructs
    config from CLI flags (`--image`, `--name`, `--cpus`, `--memory`, `--port`, `-e`/`--env`,
    `--state-path`) via `build_inline_config()`.

- **Global Options:**
  - `--user-state-file-path` (default `./user_state.json`)
  - `--dockerfile-path` (default `.`)
  - `--context-path` (default `.`)

- **Helpers:**
  - `build_inline_config()` — constructs `oct_config::Config` from CLI args for the `Run` command.
  - `build_destroy_config()` — constructs a minimal `Config` with local state backend for `Destroy --state-path`.

- **Flow:** parse args → `Config::new()` or inline config builder → `OrchestratorWithGraph` → call matching command method.

## Testing

- **Run tests:**
  ```bash
  cargo test -p oct-cli
  ```
- **Unit tests** (`src/main.rs`): verify Clap parsing with `Cli::parse_from()`.
- **Integration tests** (`tests/cli.rs`): use `assert_cmd` + `predicates` to run the
  compiled binary and assert on stderr output (e.g., missing `oct.toml` error).
- **Style:** explicit `// Arrange`, `// Assert` sections.

## Symlinks

- Keep `CLAUDE.md` and `GEMINI.md` in this directory as symlinks to `AGENTS.md`.

## Directory Index

- `AGENTS.md` - Local crate-specific agent instructions.
- `CLAUDE.md` - Symlink to `AGENTS.md`.
- `GEMINI.md` - Symlink to `AGENTS.md`.
- `src/` - Rust source code for `oct-cli`.
  - `main.rs` - Clap CLI definition and async main entry point.
- `tests/` - Integration tests.
  - `cli.rs` - Binary-level tests via `assert_cmd`.
