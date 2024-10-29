[![Actions status](https://github.com/21inchLingcod/opencloudtool/actions/workflows/postsubmit.yml/badge.svg)](https://github.com/21inchLingcod/opencloudtool/actions)

# Open Cloud Tool

A tool to hide the complexity of the cloud

## High Level Design

![OpenCloudTool Design](./docs/high-level-design.png)

## Versions Design

![OpenCloudTool Versions](./docs/versions-design.png)

## TODOs

### [PoC] Deploy simple rest service to the cloud:

- [x] Add example of how to use the tool (simple fastapi app)
- [x] Add `cli` app with `deploy`, `destroy` commands (clap library)
- [x] Add `cloud` app to interact with the cloud (aws for now)
- [x] Deploy the fastapi app to the cloud

### Next steps:

- [ ] Add config file for the cloud deploying services
- [ ] Add support for multiple cloud providers
- [ ] Add support for multiple cloud regions
- [ ] Add UI for monitoring and configuring the cloud
- [ ] Add security for the cloud (connect from specific ip address)

## Usage

### Build project

```bash
 cargo build
```

### Run deploy command

```bash
 cargo run oct-cli deploy --dockerfile-path "your_dockerfile_path" --context-path "your_context_path"
```

### Run destroy command

```bash
 cargo run oct-cli destroy
```

### Show all available commands

```bash
 cargo run oct-cli --help
```

### Show all available parameters for command

```bash
 cargo run oct-cli command --help
```

For example:

```bash
 cargo run oct-cli deploy --help
```
