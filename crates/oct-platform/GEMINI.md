# OCT-PLATFORM Crate Context

This crate (`oct-platform`) provides the web interface for opencloudtool.
It uses **Axum** for the backend, **Askama** for templating, and **HTMX** for interactivity.
The frontend is styled with **Tailwind CSS** (via CDN in `base.html` for prototyping speed).

## Architecture

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
    - **Default:** Logic forces Dark mode if no preference is saved, overriding system preference to maintain brand identity.

## Logging

- **Default:** `RUST_LOG=warn,oct_platform=info` (Quiet by default).
- **Verbose:** `RUST_LOG=debug,tower_http=debug,oct_platform=debug,oct_config=info` (Enable with `--verbose` flag).
- **Tests:** `VERBOSE=1` env var enables verbose logging in Playwright tests.

## Testing

- **E2E:** Playwright tests in `e2e/`.
- **Mocking:** `OCT_PLATFORM_MOCK=true` mocks cloud operations (AWS/Terraform) for faster UI testing.
- **Running Tests:**
  ```bash
  cd crates/oct-platform/e2e
  npx playwright test
  ```
- **Running Unit Tests:**
  ```bash
  cargo test -p oct-platform
  ```

## Development Instructions

1.  **Run with Hot Reload (Bacon):**
    ```bash
    bacon run -p oct-platform
    ```
2.  **Run Manual:**
    ```bash
    cargo run -p oct-platform
    # OR for verbose logs
    cargo run -p oct-platform -- --verbose
    ```
3.  **UI Branding:**
    - Font: "Share Tech Mono"
    - Logo: "oct;" + "platform"
    - Colors: Semantic classes (`bg-surface`, `text-main`) to support theming.
