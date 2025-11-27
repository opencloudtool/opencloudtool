# `opencloudtool` Platform

## How to use

```bash
export GITHUB_CLIENT_ID=<CLIENT_ID>
export GITHUB_CLIENT_SECRET=<CLIENT_SECRET>

cargo run -p oct-platform
```

## SSR UI framework

### Templates folders structure

```
/pages - pages definitions that have routes
/pages/index.html - root (`/`) page

/shared - shared base pages (e.g. `base.html` with base html page structure)

/components - idependent components
```

### Template/handler specs

- Page:
  - Template struct
  - Render handler
- Component:
  - Template struct
  - Render handler
- Form
  - Template struct
  - Render handler
  - Request struct
  - Request handler (JSON)
  - Response (?)
