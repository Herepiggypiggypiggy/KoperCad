//! Scripting API (owned by WS-28): what the `novoelectric` Python module
//! exposes, as plain Rust.
//!
//! A [`ScriptProject`] is an open project with undo, optionally backed by a
//! project file. Every edit method builds one [`novo_core::Command`] and runs
//! it as one undo step, exactly as the UI does, so scripts, the app and the
//! AI assistant share one door into the model. Queries return JSON values so
//! the Python side gets plain dicts and lists.
//!
//! The Python bindings live in `python.rs` behind the `python` feature, so
//! the workspace builds and tests without a Python toolchain. Build the wheel
//! with `maturin build --release` in this crate's folder; it installs with
//! `pip install --user` (no admin rights).
//!
//! ```
//! use novo_py::{Role, ScriptProject};
//!
//! let mut p = ScriptProject::new("Demo");
//! let page = p.add_page("1", "Main circuit")?;
//! let k1 = p.add_device("-K1", "Contactor", "general")?;
//! let coil = p.place(page, "coil", 0.0, 0.0, 0, false, Role::Device { device: k1, main: true })?;
//! p.set_tag(k1, "-K2")?;
//! assert_eq!(p.device_by_tag("-K2")?, Some(k1));
//! p.undo()?;
//! assert_eq!(p.device_by_tag("-K1")?, Some(k1));
//! # let _ = coil;
//! # Ok::<(), novo_py::ScriptError>(())
//! ```

use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use novo_core::{
    AnyId, ChangeSet, Command, DeviceId, DeviceKind, DeviceTag, Document, Endpoint, ModelError,
    PageFormat, PageId, PageKind, PartId, Placement, Point, PotentialId, PotentialKind,
    PotentialSpec, Project, Rotation, SymbolId, SymbolKey, SymbolRole, TagError, TerminalId,
    WireId, natural_cmp,
};
use novo_storage::{Session, StorageError, autosave_path};
use novo_symbols::Library;
use serde_json::{Value, json};
use thiserror::Error;

mod export;
#[cfg(feature = "python")]
mod python;

pub use export::{
    ReportFormat, generate, is_project_path, report_bytes, report_ids, report_json, write_export,
};

/// Why a script call was refused. A refused edit changes nothing.
#[derive(Debug, Error)]
pub enum ScriptError {
    #[error(transparent)]
    Model(#[from] ModelError),
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error(transparent)]
    Tag(#[from] TagError),
    #[error("not a valid command: {0}")]
    Command(serde_json::Error),
    #[error(
        "no symbol `{0}` in the library (use a library id such as `iec/contacts/no` or an alias such as `no`)"
    )]
    UnknownSymbol(String),
    #[error("{symbol} has no pin `{pin}`")]
    UnknownPin { symbol: SymbolId, pin: String },
    #[error("{symbol} has more than {max} pins", max = u16::MAX)]
    TooManyPins { symbol: String },
    #[error("unknown {what} `{value}`")]
    UnknownValue { what: &'static str, value: String },
    #[error(
        "{} has unsaved work from an app session that did not close cleanly; open it in NovoElectric to recover or discard that work first",
        .0.display()
    )]
    RecoveryPending(PathBuf),
    #[error("{} already exists; pass overwrite to replace it", .0.display())]
    Exists(PathBuf),
    #[error("this project has no file yet: use save_as(path)")]
    NoFile,
    #[error("`{0}` ran but did not create the object it should have; please report this")]
    NothingCreated(String),
    #[error("refusing to write an export over {}: it is a project file", .0.display())]
    ProjectFileTarget(PathBuf),
    #[error("cannot write {}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("export failed: {0}")]
    Export(String),
    #[error("the symbol library did not load: {0}")]
    Library(String),
}

pub type Result<T, E = ScriptError> = std::result::Result<T, E>;

/// What a placed symbol stands for.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Role {
    /// Not linked to anything yet.
    #[default]
    Unassigned,
    /// A function of a device; `main` marks the main function (the coil).
    Device { device: DeviceId, main: bool },
    /// One terminal of a terminal strip.
    Terminal(TerminalId),
    /// A potential definition point.
    Potential(PotentialId),
    /// An interruption point (off-page connector) with this name.
    Interruption(String),
}

