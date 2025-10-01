# This FastAPI project configured for deployment with OpenCloudTool.

This project demonstrates how to deploy simple FastAPI service with one endpoint using OpenCloudTool Python SDK.

## How to deploy

### 1. Change directory to this example project.

```bash
cd examples/projects/fastapi-python-sdk
```

### 2. Install dependencies

```bash
uv sync
```

### 3. Run deployment script

```bash
python clouder.py
```

To destroy created infrastructure run this command in current directory

```bash
 cargo run -p oct-cli destroy
```
