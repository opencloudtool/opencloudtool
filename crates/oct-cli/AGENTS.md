# OCT-CLI Crate Context

This crate (`oct-cli`) is the command-line entry point for OpenCloudTool. It is a
thin Clap wrapper that parses arguments and delegates to `oct-orchestrator`.

## Architecture

- **Commands** (Clap derive):
  - `Genesis` — initialize application infrastructure.
  - `Apply` — deploy/apply configuration changes.
  - `Destroy` — tear down infrastructure.

- **Global Options:**
  - `--user-state-file-path` (default `./user_state.json`)
  - `--dockerfile-path` (default `.`)
  - `--context-path` (default `.`)

- **Flow:** parse args → `Config::new()` → `OrchestratorWithGraph` → call matching command method.

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
