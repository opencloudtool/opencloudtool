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


def deploy(path: str = "."):
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


def destroy(path: str = "."):
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