impl From<Role> for SymbolRole {
    fn from(role: Role) -> SymbolRole {
        match role {
            Role::Unassigned => SymbolRole::Unassigned,
            Role::Device { device, main } => SymbolRole::Device { device, main },
            Role::Terminal(terminal) => SymbolRole::Terminal {
                terminals: vec![terminal],
            },
            Role::Potential(potential) => SymbolRole::PotentialFeed { potential },
            Role::Interruption(name) => SymbolRole::Interruption { name },
        }
    }
}

/// The built-in symbol library, loaded once per process.
pub fn library() -> Result<&'static Library> {
    static LIBRARY: OnceLock<std::result::Result<Library, String>> = OnceLock::new();
    LIBRARY
        .get_or_init(|| Library::builtin().map_err(|e| e.to_string()))
        .as_ref()
        .map_err(|e| ScriptError::Library(e.clone()))
}

/// An open project, edited one command (one undo step) at a time.
#[derive(Debug)]
pub struct ScriptProject {
    doc: Document,
    /// The project file, once the project has one. Holding it keeps the file
    /// locked against the app and other scripts.
    session: Option<Session>,
}

impl ScriptProject {
    /// A new, unsaved project with the recommended rules.
    pub fn new(name: &str) -> ScriptProject {
        ScriptProject {
            doc: Document::new(Project::new(name)),
            session: None,
        }
    }

    /// A new project saved at once to `path`. An existing file is refused
    /// unless `overwrite`; see [`ScriptProject::save_as`].
    pub fn create(path: impl AsRef<Path>, name: &str, overwrite: bool) -> Result<ScriptProject> {
        let mut project = ScriptProject::new(name);
        project.save_as(path, overwrite)?;
        Ok(project)
    }

    /// Opens a project file for editing. A file with recovered work from a
    /// crashed app session is refused and left untouched, so a script can
    /// never save over that work.
    pub fn open(path: impl AsRef<Path>) -> Result<ScriptProject> {
        let path = path.as_ref();
        let opened = Session::open(path)?;
        if opened.recovered.is_some() {
            // Closing while the decision is pending keeps the recovery file.
            opened.session.close()?;
            return Err(ScriptError::RecoveryPending(path.to_path_buf()));
        }
        Ok(ScriptProject {
            doc: Document::new(opened.project),
            session: Some(opened.session),
        })
    }

    pub fn project(&self) -> &Project {
        self.doc.project()
    }

    /// The project file, if the project has one.
    pub fn path(&self) -> Option<&Path> {
        self.session.as_ref().map(Session::path)
    }

    /// Whether the project differs from the last save.
    pub fn is_dirty(&self) -> bool {
        self.doc.is_dirty()
    }

    /// Writes the project to its file.
    pub fn save(&mut self) -> Result<()> {
        let session = self.session.as_mut().ok_or(ScriptError::NoFile)?;
        session.save(self.doc.project())?;
        self.doc.mark_saved();
        Ok(())
    }

    /// Writes the project to `path` and continues there. Another existing
    /// file is refused unless `overwrite`, so a wrong path cannot wipe a real
    /// project. A file with unrecovered work from a crashed app session is
    /// refused even then, because replacing it would delete that work.
    /// Saving onto the project's own file is always allowed.
    pub fn save_as(&mut self, path: impl AsRef<Path>, overwrite: bool) -> Result<()> {
        let path = path.as_ref();
        let own_file = self.path().is_some_and(|own| same_file(own, path));
        if !own_file {
            if autosave_path(path).exists() {
                return Err(ScriptError::RecoveryPending(path.to_path_buf()));
            }
            if !overwrite && path.exists() {
                return Err(ScriptError::Exists(path.to_path_buf()));
            }
        }
        match &mut self.session {
            Some(session) => session.save_as(path, self.doc.project())?,
            None => self.session = Some(Session::create(path, self.doc.project())?),
        }
        self.doc.mark_saved();
        Ok(())
    }

    /// Releases the file. Unsaved changes are dropped; call `save` first.
    pub fn close(self) -> Result<()> {
        match self.session {
            Some(session) => Ok(session.close()?),
            None => Ok(()),
        }
    }

    fn record(&mut self, changes: &ChangeSet) {
        if let Some(session) = &mut self.session {
            session.record(changes);
        }
    }

    // ----- commands ---------------------------------------------------------

    /// Runs one command as one undo step and returns the ids it created.
    pub fn execute(&mut self, command: Command) -> Result<Vec<AnyId>> {
        let outcome = self.doc.execute(command)?;
        self.record(&outcome.changes);
        Ok(outcome.created)
    }

