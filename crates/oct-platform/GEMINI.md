# OCT-PLATFORM Crate Context

This crate (`oct-platform`) provides the web interface for opencloudtool.
It uses **Axum** for the backend, **Askama** for templating, **HTMX** for navigation, and **Alpine.js** for client-side interactivity.
The frontend is styled with **Tailwind CSS** (via CDN in `base.html` for prototyping speed).

## Architecture

- **Interactivity (Alpine.js):**
  - **Theme Management:** Reactive theme switching (Dark/Light) and persistence in `localStorage`.
  - **Log Console:** SSE (Server-Sent Events) streaming with auto-scroll and level-based coloring.
  - **Mermaid.js:** Dynamic rendering of infrastructure graphs with pan-zoom support, reactive to theme changes.
  - **Confirmations:** Intercepting HTMX actions for destructive operations.

- **Routing:**
  - `GET /` -> Redirects to `/projects`
  - `GET /projects` -> Lists projects
  - `POST /projects` -> Creates a project
  - `GET /projects/:name` -> Project details (dashboard)
  - `GET /projects/:name/state` -> Visualization of infrastructure state (Mermaid.js)
  - `GET /projects/:name/edit` -> Edit configuration form
  - `PUT /projects/:name/config` -> Updates configuration
  - `GET /projects/:name/action/:action` -> Streams logs for apply/destroy/genesis

- **Templates:** located in `templates/`
  - `shared/base.html`: Layout, HTMX config, Tailwind config, Theme logic.
  - `pages/`: Individual page templates.
  - `partials/`: Reusable components (e.g. `service_row.html`).

- **Theme System:**
  - **Modes:** Dark (default) and Light. Toggled via sidebar button.
  - **Storage:** Persisted in `localStorage` key `theme`.
  - **Implementation:**
    - CSS Variables defined in `base.html` (`--bg-body`, `--bg-surface`, etc.).
    - Tailwind Config in `base.html` maps these to utility classes (`bg-body`, `bg-surface`, `text-main`).
    - **HTMX Partials:** Use `#[template(path = "...", block = "content")]` structs (e.g. `EditContentTemplate`) for HTMX swaps to avoid re-rendering the sidebar and other layout elements, which prevents visual artifacts like duplicated sidebars.
    - **Default:** Logic respects the system's `prefers-color-scheme` if no preference is saved in `localStorage`.

## Logging

- **Default:** `RUST_LOG=warn,oct_platform=info,oct_cloud=info,oct_orchestrator=info,oct_ctl_sdk=info` (Quiet by default).
- **Verbose:** `RUST_LOG=debug,tower_http=debug,oct_platform=debug,oct_config=info,oct_cloud=debug,oct_orchestrator=debug,oct_ctl_sdk=debug` (Enable with `--verbose` flag).
- **Tests:** `VERBOSE=1` env var enables verbose logging in Playwright tests.

## Testing

- **E2E:** Playwright tests in `e2e/`.
- **Mocking:** `OCT_PLATFORM_MOCK=true` mocks cloud operations (AWS/Terraform) for faster UI testing.
- **Running Tests:**
  ```bash
  cd crates/oct-platform/e2e
  deno task test
  ```
- **Running Unit Tests:**
  ```bash
  cargo test -p oct-platform
  ```

## Development Instructions

1.  **Testing Requirement:** **ALWAYS** run the full E2E test suite (`deno task test` in `e2e/`) after making any changes to the UI (templates, styles, or client-side logic) to ensure no regressions in interactivity or layout.
2.  **Install Bacon (Optional but recommended):**
    ```bash
    cargo install --locked bacon
    ```
3.  **Run with Hot Reload (Bacon):**
    ```bash
    bacon run -p oct-platform
    ```
4.  **Run Manual:**
    ```bash
    cargo run -p oct-platform
    # OR for verbose logs
    cargo run -p oct-platform -- --verbose
    ```
5.  **E2E Development (Deno):**
    ```bash
    cd crates/oct-platform/e2e
    # Run all tests
    deno task test
    # Run linter
    deno task lint
    # Run linter and apply safe fixes
    deno task lint:fix
    # Format code
    deno task format
    # Run all-in-one check (lint + format)
    deno task check
    # Run check and apply all fixes
    deno task check --apply
    ```
6.  **UI Branding:**
    - Font: "Share Tech Mono"
    - Logo: "oct;" + "platform"
    - Colors: Semantic classes (`bg-surface`, `text-main`) to support theming.
