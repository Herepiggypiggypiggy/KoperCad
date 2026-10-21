//! The `kopercad` Python module: a thin shim over [`ScriptProject`].
//!
//! Ids are plain ints, queries return dicts and lists (converted through
//! JSON), and every error is a `kopercad.KoperError`.

use std::path::PathBuf;

use novo_core::{DeviceId, PageId, PartId, PotentialId, SymbolId, TerminalId, WireId};
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::PyString;
use serde_json::Value;

use crate::{Role, ScriptError, ScriptProject};

pyo3::create_exception!(
    kopercad,
    KoperError,
    PyException,
    "A KoperCAD call was refused; the project is unchanged."
);

fn err(e: ScriptError) -> PyErr {
    KoperError::new_err(e.to_string())
}

fn closed() -> PyErr {
    KoperError::new_err("the project is closed")
}

fn to_py<'py>(py: Python<'py>, value: &Value) -> PyResult<Bound<'py, PyAny>> {
    py.import("json")?
        .call_method1("loads", (value.to_string(),))
}

/// An open KoperCAD project. Every edit is one command and one undo
/// step, exactly as in the app.
///
/// ``Project(name)`` starts a new unsaved project; ``Project.create(path)``
/// and ``Project.open(path)`` work on a file, which stays locked against the
/// app until ``close()`` (or the end of a ``with`` block).
#[pyclass(name = "Project", module = "kopercad", unsendable)]
struct PyProject {
    inner: Option<ScriptProject>,
}

impl PyProject {
    fn get(&self) -> PyResult<&ScriptProject> {
        self.inner.as_ref().ok_or_else(closed)
    }

    fn get_mut(&mut self) -> PyResult<&mut ScriptProject> {
        self.inner.as_mut().ok_or_else(closed)
    }
}

#[pymethods]
impl PyProject {
    #[new]
    #[pyo3(signature = (name = "Untitled"))]
    fn new(name: &str) -> Self {
        PyProject {
            inner: Some(ScriptProject::new(name)),
        }
    }

    /// Creates a new project file at ``path``. An existing file is refused
    /// unless ``overwrite=True``; a file with unrecovered work from a
    /// crashed app session is always refused.
    #[staticmethod]
    #[pyo3(signature = (path, name = "Untitled", *, overwrite = false))]
    fn create(path: PathBuf, name: &str, overwrite: bool) -> PyResult<Self> {
        Ok(PyProject {
            inner: Some(ScriptProject::create(path, name, overwrite).map_err(err)?),
        })
    }

    /// Opens a project file for editing.
    #[staticmethod]
    fn open(path: PathBuf) -> PyResult<Self> {
        Ok(PyProject {
            inner: Some(ScriptProject::open(path).map_err(err)?),
        })
    }

    /// The project file, or None before the first ``save_as``.
    #[getter]
    fn path(&self) -> PyResult<Option<PathBuf>> {
        Ok(self.get()?.path().map(PathBuf::from))
    }

    /// Whether there are changes since the last save.
    #[getter]
    fn dirty(&self) -> PyResult<bool> {
        Ok(self.get()?.is_dirty())
    }

    fn save(&mut self) -> PyResult<()> {
        self.get_mut()?.save().map_err(err)
    }

    /// Saves to ``path`` and continues there. Another existing file is
    /// refused unless ``overwrite=True``; a file with unrecovered work from a
    /// crashed app session is always refused.
    #[pyo3(signature = (path, *, overwrite = false))]
    fn save_as(&mut self, path: PathBuf, overwrite: bool) -> PyResult<()> {
        self.get_mut()?.save_as(path, overwrite).map_err(err)
    }

    /// Releases the file. Unsaved changes are dropped; ``save()`` first.
    fn close(&mut self) -> PyResult<()> {
        match self.inner.take() {
            Some(project) => project.close().map_err(err),
            None => Ok(()),
        }
    }

    fn __enter__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    #[pyo3(signature = (*_args))]
    fn __exit__(&mut self, _args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<bool> {
        self.close()?;
        Ok(false)
    }

    // ----- commands -------------------------------------------------------

    /// Runs one command in its JSON form, as a dict or a JSON string, e.g.
    /// ``{"command": "add_page", "name": "3"}``. Returns the created ids as
    /// ``[{"kind": "page", "id": 7}]``.
    fn execute<'py>(
        &mut self,
        py: Python<'py>,
        command: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let text: String = if command.is_instance_of::<PyString>() {
            command.extract()?
        } else {
            py.import("json")?
                .call_method1("dumps", (command,))?
                .extract()?
        };
        let created = self.get_mut()?.execute_json(&text).map_err(err)?;
        to_py(py, &serde_json::to_value(created).expect("ids serialize"))
    }

