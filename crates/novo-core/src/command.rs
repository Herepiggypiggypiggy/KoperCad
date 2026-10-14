//! Serializable commands: the one door for UI actions, macros, Python scripts
//! and AI edits.
//!
//! A [`Command`] is plain data (it round-trips through JSON), so anything that
//! can produce JSON can drive the model. Each command maps onto [`Tx`] methods;
//! [`Document::execute`](crate::Document::execute) runs one as an undo step and
//! [`Tx::execute`] runs several inside one.

use serde::{Deserialize, Serialize};

use crate::entity::{
    CableCore, DeviceKind, Endpoint, PageFormat, PageKind, Pin, PotentialKind, PotentialSpec,
    ProjectInfo, SymbolKey, SymbolRole, TextContent, TextPlacement, Value,
};
use crate::error::Result;
use crate::geom::{Placement, Point};
use crate::id::{AnyId, CableId, DeviceId, PageId, PartId, PotentialId, SymbolId, TextId, WireId};
use crate::record::Record;
use crate::tag::DeviceTag;
use crate::tx::Tx;

/// One edit, as data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Command {
    SetProjectInfo {
        info: ProjectInfo,
    },
    AddPage {
        name: String,
        #[serde(default)]
        title: String,
        #[serde(default)]
        kind: PageKind,
        #[serde(default)]
        format: PageFormat,
    },
    AddDevice {
        tag: DeviceTag,
        #[serde(default)]
        description: String,
        #[serde(default)]
        kind: DeviceKind,
    },
    SetTag {
        device: DeviceId,
        tag: DeviceTag,
    },
    PlaceSymbol {
        page: PageId,
        symbol: SymbolKey,
        placement: Placement,
        #[serde(default)]
        role: SymbolRole,
        #[serde(default)]
        pins: Vec<Pin>,
        #[serde(default)]
        bridges: Vec<Vec<u16>>,
        #[serde(default)]
        pass_through: Vec<Vec<u16>>,
    },
    MoveSymbol {
        symbol: SymbolId,
        placement: Placement,
    },
    /// Links a symbol to a device or terminals, or unlinks it; see
    /// [`Tx::set_role`].
    SetRole {
        symbol: SymbolId,
        role: SymbolRole,
    },
    AddJunction {
        page: PageId,
        position: Point,
    },
    /// Adds a text or label; see [`Tx::add_text`].
    AddText {
        page: PageId,
        content: TextContent,
        #[serde(default)]
        placement: TextPlacement,
        #[serde(default)]
        attached_to: Option<AnyId>,
    },
    /// Moves, rotates or re-anchors a text on its own.
    MoveText {
        text: TextId,
        placement: TextPlacement,
    },
    /// Changes what a text shows and/or its height; see [`Tx::set_text`].
    SetText {
        text: TextId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        content: Option<TextContent>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        height_mm: Option<f64>,
    },
    Connect {
        from: Endpoint,
        to: Endpoint,
        #[serde(default)]
        route: Vec<Point>,
    },
    SplitWire {
        wire: WireId,
        at: Point,
    },
    SetWireNumber {
        wire: WireId,
        number: Option<String>,
    },
    /// Sets or clears the potential one wire defines.
    SetWirePotential {
        wire: WireId,
        potential: Option<PotentialId>,
    },
    /// Assigns potentials to wires in order, by name, creating missing
    /// ones; see [`Tx::assign_potentials`]. Use
    /// [`PotentialSet::specs`](crate::PotentialSet::specs) for L1/L2/L3/N/PE.
    AssignPotentials {
        wires: Vec<WireId>,
        potentials: Vec<PotentialSpec>,
    },
    AddPotential {
        name: String,
        #[serde(default)]
        kind: PotentialKind,
        #[serde(default)]
        voltage: Option<f64>,
    },
    AddTerminal {
        strip: DeviceId,
        name: String,
    },
    AddCable {
        tag: DeviceTag,
        #[serde(default)]
        cores: Vec<CableCore>,
    },
    AssignCore {
        cable: CableId,
        core: usize,
        wire: Option<WireId>,
    },
    AddPart {
        manufacturer: String,
        order_number: String,
        #[serde(default)]
        type_number: String,
        #[serde(default)]
        description: String,
    },
    AssignPart {
        device: DeviceId,
        part: PartId,
    },
    SetProperty {
        target: AnyId,
        key: String,
        /// `None` removes the property.
        value: Option<Value>,
    },
    /// Replaces a whole object with a new version (same id). The general
    /// edit for anything the specific commands do not cover.
    Replace {
        record: Record,
    },
    /// Inserts an object with a given, unused id (paste, import).
    Insert {
        record: Record,
    },
    /// Deletes an object and what depends on it; see [`Tx::remove`].
    Remove {
        id: AnyId,
    },
    /// Several commands as one undo step.
    Batch {
        label: String,
        commands: Vec<Command>,
    },
}

