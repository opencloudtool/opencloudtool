# Opencloudtool Python Library

A tool to hide the complexity of the cloud, now available as a Python library.
This library allows you to deploy services directly from your Python scripts.The core of this library is written in Rust
for high performance and reliability.

## Installation

You can install the library from PyPI using `pip`

```bash
pip install opencloudtool

```

## Basic Usage

To use the library, you need an `oct.toml` configuration file in your project directory.
The library provides `deploy` and `destroy` functions to manage your stack.

Example `deploy.py`

```python
import opencloudtool as oct

# The path to the project directory containing oct.toml
project_path = "./my-app"

oct.deploy(path=project_path)
```

To destroy infrastructure:

```python
oct.destroy(path=project_path)
```

Main repo [opencloudtool](https://github.com/opencloudtool/opencloudtool)

### Dev

#### How it works: Python-Rust binding

#### The connection between Python and Rust is managed by `maturin` and `PyO3`

1. `maturin` compiles the Rust code in the `oct-py` crate into a native Python module.
2. We configure the name of this compiled module in `pyproject.toml` to be `opencloudtool._internal`.
3. The leading underscore (`_`) is a standard Python convention that signals that `_internal` is a low-level module not meant for direct use.
4. Our user-facing Python code in `opencloudtool/py_api.py` imports functions from `_internal` and presents them as a clean, stable API.

#### 1. Navigate to the Python Directory

```bash
cd crates/oct-py
```

#### 2. Create and activate the Virtual environment

```bash
uv venv

source .venv/bin/activate # Windows: .venv\Scripts\activate
```

#### 3. Install dependencies

```bash
uv sync --group dev
```

#### 4. Build the Library

```bash
maturin develop
```

#### 5. Run the example

```bash
cd ../../examples/projects/single-host-python-lib
```

You can now run `python deploy.py` or `python destroy.py` to test your changes.

### Releasing the Python Library

#### Bump the Version Number

1. Before releasing, you must increment the version number. PyPI will not accept a version that already exists.

- `crates/oct-py/pyproject.toml`

2. Build & upload the Release Packages:

```bash
cd crates/oct-py

maturin sdist

maturin build --release

cd ../..

twine upload target/wheels/*
```
