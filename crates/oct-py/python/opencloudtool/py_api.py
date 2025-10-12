import os

import logging
from ._internal import deploy as _rust_deploy
from ._internal import destroy as _rust_destroy
from ._internal import init_logging as _rust_init_logging

# A flag to ensure we only initialize the logger once per session.
_logging_initialized = False

logger = logging.getLogger(name=__name__)


def init_logging():
    """
    Initializes the Rust logging system to show logs in the console.

    This is called automatically by other functions like `deploy`,
    so you don't typically need to call it yourself.
    """
    global _logging_initialized
    if not _logging_initialized:
        _rust_init_logging()
        _logging_initialized = True


def deploy(path: str = ".") -> None:
    """
    Deploys the application using the Rust core orchestrator.

    Args:
        path (str): The path to the project directory containing the
                    `oct.toml` file. Defaults to the current directory.
    """

    init_logging()
    project_path = os.path.abspath(path)
    logger.info("[Python] Triggering deployment")

    try:
        _rust_deploy(project_path)
        logger.info("[Python] Deployment call completed successfully.")
    except (RuntimeError, IOError) as e:
        logger.exception(f"[Python] An error occurred during deployment: {e}")
        raise


def destroy(path: str = ".") -> None:
    """
    Destroys the application using the Rust core orchestrator.

    Args:
        path (str): The path to the project directory containing the
                    `oct.toml` file. Defaults to the current directory.
    """

    init_logging()
    project_path = os.path.abspath(path)
    logger.info("[Python] Triggering destroy")

    try:
        _rust_destroy(project_path)
        logger.info("[Python] Destroy call completed successfully.")
    except (RuntimeError, IOError) as e:
        logger.exception(f"[Python] An error occurred during destroy process: {e}")
        raise


def deploy_service(path: str) -> None:
    """
    Automates the deployment of a standard FastAPI service.

    This high-level function generates the necessary Dockerfile, requirements.txt,
    and oct.toml, then calls the core deploy function.

    Args:
        path: The path to the FastAPI application's main.py file.
    """
    if not os.path.isfile(path) or not path.endswith("main.py"):
        raise ValueError("The path must point to a 'main.py' file.")

    absolute_path = os.path.abspath(path)
    app_dir = os.path.dirname(absolute_path)
    project_root = os.path.dirname(app_dir)
    project_name = os.path.basename(project_root)
    logger.info(f"Detected project root: '{project_root}' with name '{project_name}'")

    dockerfile_content = """\
    FROM python:3.9-slim
    WORKDIR /code
    COPY ./requirements.txt /code/requirements.txt
    RUN pip install --no-cache-dir --upgrade -r /code/requirements.txt
    COPY ./app /code/app
    CMD ["uvicorn", "app.main:app", "--host", "0.0.0.0", "--port", "80"]
    """

    requirements_content = """\
    fastapi
    uvicorn[standard]
    """

    oct_toml_content = f"""\
    [project]
    name = "{project_name}"

    [project.state_backend.local]
    path = "./state.json"

    [project.user_state_backend.local]
    path = "./user_state.json"

    [project.services.app_1]
    image = ""
    dockerfile_path = "Dockerfile"
    internal_port = 80
    external_port = 80
    cpus = 250
    memory = 64
    """

    try:
        dockerfile_path = os.path.join(project_root, "Dockerfile")
        with open(dockerfile_path, "w") as f:
            f.write(dockerfile_content)
        logger.info(f"Generated Dockerfile at '{dockerfile_path}'")

        reqs_path = os.path.join(project_root, "requirements.txt")
        with open(reqs_path, "w") as f:
            f.write(requirements_content)
        logger.info(f"Generated requirements.txt at '{reqs_path}'")

        toml_path = os.path.join(project_root, "oct.toml")
        with open(toml_path, "w") as f:
            f.write(oct_toml_content)
        logger.info(f"Generated oct.toml at '{toml_path}'")

    except IOError:
        logger.exception("Error writing configuration files")
        raise

    deploy(project_root)