    /// Runs a command given in its JSON form (`{"command": "add_page", ...}`),
    /// the same form macros and the AI assistant use.
    pub fn execute_json(&mut self, command: &str) -> Result<Vec<AnyId>> {
        let command: Command = serde_json::from_str(command).map_err(ScriptError::Command)?;
        self.execute(command)
    }

    /// Runs a command that creates one object and returns that object's id,
    /// picked out of what it created by `pick`.
    fn execute_one<T>(&mut self, command: Command, pick: fn(AnyId) -> Option<T>) -> Result<T> {
        let label = command.label();
        let created = self.execute(command)?;
        created
            .first()
            .copied()
            .and_then(pick)
            .ok_or(ScriptError::NothingCreated(label))
    }

    pub fn add_page(&mut self, name: &str, title: &str) -> Result<PageId> {
        self.execute_one(
            Command::AddPage {
                name: name.into(),
                title: title.into(),
                kind: PageKind::default(),
                format: PageFormat::default(),
            },
            |id| match id {
                AnyId::Page(id) => Some(id),
                _ => None,
            },
        )
    }

    /// Adds a device. `kind` is `general`, `terminal_strip`, `connector` or
    /// `plc`.
    pub fn add_device(&mut self, tag: &str, description: &str, kind: &str) -> Result<DeviceId> {
        let kind: DeviceKind = parse_kind("device kind", kind)?;
        self.execute_one(
            Command::AddDevice {
                tag: DeviceTag::parse(tag)?,
                description: description.into(),
                kind,
            },
            |id| match id {
                AnyId::Device(id) => Some(id),
                _ => None,
            },
        )
    }

    pub fn add_terminal(&mut self, strip: DeviceId, name: &str) -> Result<TerminalId> {
        self.execute_one(
            Command::AddTerminal {
                strip,
                name: name.into(),
            },
            |id| match id {
                AnyId::Terminal(id) => Some(id),
                _ => None,
            },
        )
    }

    /// Adds a potential. `kind` is `line`, `neutral`, `protective_earth`,
    /// `pen`, `dc_positive`, `dc_negative` or `signal`.
    pub fn add_potential(
        &mut self,
        name: &str,
        kind: &str,
        voltage: Option<f64>,
    ) -> Result<PotentialId> {
        let kind: PotentialKind = parse_kind("potential kind", kind)?;
        self.execute_one(
            Command::AddPotential {
                name: name.into(),
                kind,
                voltage,
            },
            |id| match id {
                AnyId::Potential(id) => Some(id),
                _ => None,
            },
        )
    }

    /// Adds a part (an article) to the project's parts, e.g. a contactor by
    /// manufacturer and order number.
    pub fn add_part(
        &mut self,
        manufacturer: &str,
        order_number: &str,
        type_number: &str,
        description: &str,
    ) -> Result<PartId> {
        self.execute_one(
            Command::AddPart {
                manufacturer: manufacturer.into(),
                order_number: order_number.into(),
                type_number: type_number.into(),
                description: description.into(),
            },
            |id| match id {
                AnyId::Part(id) => Some(id),
                _ => None,
            },
        )
    }

    /// Adds a part to a device's part list.
    pub fn assign_part(&mut self, device: DeviceId, part: PartId) -> Result<()> {
        self.execute(Command::AssignPart { device, part })?;
        Ok(())
    }

    /// Renames a device: a new tag, e.g. `-K2` or `=A1+CAB1-K2`.
    pub fn set_tag(&mut self, device: DeviceId, tag: &str) -> Result<()> {
        self.execute(Command::SetTag {
            device,
            tag: DeviceTag::parse(tag)?,
        })?;
        Ok(())
    }

