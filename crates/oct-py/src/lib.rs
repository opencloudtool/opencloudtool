use pyo3::prelude::*;
use std::env;

use oct_orchestrator::OrchestratorWithGraph;
use std::path::PathBuf;
use std::sync::OnceLock;
use tokio::sync::Mutex;

static CWD_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct DirRestoreGuard {
    prev: PathBuf,
}

impl Drop for DirRestoreGuard {
    fn drop(&mut self) {
        if let Err(e) = env::set_current_dir(&self.prev) {
            log::error!(
                "opencloudtool._internal: failed to restore current directory to '{}': {e}",
                self.prev.display(),
            );
        }
    }
}

#[pyfunction]
fn init_logging() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .try_init();
}

#[pyfunction]
fn deploy(py: Python, path: String) -> PyResult<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                "Failed to create Tokio runtime: {e}",
            ))
        })?;

    py.detach(|| {
        rt.block_on(async move {
            // Lock to ensure only this thread can change the CWD.
            let _cwd_lock = CWD_LOCK.get_or_init(|| Mutex::new(())).lock().await;
            // Save the original directory.
            let prev_cwd = env::current_dir().map_err(|e| {
                pyo3::exceptions::PyIOError::new_err(format!(
                    "Failed to read current directory: {e}"
                ))
            })?;
            // Change to the target directory.
            env::set_current_dir(&path).map_err(|e| {
                pyo3::exceptions::PyIOError::new_err(format!(
                    "Failed to change directory to '{path}': {e}"
                ))
            })?;
            // The guard is now active. When the function ends, it will automatically
            // change the directory back to `prev_cwd`.
            let _cwd_restore = DirRestoreGuard { prev: prev_cwd };

            let orchestrator = OrchestratorWithGraph;
            let deploy_result = orchestrator.deploy().await;

            match deploy_result {
                Ok(()) => Ok(()),
                Err(e) => Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                    "Deployment failed in Rust core: {e}"
                ))),
            }
        })
    })
}

#[pyfunction]
fn destroy(py: Python, path: String) -> PyResult<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                "Failed to create Tokio runtime: {e}",
            ))
        })?;

    py.detach(|| {
        rt.block_on(async move {
            let _cwd_lock = CWD_LOCK.get_or_init(|| Mutex::new(())).lock().await;

            let prev_cwd = env::current_dir().map_err(|e| {
                pyo3::exceptions::PyIOError::new_err(format!(
                    "Failed to read current directory: {e}"
                ))
            })?;
            env::set_current_dir(&path).map_err(|e| {
                pyo3::exceptions::PyIOError::new_err(format!(
                    "Failed to change directory to '{path}': {e}"
                ))
            })?;
            let _cwd_restore = DirRestoreGuard { prev: prev_cwd };

            let orchestrator = OrchestratorWithGraph;
            let destroy_result = orchestrator.destroy().await;

            match destroy_result {
                Ok(()) => Ok(()),
                Err(e) => Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                    "Destruction failed in Rust core: {e}"
                ))),
            }
        })
    })
}

/// This function defines the `opencloudtool` Python module.
#[pymodule]
fn _internal(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(deploy, m)?)?;
    m.add_function(wrap_pyfunction!(destroy, m)?)?;
    m.add_function(wrap_pyfunction!(init_logging, m)?)?;

    Ok(())
}
