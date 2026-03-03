# OCT-CTL-SDK Crate Context

This crate (`oct-ctl-sdk`) provides an HTTP client for the `oct-ctl` REST API
running on leader VMs. It handles health-check gating with retries before
forwarding apply/destroy requests.

## Architecture

- **Client:**
  - `Client::new(public_ip)` — constructor, default port 31888.
  - `client.apply(config)` — serializes `Config` into `ApplyRequest`, POSTs to `/apply`.
  - `client.destroy()` — POSTs to `/destroy`.
  - Both methods call `check_host_health()` first (24 retries × 5 s = 120 s max wait).

- **Health Check:**
  - `health_check()` — single GET `/health-check` with 5 s timeout.
  - `check_host_health()` — retry loop around `health_check()`.

- **Single-file crate:** all code lives in `src/lib.rs`.

## Testing

- **Run tests:**
  ```bash
  cargo test -p oct-ctl-sdk
  ```
- **Mock pattern:** `mockito` async HTTP server; helper `setup_server()` creates
  isolated mock with random port/IP.
- **Assertions:** verify HTTP status codes and request headers (Content-Type, Accept).
- **Style:** explicit `// Arrange`, `// Act`, `// Assert` sections.

## Symlinks

- Keep `CLAUDE.md` and `GEMINI.md` in this directory as symlinks to `AGENTS.md`.

## Directory Index

- `AGENTS.md` - Local crate-specific agent instructions.
- `CLAUDE.md` - Symlink to `AGENTS.md`.
- `GEMINI.md` - Symlink to `AGENTS.md`.
- `src/` - Rust source code for `oct-ctl-sdk`.
  - `lib.rs` - Client, health-check retry logic, and tests.
