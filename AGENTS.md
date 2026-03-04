# OpenCloudTool

Workspace-level instructions for AI agents operating in this repository.

## MANDATORY

> Stop and read this section before making changes.

- **Document Code:** Add or refresh doc comments/docstrings for added or changed behavior.
- **Update `AGENTS.md` Carefully:** Only update `AGENTS.md` when a user instruction establishes a persistent convention.
- **Directory Index:** Keep the `Directory Index` section in sync when files or folders are added, removed, or renamed.
- **Local Paths Only:** In `AGENTS.md`, document only files/folders in the same directory; do not use parent-relative paths.
- **Context First:** Before broad exploration in a folder, read its local `AGENTS.md` (if present).
- **Symlink Policy:** When creating an `AGENTS.md` in a directory, define `CLAUDE.md` and `GEMINI.md` as symlinks to `AGENTS.md`.

## Project Facts

- This repository is a Rust workspace (`members = ["crates/*"]`).
- Workspace crates use the `oct-` prefix (`oct-cloud`, `oct-config`, `oct-ctl-sdk`, `oct-orchestrator`, `oct-platform`, etc.).
- `oct-platform` is the web UI crate (Axum + Askama + HTMX + Alpine.js).
- Root dependency versions are managed in `[workspace.dependencies]`.

## Development Instructions

- Keep context lean: prefer targeted reads (`rg`, line ranges) over dumping full files.
- Prefer pragmatic edits with minimal diff unless a larger refactor is clearly justified.
- Keep `README.md` aligned with user-facing behavior when features/usage change.

## Testing Strategy

### Rust Tests

- Run workspace tests:

```bash
cargo test --workspace
```

### `oct-platform` E2E Tests

```bash
cargo build -p oct-platform
cd crates/oct-platform/e2e
deno task test
```

## Rust Style Guide

### `Cargo.toml`

- Group dependencies by empty lines in this order:
  1. Internal `oct-*` crates
  2. Third-party crates
- Keep dependencies alphabetically sorted inside each group.

### Imports

- Group and order imports as:
  1. `std`
  2. External crates
  3. Internal `oct-*` crates
  4. `crate` local modules

### Tests

- Always check for existing tests in touched modules.
- Add tests for new/changed logic when practical.
- Use explicit `// Arrange`, `// Act`, `// Assert` sections.
- Prefer `expect("...")` over `unwrap()` for clearer failures.
- Use `unwrap_err()` and assert on error details when validating failures.

## Quality Checks

Run these checks before finishing significant code changes:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features --no-deps
cargo test --workspace
```

## Directory Index

- `.agentty/` - Agentty runtime/session metadata for local worktree sessions.
- `.github/` - GitHub workflow and configuration files.
- `AGENTS.md` - Workspace instructions for AI agents.
- `CLAUDE.md` - Symlink to `AGENTS.md`.
- `GEMINI.md` - Symlink to `AGENTS.md`.
- `Cargo.toml` - Workspace manifest and shared dependencies.
- `Cargo.lock` - Locked dependency graph.
- `crates/` - Workspace member crates.
- `docs/` - Project documentation.
- `examples/` - Example assets and scenarios.
- `scripts/` - Utility scripts used by contributors/CI.
- `skills/` - Repository-specific reusable agent skills.
- `README.md` - Main project overview and usage docs.