    #[pyo3(signature = (name, title = ""))]
    fn add_page(&mut self, name: &str, title: &str) -> PyResult<u64> {
        Ok(self.get_mut()?.add_page(name, title).map_err(err)?.raw())
    }

    /// Adds a device; ``kind`` is general, terminal_strip, connector or plc.
    #[pyo3(signature = (tag, description = "", kind = "general"))]
    fn add_device(&mut self, tag: &str, description: &str, kind: &str) -> PyResult<u64> {
        Ok(self
            .get_mut()?
            .add_device(tag, description, kind)
            .map_err(err)?
            .raw())
    }

    fn add_terminal(&mut self, strip: u64, name: &str) -> PyResult<u64> {
        Ok(self
            .get_mut()?
            .add_terminal(DeviceId(strip), name)
            .map_err(err)?
            .raw())
    }

    /// Adds a potential; ``kind`` is line, neutral, protective_earth, pen,
    /// dc_positive, dc_negative or signal.
    #[pyo3(signature = (name, kind = "signal", voltage = None))]
    fn add_potential(&mut self, name: &str, kind: &str, voltage: Option<f64>) -> PyResult<u64> {
        Ok(self
            .get_mut()?
            .add_potential(name, kind, voltage)
            .map_err(err)?
            .raw())
    }

    /// Adds a part (an article): ``add_part("Siemens", "3RT2015-1BB41")``.
    #[pyo3(signature = (manufacturer, order_number, type_number = "", description = ""))]
    fn add_part(
        &mut self,
        manufacturer: &str,
        order_number: &str,
        type_number: &str,
        description: &str,
    ) -> PyResult<u64> {
        Ok(self
            .get_mut()?
            .add_part(manufacturer, order_number, type_number, description)
            .map_err(err)?
            .raw())
    }

    /// Adds a part to a device's part list.
    fn assign_part(&mut self, device: u64, part: u64) -> PyResult<()> {
        self.get_mut()?
            .assign_part(DeviceId(device), PartId(part))
            .map_err(err)
    }

    /// Renames a device: ``set_tag(k1, "-K2")``.
    fn set_tag(&mut self, device: u64, tag: &str) -> PyResult<()> {
        self.get_mut()?.set_tag(DeviceId(device), tag).map_err(err)
    }