    /// Places a library symbol, found by id (`iec/contacts/no`) or alias
    /// (`no`), with the library's pins, internal bridges and pass-through
    /// paths. `rotation` is in degrees: 0, 90, 180 or 270.
    #[allow(clippy::too_many_arguments)]
    pub fn place(
        &mut self,
        page: PageId,
        symbol: &str,
        x: f64,
        y: f64,
        rotation: u16,
        mirrored: bool,
        role: Role,
    ) -> Result<SymbolId> {
        let library = library()?;
        let def = library
            .get(symbol)
            .or_else(|| library.by_alias(symbol))
            .ok_or_else(|| ScriptError::UnknownSymbol(symbol.into()))?;
        let rotation = match rotation {
            0 => Rotation::R0,
            90 => Rotation::R90,
            180 => Rotation::R180,
            270 => Rotation::R270,
            other => {
                return Err(ScriptError::UnknownValue {
                    what: "rotation (use 0, 90, 180 or 270)",
                    value: other.to_string(),
                });
            }
        };
        let to_u16 = |group: &Vec<usize>| -> Result<Vec<u16>> {
            group
                .iter()
                .map(|&i| {
                    u16::try_from(i).map_err(|_| ScriptError::TooManyPins {
                        symbol: def.id.clone(),
                    })
                })
                .collect()
        };
        let bridges = def
            .bridges()
            .iter()
            .map(to_u16)
            .collect::<Result<Vec<_>>>()?;
        // The core keeps pass-through paths apart from bridges.
        let pass_through = def
            .pass_through()
            .paths()
            .iter()
            .map(to_u16)
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .filter(|group| !bridges.contains(group))
            .collect();
        let pins = def
            .connections
            .iter()
            .map(|c| novo_core::Pin::new(c.id.clone(), c.at[0], c.at[1]))
            .collect();
        self.execute_one(
            Command::PlaceSymbol {
                page,
                symbol: SymbolKey(def.id.clone()),
                placement: Placement {
                    origin: Point::new(x, y),
                    rotation,
                    mirrored,
                },
                role: role.into(),
                pins,
                bridges,
                pass_through,
            },
            |id| match id {
                AnyId::Symbol(id) => Some(id),
                _ => None,
            },
        )
    }

    /// The endpoint of a symbol's pin, by its designation (`13`, `A1`).
    pub fn pin(&self, symbol: SymbolId, pin: &str) -> Result<Endpoint> {
        let instance = self
            .project()
            .symbol(symbol)
            .ok_or(ModelError::NotFound(symbol.into()))?;
        let index = instance
            .pins
            .iter()
            .position(|p| p.name == pin)
            .ok_or_else(|| ScriptError::UnknownPin {
                symbol,
                pin: pin.into(),
            })?;
        let index = u16::try_from(index).map_err(|_| ScriptError::TooManyPins {
            symbol: instance.symbol.0.clone(),
        })?;
        Ok(Endpoint::pin(symbol, index))
    }

    /// Draws a wire between two pins, each given as symbol and designation.
    pub fn connect(&mut self, from: (SymbolId, &str), to: (SymbolId, &str)) -> Result<WireId> {
        let from = self.pin(from.0, from.1)?;
        let to = self.pin(to.0, to.1)?;
        self.execute_one(
            Command::Connect {
                from,
                to,
                route: Vec::new(),
                page: None,
            },
            |id| match id {
                AnyId::Wire(id) => Some(id),
                _ => None,
            },
        )
    }

    /// Assigns potentials to wires in order, by name (`L1`, `L2`, `L3`),
    /// reusing existing potentials and creating missing ones, as one undo
    /// step. A new potential's kind follows its name (`N`, `PE`, `PEN`,
    /// `L+`, `L-`, otherwise a line).
    pub fn assign_potentials(&mut self, wires: &[WireId], names: &[&str]) -> Result<()> {
        let potentials = names
            .iter()
            .map(|&name| {
                let existing = self.project().potentials().find(|p| p.name == name);
                match existing {
                    Some(p) => PotentialSpec::new(name, p.kind, p.voltage),
                    None => PotentialSpec::new(name, kind_from_name(name), None),
                }
            })
            .collect();
        self.execute(Command::AssignPotentials {
            wires: wires.to_vec(),
            potentials,
        })?;
        Ok(())
    }

    /// Undoes the last step. Returns whether there was one.
    pub fn undo(&mut self) -> Result<bool> {
        let changes = self.doc.undo()?;
        if let Some(changes) = &changes {
            self.record(changes);
        }
        Ok(changes.is_some())
    }

    /// Redoes the last undone step. Returns whether there was one.
    pub fn redo(&mut self) -> Result<bool> {
        let changes = self.doc.redo()?;
        if let Some(changes) = &changes {
            self.record(changes);
        }
        Ok(changes.is_some())
    }

    pub fn can_undo(&self) -> bool {
        self.doc.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.doc.can_redo()
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.doc.undo_label()
    }

    // ----- queries ----------------------------------------------------------

