# OCT-PY Crate Context

This crate (`oct-py`) provides Python bindings for OpenCloudTool via PyO3 and maturin.
It exposes deploy/destroy functionality as a native Python package (`opencloudtool`).

## Architecture

- **Rust FFI Layer** (`src/lib.rs`):
  - `deploy(py, path)` — loads config and runs `OrchestratorWithGraph::genesis()` + `apply()`.
  - `destroy(py, path)` — loads config and runs `OrchestratorWithGraph::destroy()`.
  - `init_logging()` — initializes `env_logger` for Rust components.
  - `CWD_LOCK` — `Mutex` preventing concurrent working-directory changes.
  - `DirRestoreGuard` — RAII guard that restores the original directory on drop.
  - GIL is released via `py.detach()` during long-running Rust operations.

- **Python API** (`python/opencloudtool/`):
  - `deploy(path=".")` — deploy project at path.
  - `destroy(path=".")` — destroy infrastructure at path.
  - `deploy_service(path)` — auto-generates Dockerfile, requirements.txt, and `oct.toml`,
    then deploys a FastAPI service.
  - `init_logging()` — initialize Rust logging.
  - All Python functions auto-initialize logging and resolve relative paths.

- **Build System:**
  - `maturin` compiles Rust to a `cdylib` Python extension module (`opencloudtool._internal`).
  - Dev: `maturin develop`
  - Release: `maturin build --release`

## Testing

- **No dedicated test suite.** Verified via example projects.
- **Run any unit tests:**
  ```bash
  cargo test -p oct-py
  ```

## Symlinks

- Keep `CLAUDE.md` and `GEMINI.md` in this directory as symlinks to `AGENTS.md`.

## Directory Index

- `AGENTS.md` - Local crate-specific agent instructions.
- `CLAUDE.md` - Symlink to `AGENTS.md`.
- `GEMINI.md` - Symlink to `AGENTS.md`.
- `src/` - Rust source code for `oct-py`.
  - `lib.rs` - PyO3 bindings, CWD locking, DirRestoreGuard.
- `python/` - Python package source.
  - `opencloudtool/` - Public Python API (`__init__.py`, `py_api.py`).
- `pyproject.toml` - Python/maturin build configuration.
- `.python-version` - Python version specifier.
- `uv.lock` - Python dependency lock file.
- `README.md` - Package documentation.
