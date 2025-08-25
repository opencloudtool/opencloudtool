# Deploy as python library

### 1. Navigate to the Python Directory

```bash
cd crates/python
```

### 2. Create and activate the Virtual Enviroment

```bash
uv venv

source .venv/bin/activate
```

### 3. Install dependencies

```bash
uv sync
```

### 4. Build the Library

```bash
maturin develop
```

### 5. Run the example

```bash
cd examples/http-server-with-dockerfile/
```

Deploy

```bash
python deploy.py
```

Destroy

```bash
python destroy.py
```