    /// The device with this tag, if exactly one has it.
    pub fn device_by_tag(&self, tag: &str) -> Result<Option<DeviceId>> {
        let found = self.project().devices_by_tag(&DeviceTag::parse(tag)?);
        Ok(match found.as_slice() {
            [one] => Some(*one),
            _ => None,
        })
    }

    pub fn info(&self) -> Value {
        info_json(self.project(), self.path())
    }

    pub fn pages(&self) -> Value {
        pages_json(self.project())
    }

    pub fn devices(&self) -> Value {
        devices_json(self.project())
    }

    pub fn symbols(&self) -> Value {
        let p = self.project();
        let mut symbols: Vec<_> = p.symbols().collect();
        symbols.sort_by_key(|s| s.id);
        Value::Array(
            symbols
                .into_iter()
                .map(|s| {
                    json!({
                        "id": s.id.raw(),
                        "page": s.page.raw(),
                        "symbol": s.symbol.0,
                        "x": s.placement.origin.x,
                        "y": s.placement.origin.y,
                        "role": s.role,
                        "pins": s.pins.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
                    })
                })
                .collect(),
        )
    }

    pub fn wires(&self) -> Value {
        wires_json(self.project())
    }

    pub fn potentials(&self) -> Value {
        potentials_json(self.project())
    }

    pub fn terminals(&self) -> Value {
        let p = self.project();
        let mut terminals: Vec<_> = p.terminals().collect();
        terminals.sort_by_key(|t| t.id);
        Value::Array(
            terminals
                .into_iter()
                .map(|t| {
                    json!({
                        "id": t.id.raw(),
                        "strip": t.strip.raw(),
                        "name": t.name,
                        "designation": p.terminal_designation(t.id),
                    })
                })
                .collect(),
        )
    }

    /// The netlist: each net's pins (by designation), junction count, wires
    /// and potentials.
    pub fn nets(&self) -> Value {
        nets_json(self.project())
    }

    /// A built-in report (`toc`, `bom`, `devices`, `wires`) as plain data.
    pub fn report(&self, id: &str) -> Result<Value> {
        Ok(report_json(&generate(self.project(), id)?))
    }

    /// Writes reports to `path`: `.xlsx` (one sheet per report; every
    /// report when `ids` is empty) or `.csv` (exactly one report). Never
    /// writes over a project file.
    pub fn export_reports(&self, ids: &[&str], path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        if is_project_path(path) {
            return Err(ScriptError::ProjectFileTarget(path.to_path_buf()));
        }
        let bytes = report_bytes(self.project(), ids, ReportFormat::from_path(path)?)?;
        write_export(path, &bytes, self.path())
    }

