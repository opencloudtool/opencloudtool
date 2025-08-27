# Deploy as Python library

This guide explains how to set up the environment and use the `oct` Python library.

## How it works: Python-Rust binding

### The connection between Python and Rust is managed by `maturin` and `PyO3`

1. `maturin` compiles the Rust code in the `oct-py` crate into a native Python module.
2. We configure the name of this compiled module in `pyproject.toml` to be `oct._internal`.
3. The leading underscore (`_`) is a standard Python convention that signals that `_internal` is a low-level module not meant for direct use.
4. Our user-facing Python code in `oct/py_api.py` imports functions from `_internal` and presents them as a clean, stable API.

### 1. Navigate to the Python Directory

```bash
cd crates/oct-py
```

### 2. Create and activate the Virtual environment

```bash
uv venv

source .venv/bin/activate # Windows: .venv\Scripts\activate
```

### 3. Install dependencies

```bash
uv sync --group dev
```

### 4. Build the Library

```bash
maturin develop
```

### 5. Run the example

```bash
cd ../../examples/projects/single-host-python-lib
```

### Deploy

```bash
python deploy.py
```

### Destroy

```bash
python destroy.py
```
