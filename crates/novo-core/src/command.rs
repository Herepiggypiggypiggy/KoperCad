//! Serializable commands: the one door for UI actions, macros, Python scripts
//! and AI edits.
//!
//! A [`Command`] is plain data (it round-trips through JSON), so anything that
//! can produce JSON can drive the model. Each command maps onto [`Tx`] methods;
//! [`Document::execute`](crate::Document::execute) runs one as an undo step and
//! [`Tx::execute`] runs several inside one.

use serde::{Deserialize, Serialize};

use crate::entity::{
    BlockOverride, BlockParameter, CableCore, DeviceKind, Endpoint, MountingPlate, PageFormat,
    PageKind, PeOverride, Pin, PotentialKind, PotentialSet, PotentialSpec, ProjectInfo, RailKind,
    SymbolKey, SymbolRole, TextContent, TextPlacement, Value,
};
use crate::error::{ModelError, Result};
use crate::geom::{Placement, Point};
use crate::id::{
    AnyId, BlockDefinitionId, BlockInstanceId, CabinetPlacementId, CableId, DeviceId, PageId,
    PartId, PotentialId, PotentialSystemId, RailId, StructureId, SymbolId, TerminalId, TextId,
    WireId,
};
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
    /// The user deletes a page; see [`Tx::delete_page`].
    DeletePage {
        page: PageId,
    },
    /// Renames a page (its name is what references print).
    RenamePage {
        page: PageId,
        name: String,
    },
    /// Renames a group (`=10D41`) or panel (`+K1B1G1`) everywhere its name
    /// is copied; see [`Tx::rename_level`].
    RenameLevel {
        aspect: crate::tag::Aspect,
        from: Vec<String>,
        to: Vec<String>,
    },
    AddPage {
        name: String,
        #[serde(default)]
        title: String,
        #[serde(default)]
        kind: PageKind,
        #[serde(default)]
        format: PageFormat,
        /// Page group (`=10D41`) and panel (`+K1B1G1`); see [`Page::structure`].
        #[serde(default)]
        structure: DeviceTag,
    },
    /// Moves a page into another page group and/or panel.
    SetPageStructure {
        page: PageId,
        structure: DeviceTag,
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
    /// Moves symbols, junctions and free texts to a page with their wires
    /// and labels; see [`Tx::move_to_page`].
    MoveToPage {
        items: Vec<AnyId>,
        page: PageId,
        #[serde(default)]
        offset: Point,
    },
    /// Issues a revision and freezes the drawn pages; see
    /// [`Tx::issue_revision`].
    IssueRevision {
        index: String,
        date: String,
        #[serde(default)]
        author: String,
        #[serde(default)]
        description: String,
    },
    /// Freezes a page as issued; see [`Tx::freeze_page`].
    FreezePage {
        page: PageId,
        revision: String,
    },
    /// Releases a frozen page: references to it follow the drawing again.
    UnfreezePage {
        page: PageId,
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
        /// The page, only needed when both ends are open
        /// ([`Endpoint::Free`]); otherwise it is that of the pin or
        /// junction.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        page: Option<PageId>,
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
    /// Adds a supply system and its potentials; see
    /// [`Tx::add_potential_system`].
    AddPotentialSystem {
        name: String,
        set: PotentialSet,
        #[serde(default)]
        frequency_hz: Option<f64>,
        /// Prefix for an isolated system's own potentials; see
        /// [`Tx::add_potential_system`].
        #[serde(default)]
        prefix: String,
    },
    /// Feeds a system's potentials from a source symbol's pins, in order.
    FeedSystem {
        symbol: SymbolId,
        pins: Vec<u16>,
        system: PotentialSystemId,
    },
    /// Sets or clears the potential one pin defines.
    SetPinFeed {
        symbol: SymbolId,
        pin: u16,
        potential: Option<PotentialId>,
    },
    /// Sets or clears the potential a terminal feeds.
    SetTerminalFeed {
        terminal: TerminalId,
        potential: Option<PotentialId>,
    },
    /// Sets whether the terminals are PE terminals by rule or by hand
    /// ("Toggle PE terminal"); see [`PeOverride`].
    SetTerminalPe {
        terminals: Vec<TerminalId>,
        mode: PeOverride,
    },
    AddPotential {
        name: String,
        #[serde(default)]
        kind: PotentialKind,
        #[serde(default)]
        voltage: Option<f64>,
    },
    /// Captures a block definition from the drawing; see [`Tx::define_block`].
    DefineBlock {
        name: String,
        items: Vec<AnyId>,
        #[serde(default)]
        origin: Point,
        #[serde(default)]
        parameters: Vec<BlockParameter>,
    },
    /// Places a block; see [`Tx::place_block`].
    PlaceBlock {
        definition: BlockDefinitionId,
        page: PageId,
        origin: Point,
        #[serde(default)]
        values: Vec<(String, Value)>,
    },
    /// Sets a placed block's parameter values; see [`Tx::set_block_values`].
    SetBlockValues {
        instance: BlockInstanceId,
        values: Vec<(String, Value)>,
    },
    /// Marks or clears a hand edit on a block member; see
    /// [`Tx::set_block_override`].
    SetBlockOverride {
        instance: BlockInstanceId,
        member: AnyId,
        which: BlockOverride,
        on: bool,
    },
    /// Stops a placed block from following its definition.
    DetachBlock {
        instance: BlockInstanceId,
        reason: String,
    },
    /// Describes a plant section or location; see [`Tx::add_structure`].
    AddStructure {
        tag: DeviceTag,
        #[serde(default)]
        description: String,
        #[serde(default)]
        enclosure: bool,
    },
    /// Records a declined proposal; see [`Tx::dismiss`]. Remove the
    /// dismissal to bring the proposal back.
    /// Sets the mounting plate of an enclosure; `None` removes it.
    SetPlateSize {
        structure: StructureId,
        plate: Option<MountingPlate>,
    },
    /// Adds a rail or duct to the plate of an enclosure.
    AddRail {
        structure: StructureId,
        x: f64,
        y: f64,
        length: f64,
        #[serde(default)]
        kind: RailKind,
    },
    /// Places a device on the plate, or moves it if it is already placed.
    /// Several together go in a [`Command::Batch`].
    PlaceDevice {
        device: DeviceId,
        structure: StructureId,
        x: f64,
        y: f64,
        #[serde(default)]
        rotated: bool,
        /// The rail and the offset along it.
        #[serde(default)]
        rail: Option<(RailId, f64)>,
    },
    RotatePlacement {
        placement: CabinetPlacementId,
        rotated: bool,
    },
    RemovePlacement {
        placement: CabinetPlacementId,
    },
    Dismiss {
        proposal: String,
        subjects: Vec<AnyId>,
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
            Command::DeletePage { .. } => "Delete page".into(),
            Command::RenamePage { .. } => "Rename page".into(),
            Command::RenameLevel { aspect, .. } => match aspect {
                crate::tag::Aspect::Function => "Rename group".into(),
                _ => "Rename panel".into(),
            },
            Command::AddPage { .. } => "Add page".into(),
            Command::SetPageStructure { .. } => "Move page to group or panel".into(),
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
            Command::AddPotentialSystem { .. } => "Add supply system".into(),
            Command::FeedSystem { .. }
            | Command::SetPinFeed { .. }
            | Command::SetTerminalFeed { .. } => "Feed potential".into(),
            Command::SetTerminalPe { .. } => "Toggle PE terminal".into(),
            Command::AddTerminal { .. } => "Add terminal".into(),
            Command::AddCable { .. } => "Add cable".into(),
            Command::AssignCore { .. } => "Assign cable core".into(),
            Command::AddPart { .. } => "Add part".into(),
            Command::MoveToPage { .. } => "Move to page".into(),
            Command::AddStructure { .. } => "Add structure".into(),
            Command::DefineBlock { .. } => "Define block".into(),
            Command::PlaceBlock { .. } => "Place block".into(),
            Command::SetBlockValues { .. } => "Set block parameters".into(),
            Command::SetBlockOverride { .. } => "Mark block edit".into(),
            Command::DetachBlock { .. } => "Detach block".into(),
            Command::IssueRevision { index, .. } => format!("Issue revision {index}"),
            Command::FreezePage { .. } => "Freeze page".into(),
            Command::UnfreezePage { .. } => "Unfreeze page".into(),
            Command::SetPlateSize { .. } => "Set plate size".into(),
            Command::AddRail { .. } => "Add rail".into(),
            Command::PlaceDevice { .. } => "Place device".into(),
            Command::RotatePlacement { .. } => "Rotate device".into(),
            Command::RemovePlacement { .. } => "Remove from plate".into(),
            Command::Dismiss { .. } => "Dismiss proposal".into(),
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
            Command::RenamePage { page, name } => {
                let name = name.trim().to_owned();
                if name.is_empty() {
                    return Err(ModelError::InvalidValue {
                        id: page.into(),
                        field: "name",
                    });
                }
                self.update::<crate::Page>(page, |p| p.name = name)?;
                return Ok(Vec::new());
            }
            Command::RenameLevel { aspect, from, to } => {
                self.rename_level(aspect, &from, &to)?;
                return Ok(Vec::new());
            }
            Command::DeletePage { page } => {
                self.delete_page(page)?;
                return Ok(Vec::new());
            }
            Command::AddPage {
                name,
                title,
                kind,
                format,
                structure,
            } => {
                let page = self.add_page(name)?;
                self.update::<crate::Page>(page, |p| {
                    p.title = title;
                    p.kind = kind;
                    p.format = format;
                    p.structure = structure;
                })?;
                page.into()
            }
            Command::SetPageStructure { page, structure } => {
                self.update::<crate::Page>(page, |p| p.structure = structure)?;
                return Ok(Vec::new());
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
                    feeds: Vec::new(),
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
            Command::Connect {
                from,
                to,
                route,
                page,
            } => {
                let wire = match page {
                    Some(page) => self.connect_on(page, from, to)?,
                    None => self.connect(from, to)?,
                };
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
            Command::AddPotentialSystem {
                name,
                set,
                frequency_hz,
                prefix,
            } => {
                let (system, created) =
                    self.add_potential_system(name, set, frequency_hz, &prefix)?;
                return Ok(std::iter::once(system.into())
                    .chain(created.into_iter().map(AnyId::from))
                    .collect());
            }
            Command::FeedSystem {
                symbol,
                pins,
                system,
            } => {
                self.feed_system(symbol, &pins, system)?;
                return Ok(Vec::new());
            }
            Command::SetPinFeed {
                symbol,
                pin,
                potential,
            } => {
                self.set_pin_feed(symbol, pin, potential)?;
                return Ok(Vec::new());
            }
            Command::SetTerminalFeed {
                terminal,
                potential,
            } => {
                self.set_terminal_feed(terminal, potential)?;
                return Ok(Vec::new());
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
            Command::SetTerminalPe { terminals, mode } => {
                self.set_terminal_pe(&terminals, mode)?;
                return Ok(Vec::new());
            }
            Command::AddTerminal { strip, name } => self.add_terminal(strip, name)?.into(),
            Command::AddCable { tag, cores } => self.add_cable(tag, cores)?.into(),
            Command::AssignCore { cable, core, wire } => {
                self.assign_core(cable, core, wire)?;
                return Ok(Vec::new());
            }
            Command::MoveToPage {
                items,
                page,
                offset,
            } => {
                self.move_to_page(&items, page, offset)?;
                return Ok(Vec::new());
            }
            Command::IssueRevision {
                index,
                date,
                author,
                description,
            } => self
                .issue_revision(index, date, author, description)?
                .into(),
            Command::FreezePage { page, revision } => {
                self.freeze_page(page, revision)?;
                return Ok(Vec::new());
            }
            Command::UnfreezePage { page } => {
                self.unfreeze_page(page)?;
                return Ok(Vec::new());
            }
            Command::DefineBlock {
                name,
                items,
                origin,
                parameters,
            } => self.define_block(name, &items, origin, parameters)?.into(),
            Command::PlaceBlock {
                definition,
                page,
                origin,
                values,
            } => self.place_block(definition, page, origin, values)?.into(),
            Command::SetBlockValues { instance, values } => {
                self.set_block_values(instance, values)?;
                return Ok(Vec::new());
            }
            Command::SetBlockOverride {
                instance,
                member,
                which,
                on,
            } => {
                self.set_block_override(instance, member, which, on)?;
                return Ok(Vec::new());
            }
            Command::DetachBlock { instance, reason } => {
                self.detach_block(instance, reason)?;
                return Ok(Vec::new());
            }
            Command::AddStructure {
                tag,
                description,
                enclosure,
            } => self.add_structure(tag, description, enclosure)?.into(),
            Command::SetPlateSize { structure, plate } => {
                self.set_plate(structure, plate)?;
                return Ok(Vec::new());
            }
            Command::AddRail {
                structure,
                x,
                y,
                length,
                kind,
            } => self.add_rail(structure, x, y, length, kind)?.into(),
            Command::PlaceDevice {
                device,
                structure,
                x,
                y,
                rotated,
                rail,
            } => {
                let (placement, new) =
                    self.place_device(device, structure, Point::new(x, y), rotated, rail)?;
                if !new {
                    return Ok(Vec::new());
                }
                placement.into()
            }
            Command::RotatePlacement { placement, rotated } => {
                self.update::<crate::CabinetPlacement>(placement, |p| p.rotated = rotated)?;
                return Ok(Vec::new());
            }
            Command::RemovePlacement { placement } => {
                self.remove(placement)?;
                return Ok(Vec::new());
            }
            Command::Dismiss { proposal, subjects } => self.dismiss(proposal, subjects)?.into(),
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
                // An issued set went out with this index, and pages are
                // frozen with it; undoing the issue is the way back.
                if let AnyId::Revision(revision) = id {
                    return Err(ModelError::IssuedRevision(revision));
                }
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