impl Command {
    /// A short human label for the undo menu.
    pub fn label(&self) -> String {
        match self {
            Command::SetProjectInfo { .. } => "Edit project info".into(),
            Command::AddPage { .. } => "Add page".into(),
            Command::AddDevice { .. } => "Add device".into(),
            Command::SetTag { .. } => "Rename device".into(),
            Command::PlaceSymbol { .. } => "Place symbol".into(),
            Command::MoveSymbol { .. } => "Move symbol".into(),
            Command::SetRole { .. } => "Cross reference".into(),
            Command::AddJunction { .. } => "Add junction".into(),
            Command::AddText { .. } => "Add text".into(),
            Command::MoveText { .. } => "Move text".into(),
            Command::SetText { .. } => "Edit text".into(),
            Command::Connect { .. } => "Draw wire".into(),
            Command::SplitWire { .. } => "Branch wire".into(),
            Command::SetWireNumber { .. } => "Number wire".into(),
            Command::AddPotential { .. } => "Add potential".into(),
            Command::SetWirePotential { .. } | Command::AssignPotentials { .. } => {
                "Assign potential".into()
            }
            Command::AddTerminal { .. } => "Add terminal".into(),
            Command::AddCable { .. } => "Add cable".into(),
            Command::AssignCore { .. } => "Assign cable core".into(),
            Command::AddPart { .. } => "Add part".into(),
            Command::AssignPart { .. } => "Assign part".into(),
            Command::SetProperty { .. } => "Set property".into(),
            Command::Replace { .. } => "Edit".into(),
            Command::Insert { .. } => "Insert".into(),
            Command::Remove { .. } => "Delete".into(),
            Command::Batch { label, .. } => label.clone(),
        }
    }
}

impl Tx<'_> {
    /// Runs a command inside this edit. Returns the ids of objects it created,
    /// in creation order.
    pub fn execute(&mut self, command: Command) -> Result<Vec<AnyId>> {
        let created: AnyId = match command {
            Command::SetProjectInfo { info } => {
                self.set_info(|i| *i = info)?;
                return Ok(Vec::new());
            }
            Command::AddPage {
                name,
                title,
                kind,
                format,
            } => {
                let page = self.add_page(name)?;
                self.update::<crate::Page>(page, |p| {
                    p.title = title;
                    p.kind = kind;
                    p.format = format;
                })?;
                page.into()
            }
            Command::AddDevice {
                tag,
                description,
                kind,
            } => self.add_device_of_kind(tag, description, kind)?.into(),
            Command::SetTag { device, tag } => {
                self.set_tag(device, tag)?;
                return Ok(Vec::new());
            }
            Command::PlaceSymbol {
                page,
                symbol,
                placement,
                role,
                pins,
                bridges,
                pass_through,
            } => {
                // Inserted in one step: a multi-level terminal is only valid
                // together with its bridges.
                self.insert(|id| crate::SymbolInstance {
                    id,
                    page,
                    symbol,
                    placement,
                    role,
                    pins,
                    bridges,
                    pass_through,
                    properties: Default::default(),
                })?
                .into()
            }
            Command::MoveSymbol { symbol, placement } => {
                self.move_symbol(symbol, placement)?;
                return Ok(Vec::new());
            }
            Command::SetRole { symbol, role } => {
                self.set_role(symbol, role)?;
                return Ok(Vec::new());
            }
            Command::AddJunction { page, position } => self.add_junction(page, position)?.into(),
            Command::AddText {
                page,
                content,
                placement,
                attached_to,
            } => self.add_text(page, content, placement, attached_to)?.into(),
            Command::MoveText { text, placement } => {
                self.move_text(text, placement)?;
                return Ok(Vec::new());
            }
            Command::SetText {
                text,
                content,
                height_mm,
            } => {
                self.set_text(text, content, height_mm)?;
                return Ok(Vec::new());
            }
            Command::Connect { from, to, route } => {
                let wire = self.connect(from, to)?;
                if !route.is_empty() {
                    self.update::<crate::Wire>(wire, |w| w.route = route)?;
                }
                wire.into()
            }
            Command::SplitWire { wire, at } => {
                let (junction, second) = self.split_wire(wire, at)?;
                return Ok(vec![junction.into(), second.into()]);
            }
            Command::SetWireNumber { wire, number } => {
                self.update::<crate::Wire>(wire, |w| w.number = number)?;
                return Ok(Vec::new());
            }
            Command::SetWirePotential { wire, potential } => {
                self.set_wire_potential(wire, potential)?;
                return Ok(Vec::new());
            }
            Command::AssignPotentials { wires, potentials } => {
                let created = self.assign_potentials(&wires, potentials)?;
                return Ok(created.into_iter().map(AnyId::from).collect());
            }
            Command::AddPotential {
                name,
                kind,
                voltage,
            } => {
                let id = self.add_potential(name, kind)?;
                if voltage.is_some() {
                    self.update::<crate::Potential>(id, |p| p.voltage = voltage)?;
                }
                id.into()
            }
            Command::AddTerminal { strip, name } => self.add_terminal(strip, name)?.into(),
            Command::AddCable { tag, cores } => self.add_cable(tag, cores)?.into(),
            Command::AssignCore { cable, core, wire } => {
                self.assign_core(cable, core, wire)?;
                return Ok(Vec::new());
            }
            Command::AddPart {
                manufacturer,
                order_number,
                type_number,
                description,
            } => {
                let id = self.add_part(manufacturer, order_number)?;
                self.update::<crate::Part>(id, |p| {
                    p.type_number = type_number;
                    p.description = description;
                })?;
                id.into()
            }
            Command::AssignPart { device, part } => {
                self.assign_part(device, part)?;
                return Ok(Vec::new());
            }
            Command::SetProperty { target, key, value } => {
                self.set_property(target, key, value)?;
                return Ok(Vec::new());
            }
            Command::Replace { record } => {
                self.replace(record)?;
                return Ok(Vec::new());
            }
            Command::Insert { record } => self.insert_record(record)?,
            Command::Remove { id } => {
                self.remove(id)?;
                return Ok(Vec::new());
            }
            Command::Batch { commands, .. } => {
                let mut created = Vec::new();
                for command in commands {
                    created.extend(self.execute(command)?);
                }
                return Ok(created);
            }
        };
        Ok(vec![created])
    }
}