    /// The whole project in its diffable JSON form.
    pub fn to_json(&self) -> String {
        self.project().to_json()
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

fn parse_kind<T: serde::de::DeserializeOwned>(what: &'static str, value: &str) -> Result<T> {
    serde_json::from_value(Value::String(value.into())).map_err(|_| ScriptError::UnknownValue {
        what,
        value: value.into(),
    })
}

fn kind_from_name(name: &str) -> PotentialKind {
    match name.to_ascii_uppercase().as_str() {
        "N" => PotentialKind::Neutral,
        "PE" => PotentialKind::ProtectiveEarth,
        "PEN" => PotentialKind::Pen,
        "L+" => PotentialKind::DcPositive,
        "L-" | "M" => PotentialKind::DcNegative,
        _ => PotentialKind::Line,
    }
}

// ----- JSON views shared with the command-line tool -------------------------

/// Counts and names of a project, for `novo info` and `Project.info()`.
pub fn info_json(p: &Project, path: Option<&Path>) -> Value {
    json!({
        "name": p.info().name,
        "description": p.info().description,
        "path": path.map(|p| p.display().to_string()),
        "pages": p.pages().count(),
        "devices": p.devices().count(),
        "symbols": p.symbols().count(),
        "wires": p.wires().count(),
        "potentials": p.potentials().count(),
        "terminals": p.terminals().count(),
        "cables": p.cables().count(),
        "parts": p.parts().count(),
    })
}

pub fn pages_json(p: &Project) -> Value {
    Value::Array(
        p.pages_in_order()
            .into_iter()
            .map(|page| json!({"id": page.id.raw(), "name": page.name, "title": page.title}))
            .collect(),
    )
}

pub fn devices_json(p: &Project) -> Value {
    let mut devices: Vec<_> = p.devices().collect();
    devices.sort_by(|a, b| natural_cmp(&a.tag.to_string(), &b.tag.to_string()));
    Value::Array(
        devices
            .into_iter()
            .map(|d| {
                json!({
                    "id": d.id.raw(),
                    "tag": d.tag.to_string(),
                    "description": d.description,
                    "kind": d.kind,
                })
            })
            .collect(),
    )
}

pub fn potentials_json(p: &Project) -> Value {
    let mut potentials: Vec<_> = p.potentials().collect();
    potentials.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    Value::Array(
        potentials
            .into_iter()
            .map(|pot| {
                json!({
                    "id": pot.id.raw(),
                    "name": pot.name,
                    "kind": pot.kind,
                    "voltage": pot.voltage,
                })
            })
            .collect(),
    )
}

/// How a netlist names an endpoint: `-K1:13` and `-X1:3` for devices and
/// terminals; `feed L1:1`, `ip A:1` and `symbol 12:1` for symbols without a
/// tag, so their pins stay distinct.
pub fn endpoint_name(p: &Project, end: Endpoint) -> String {
    let (symbol, pin) = match end {
        Endpoint::Pin { symbol, pin } => (symbol, pin),
        Endpoint::Junction { junction } => return junction.to_string(),
        Endpoint::Free { at } => return format!("open end ({:.1}, {:.1})", at.x, at.y),
    };
    let Some(instance) = p.symbol(symbol) else {
        return format!("{symbol} pin {pin}");
    };
    let pin_name = instance
        .pins
        .get(usize::from(pin))
        .map_or_else(|| pin.to_string(), |pin| pin.name.clone());
    match &instance.role {
        SymbolRole::Device { .. } | SymbolRole::Terminal { .. } => p
            .pin_designation(novo_core::PinRef::new(symbol, pin))
            .unwrap_or(pin_name),
        SymbolRole::PotentialFeed { potential } => {
            let name = p.potential(*potential).map_or("?", |pot| pot.name.as_str());
            format!("feed {name}:{pin_name}")
        }
        SymbolRole::Interruption { name } => format!("ip {name}:{pin_name}"),
        SymbolRole::Unassigned => format!("symbol {}:{pin_name}", symbol.raw()),
    }
}

fn potential_names(p: &Project, ids: &[PotentialId]) -> Vec<String> {
    let mut names: Vec<String> = ids
        .iter()
        .filter_map(|id| p.potential(*id).map(|pot| pot.name.clone()))
        .collect();
    names.sort_by(|a, b| natural_cmp(a, b));
    names
}

pub fn wires_json(p: &Project) -> Value {
    let netlist = p.netlist();
    let mut wires: Vec<_> = p.wires().collect();
    wires.sort_by_key(|w| w.id);
    Value::Array(
        wires
            .into_iter()
            .map(|w| {
                let potentials = netlist
                    .net_of_wire(w)
                    .map(|net| potential_names(p, &netlist.net(net).potentials))
                    .unwrap_or_default();
                json!({
                    "id": w.id.raw(),
                    "page": w.page.raw(),
                    "from": endpoint_name(p, w.from),
                    "to": endpoint_name(p, w.to),
                    "number": w.number,
                    "potentials": potentials,
                })
            })
            .collect(),
    )
}

pub fn nets_json(p: &Project) -> Value {
    let netlist = p.netlist();
    let mut nets: Vec<(Vec<String>, Value)> = netlist
        .nets()
        .iter()
        .map(|net| {
            let mut pins: Vec<String> = net
                .endpoints
                .iter()
                .filter(|e| e.as_pin().is_some())
                .map(|&e| endpoint_name(p, e))
                .collect();
            pins.sort_by(|a, b| natural_cmp(a, b));
            let junctions = net.endpoints.len() - pins.len();
            let value = json!({
                "pins": pins,
                "junctions": junctions,
                "wires": net.wires.iter().map(|w| w.raw()).collect::<Vec<_>>(),
                "potentials": potential_names(p, &net.potentials),
            });
            (pins, value)
        })
        .collect();
    nets.sort_by(|a, b| {
        a.0.iter()
            .zip(&b.0)
            .map(|(x, y)| natural_cmp(x, y))
            .find(|o| *o != Ordering::Equal)
            .unwrap_or_else(|| a.0.len().cmp(&b.0.len()))
    });
    Value::Array(nets.into_iter().map(|(_, v)| v).collect())
}