    /// Places a library symbol by id (``iec/contacts/no``) or alias (``no``)
    /// at ``x``, ``y`` mm. Link it with one of ``device`` (and ``main``),
    /// ``terminal``, ``potential`` or ``interruption``.
    #[pyo3(signature = (
        page, symbol, x, y, *, rotation = 0, mirrored = false,
        device = None, main = false, terminal = None, potential = None, interruption = None
    ))]
    #[allow(clippy::too_many_arguments)]
    fn place(
        &mut self,
        page: u64,
        symbol: &str,
        x: f64,
        y: f64,
        rotation: u16,
        mirrored: bool,
        device: Option<u64>,
        main: bool,
        terminal: Option<u64>,
        potential: Option<u64>,
        interruption: Option<String>,
    ) -> PyResult<u64> {
        let role = match (device, terminal, potential, interruption) {
            (None, None, None, None) => Role::Unassigned,
            (Some(d), None, None, None) => Role::Device {
                device: DeviceId(d),
                main,
            },
            (None, Some(t), None, None) => Role::Terminal(TerminalId(t)),
            (None, None, Some(p), None) => Role::Potential(PotentialId(p)),
            (None, None, None, Some(name)) => Role::Interruption(name),
            _ => {
                return Err(KoperError::new_err(
                    "give at most one of device, terminal, potential and interruption",
                ));
            }
        };
        Ok(self
            .get_mut()?
            .place(PageId(page), symbol, x, y, rotation, mirrored, role)
            .map_err(err)?
            .raw())
    }

    /// Draws a wire between two pins: ``connect((f1, "2"), (k1, "1"))``.
    fn connect(&mut self, a: (u64, String), b: (u64, String)) -> PyResult<u64> {
        Ok(self
            .get_mut()?
            .connect((SymbolId(a.0), &a.1), (SymbolId(b.0), &b.1))
            .map_err(err)?
            .raw())
    }

    /// Assigns potentials to wires in order: ``assign_potentials([w1, w2,
    /// w3], ["L1", "L2", "L3"])``. Missing potentials are created.
    fn assign_potentials(&mut self, wires: Vec<u64>, names: Vec<String>) -> PyResult<()> {
        let wires: Vec<WireId> = wires.into_iter().map(WireId).collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        self.get_mut()?
            .assign_potentials(&wires, &names)
            .map_err(err)
    }

    /// Undoes the last step; False when there was nothing to undo.
    fn undo(&mut self) -> PyResult<bool> {
        self.get_mut()?.undo().map_err(err)
    }

    fn redo(&mut self) -> PyResult<bool> {
        self.get_mut()?.redo().map_err(err)
    }

    #[getter]
    fn can_undo(&self) -> PyResult<bool> {
        Ok(self.get()?.can_undo())
    }

    #[getter]
    fn can_redo(&self) -> PyResult<bool> {
        Ok(self.get()?.can_redo())
    }

    #[getter]
    fn undo_label(&self) -> PyResult<Option<String>> {
        Ok(self.get()?.undo_label().map(String::from))
    }

    // ----- queries --------------------------------------------------------

    /// The id of the device with this tag, or None.
    fn device(&self, tag: &str) -> PyResult<Option<u64>> {
        Ok(self
            .get()?
            .device_by_tag(tag)
            .map_err(err)?
            .map(|d| d.raw()))
    }

    fn info<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        to_py(py, &self.get()?.info())
    }

    /// The issued revisions: index, date, author, description, and
    /// ``snapshot`` (a kept copy exists to compare with).
    fn revisions<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        to_py(py, &self.get()?.revisions())
    }

    /// What changed between two states; each is a revision index such as
    /// ``"B"`` or ``"now"``. Gives ``rows`` (group, kind, label, page and
    /// the fields with old and new values), ``info`` and ``sheets``.
    /// Read only.
    #[pyo3(signature = (from_, to = "now"))]
    fn compare<'py>(&self, py: Python<'py>, from_: &str, to: &str) -> PyResult<Bound<'py, PyAny>> {
        to_py(py, &self.get()?.compare(from_, to).map_err(err)?)
    }

    fn pages<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        to_py(py, &self.get()?.pages())
    }

    fn devices<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        to_py(py, &self.get()?.devices())
    }

    fn symbols<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        to_py(py, &self.get()?.symbols())
    }

    fn wires<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        to_py(py, &self.get()?.wires())
    }

    fn potentials<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        to_py(py, &self.get()?.potentials())
    }

    /// The open design-check problems as dicts (severity, kind, label,
    /// message, page, at): what ``kopercad-cli check --json`` prints.
    fn problems<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        to_py(py, &self.get()?.problems())
    }

    fn terminals<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        to_py(py, &self.get()?.terminals())
    }

    /// The netlist: one dict per net with its pins (``-K1:1``), junction
    /// count, wire ids and potential names.
    fn nets<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        to_py(py, &self.get()?.nets())
    }

    /// A report as data: ``report("bom")`` gives its sections with column
    /// headers and rows of text. Reports: toc, bom, devices, wires.
    fn report<'py>(&self, py: Python<'py>, id: &str) -> PyResult<Bound<'py, PyAny>> {
        to_py(py, &self.get()?.report(id).map_err(err)?)
    }

    /// Writes the bill of materials to ``path`` (``.xlsx`` or ``.csv``).
    fn export_bom(&self, path: PathBuf) -> PyResult<()> {
        self.get()?.export_reports(&["bom"], path).map_err(err)
    }

    /// Writes reports to ``path``: ``.xlsx`` (one sheet each; every report
    /// when ``reports`` is None) or ``.csv`` (exactly one report).
    #[pyo3(signature = (path, reports = None))]
    fn export_reports(&self, path: PathBuf, reports: Option<Vec<String>>) -> PyResult<()> {
        let ids = reports.unwrap_or_default();
        let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
        self.get()?.export_reports(&ids, path).map_err(err)
    }

    /// The whole project as its diffable JSON text.
    fn to_json(&self) -> PyResult<String> {
        Ok(self.get()?.to_json())
    }

    fn __repr__(&self) -> String {
        match &self.inner {
            Some(p) => match p.path() {
                Some(path) => format!(
                    "<kopercad.Project {:?} at {:?}>",
                    p.project().info().name,
                    path.display().to_string()
                ),
                None => format!("<kopercad.Project {:?}>", p.project().info().name),
            },
            None => "<kopercad.Project (closed)>".into(),
        }
    }
}

/// The built-in symbol library: ``[{"id", "name", "aliases", "pins"}]``.
#[pyfunction]
fn symbols(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    let library = crate::library().map_err(err)?;
    let list: Vec<Value> = library
        .iter()
        .map(|s| {
            serde_json::json!({
                "id": s.id,
                "name": s.name,
                "aliases": s.aliases,
                "pins": s.connections.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            })
        })
        .collect();
    to_py(py, &Value::Array(list))
}

#[pymodule]
fn kopercad(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyProject>()?;
    m.add("KoperError", m.py().get_type::<KoperError>())?;
    // The name from before the rename, so old scripts keep catching errors.
    m.add("NovoError", m.py().get_type::<KoperError>())?;
    m.add_function(wrap_pyfunction!(symbols, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
