use pyo3::{exceptions::PyValueError, prelude::*};
use rust::NativeRuntime;
use std::sync::Arc;
#[pyclass]
struct Native {
    inner: Arc<NativeRuntime>,
}
#[pymethods]
impl Native {
    #[new]
    fn new(py: Python<'_>, configuration: String) -> PyResult<Self> {
        py.detach(|| NativeRuntime::create(&configuration))
            .map(|inner| Self {
                inner: Arc::new(inner),
            })
            .map_err(PyValueError::new_err)
    }
    fn handle(&self, py: Python<'_>, request: String) -> PyResult<String> {
        py.detach(|| self.inner.handle(&request))
            .map_err(PyValueError::new_err)
    }
    fn authorize(&self, py: Python<'_>, input: String) -> PyResult<String> {
        py.detach(|| self.inner.authorize(&input))
            .map_err(PyValueError::new_err)
    }
}
#[pymodule]
fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<Native>()?;
    Ok(())
}
