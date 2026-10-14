//! Editing: the only way to change a project.

use crate::entity::{
    Cable, CableCore, Device, DeviceKind, Endpoint, Junction, Page, Part, Pin, Potential,
    PotentialKind, ProjectInfo, SymbolInstance, SymbolKey, SymbolRole, Terminal, Value, Wire,
};
use crate::error::{ModelError, Result};
use crate::geom::{Placement, Point};
use crate::id::{
    AnyId, CableId, DeviceId, JunctionId, PageId, PartId, PotentialId, SymbolId, TerminalId, WireId,
};
use crate::project::Project;
use crate::record::{Entity, Op, Record};
use crate::tag::DeviceTag;

/// An open edit on a project, handed out by [`Project::transact`] and
/// [`Document::transact`](crate::Document::transact).
///
/// Every method applies its change immediately (so later steps see it) and
/// records it. If the surrounding closure returns an error, all of it is
/// rolled back. Reads during the edit go through [`Tx::project`].
pub struct Tx<'p> {
    project: &'p mut Project,
    ops: Vec<Op>,
}

impl<'p> Tx<'p> {
    pub(crate) fn new(project: &'p mut Project) -> Self {
        Tx {
            project,
            ops: Vec::new(),
        }
    }

    /// Ends the edit: keeps the ops, or rolls them back.
    pub(crate) fn finish(self, keep: bool) -> Vec<Op> {
        if keep {
            self.ops
        } else {
            self.project.rollback(&self.ops);
            Vec::new()
        }
    }

    /// The project as edited so far.
    pub fn project(&self) -> &Project {
        self.project
    }

    fn apply(&mut self, op: Op) -> Result<()> {
        self.project.apply_op(&op)?;
        self.ops.push(op);
        Ok(())
    }

    // ----- generic edits ---------------------------------------------------

    /// Creates an object with a fresh id: `tx.insert(|id| Potential { id, .. })`.
    pub fn insert<E: Entity>(&mut self, build: impl FnOnce(E::Id) -> E) -> Result<E::Id> {
        let id = E::Id::from(self.project.alloc_id());
        let record: Record = build(id).into();
        if record.id() != id.into() {
            return Err(ModelError::IdChanged(id.into()));
        }
        self.apply(Op::Insert { record })?;
        Ok(id)
    }

    /// Inserts a record that already has an id (paste, import, scripting).
    /// The id must not be in use.
    pub fn insert_record(&mut self, record: Record) -> Result<AnyId> {
        let id = record.id();
        self.apply(Op::Insert { record })?;
        Ok(id)
    }

    /// Edits one object in place. Does nothing if the edit changes nothing.
    pub fn update<E: Entity>(&mut self, id: E::Id, edit: impl FnOnce(&mut E)) -> Result<()> {
        let before = self
            .project
            .get::<E>(id)
            .ok_or(ModelError::NotFound(id.into()))?
            .clone();
        let mut after = before.clone();
        edit(&mut after);
        self.replace(after.into())
    }

    /// Replaces an object with a new version carrying the same id.
    pub fn replace(&mut self, after: Record) -> Result<()> {
        let id = after.id();
        let before = self.project.record(id).ok_or(ModelError::NotFound(id))?;
        if before == after {
            return Ok(());
        }
        self.apply(Op::Update { before, after })
    }

    /// Edits the project info.
    pub fn set_info(&mut self, edit: impl FnOnce(&mut ProjectInfo)) -> Result<()> {
        let before = self.project.info().clone();
        let mut after = before.clone();
        edit(&mut after);
        if before == after {
            return Ok(());
        }
        self.apply(Op::SetInfo { before, after })
    }

    /// Sets (`Some`) or clears (`None`) a property on any object that has
    /// properties.
    pub fn set_property(
        &mut self,
        id: impl Into<AnyId>,
        key: impl Into<String>,
        value: Option<Value>,
    ) -> Result<()> {
        let id = id.into();
        let mut record = self.project.record(id).ok_or(ModelError::NotFound(id))?;
        let props = record
            .properties_mut()
            .ok_or_else(|| ModelError::Format(format!("{id} has no properties")))?;
        let key = key.into();
        match value {
            Some(v) => props.insert(key, v),
            None => props.remove(&key),
        };
        self.replace(record)
    }

    /// Deletes an object and everything that only exists through it:
    /// deleting a page deletes what is drawn on it, a device its symbols and
    /// terminals, a symbol its wires. References that are optional are
    /// cleared instead: deleting a part unassigns it, deleting a wire frees
    /// its cable core.
    pub fn remove(&mut self, id: impl Into<AnyId>) -> Result<()> {
        let id = id.into();
        if !self.project.contains(id) {
            return Err(ModelError::NotFound(id));
        }
        let referrers: Vec<AnyId> = self.project.referrers(id).collect();
        for referrer in referrers {
            if !self.project.contains(referrer) {
                continue; // already deleted by an earlier cascade
            }
            match (referrer, id) {
                (AnyId::Device(device), AnyId::Part(part)) => {
                    self.update::<Device>(device, |d| d.parts.retain(|&p| p != part))?;
                }
                (AnyId::Cable(cable), AnyId::Part(_)) => {
                    self.update::<Cable>(cable, |c| c.part = None)?;
                }
                (AnyId::Cable(cable), AnyId::Wire(wire)) => {
                    self.update::<Cable>(cable, |c| {
                        for core in &mut c.cores {
                            if core.wire == Some(wire) {
                                core.wire = None;
                            }
                        }
                    })?;
                }
                _ => self.remove(referrer)?,
            }
        }
        let record = self.project.record(id).ok_or(ModelError::NotFound(id))?;
        self.apply(Op::Remove { record })
    }

    // ----- convenience edits -----------------------------------------------

    /// Adds a schematic page.
    pub fn add_page(&mut self, name: impl Into<String>) -> Result<PageId> {
        let name = name.into();
        self.insert(|id| Page {
            id,
            name,
            title: String::new(),
            kind: Default::default(),
            format: Default::default(),
            properties: Default::default(),
        })
    }

    /// Adds a device.
    pub fn add_device(
        &mut self,
        tag: DeviceTag,
        description: impl Into<String>,
    ) -> Result<DeviceId> {
        self.add_device_of_kind(tag, description, DeviceKind::General)
    }

    /// Adds a device of a given kind.
    pub fn add_device_of_kind(
        &mut self,
        tag: DeviceTag,
        description: impl Into<String>,
        kind: DeviceKind,
    ) -> Result<DeviceId> {
        let description = description.into();
        self.insert(|id| Device {
            id,
            tag,
            description,
            kind,
            parts: Vec::new(),
            properties: Default::default(),
        })
    }

    /// Adds a terminal strip device (`-X1`).
    pub fn add_terminal_strip(&mut self, tag: DeviceTag) -> Result<DeviceId> {
        self.add_device_of_kind(tag, "", DeviceKind::TerminalStrip)
    }

    /// Adds a terminal to a strip.
    pub fn add_terminal(&mut self, strip: DeviceId, name: impl Into<String>) -> Result<TerminalId> {
        let name = name.into();
        self.insert(|id| Terminal {
            id,
            strip,
            name,
            kind: Default::default(),
            level: 1,
            order: 0,
            jumper: None,
            properties: Default::default(),
        })
    }

    /// Places a symbol on a page.
    pub fn place_symbol(
        &mut self,
        page: PageId,
        symbol: impl Into<SymbolKey>,
        placement: Placement,
        pins: Vec<Pin>,
        role: SymbolRole,
    ) -> Result<SymbolId> {
        let symbol = symbol.into();
        self.insert(|id| SymbolInstance {
            id,
            page,
            symbol,
            placement,
            role,
            pins,
            bridges: Vec::new(),
            properties: Default::default(),
        })
    }

    /// Moves, rotates or mirrors a placed symbol.
    pub fn move_symbol(&mut self, symbol: SymbolId, placement: Placement) -> Result<()> {
        self.update::<SymbolInstance>(symbol, |s| s.placement = placement)
    }

    /// Adds a junction (connection dot).
    pub fn add_junction(&mut self, page: PageId, position: Point) -> Result<JunctionId> {
        self.insert(|id| Junction { id, page, position })
    }

    /// Draws a wire between two endpoints on the same page.
    pub fn connect(
        &mut self,
        from: impl Into<Endpoint>,
        to: impl Into<Endpoint>,
    ) -> Result<WireId> {
        let (from, to) = (from.into(), to.into());
        let page = self.endpoint_page(from)?;
        self.insert(|id| Wire::new(id, page, from, to))
    }

    fn endpoint_page(&self, end: Endpoint) -> Result<PageId> {
        match end {
            Endpoint::Pin { symbol, .. } => self
                .project
                .symbol(symbol)
                .map(|s| s.page)
                .ok_or(ModelError::NotFound(symbol.into())),
            Endpoint::Junction { junction } => self
                .project
                .junction(junction)
                .map(|j| j.page)
                .ok_or(ModelError::NotFound(junction.into())),
        }
    }

    /// Splits a wire at the point closest to `at` with a new junction, for
    /// branching off (T-junction). The original wire keeps its id, number and
    /// cable core and now ends on the junction; a copy runs on from it.
    pub fn split_wire(&mut self, wire: WireId, at: Point) -> Result<(JunctionId, WireId)> {
        let original = self
            .project
            .wire(wire)
            .ok_or(ModelError::NotFound(wire.into()))?
            .clone();
        let start = self.project.endpoint_position(original.from).unwrap_or(at);
        let end = self.project.endpoint_position(original.to).unwrap_or(at);
        let mut points = vec![start];
        points.extend(&original.route);
        points.push(end);
        let segment = (0..points.len() - 1)
            .min_by(|&a, &b| {
                let da = distance_to_segment(at, points[a], points[a + 1]);
                let db = distance_to_segment(at, points[b], points[b + 1]);
                da.total_cmp(&db)
            })
            .unwrap_or(0);
        let junction = self.add_junction(original.page, at)?;
        let mut tail = original.clone();
        self.update::<Wire>(wire, |w| {
            w.to = Endpoint::junction(junction);
            w.route.truncate(segment);
        })?;
        let second = self.insert(|id| {
            tail.id = id;
            tail.from = Endpoint::junction(junction);
            tail.route = original.route[segment..].to_vec();
            tail
        })?;
        Ok((junction, second))
    }

    /// Adds a potential.
    pub fn add_potential(
        &mut self,
        name: impl Into<String>,
        kind: PotentialKind,
    ) -> Result<PotentialId> {
        let name = name.into();
        self.insert(|id| Potential {
            id,
            name,
            kind,
            voltage: None,
            properties: Default::default(),
        })
    }

    /// Adds a cable with the given cores.
    pub fn add_cable(&mut self, tag: DeviceTag, cores: Vec<CableCore>) -> Result<CableId> {
        self.insert(|id| Cable {
            id,
            tag,
            description: String::new(),
            part: None,
            cores,
            length_m: None,
            properties: Default::default(),
        })
    }

    /// Assigns a wire to a cable core, or frees the core with `None`.
    pub fn assign_core(&mut self, cable: CableId, core: usize, wire: Option<WireId>) -> Result<()> {
        let mut record = self
            .project
            .cable(cable)
            .ok_or(ModelError::NotFound(cable.into()))?
            .clone();
        record
            .cores
            .get_mut(core)
            .ok_or(ModelError::InvalidCore { cable, core })?
            .wire = wire;
        self.replace(record.into())
    }

    /// Adds a part to the project's part list.
    pub fn add_part(
        &mut self,
        manufacturer: impl Into<String>,
        order_number: impl Into<String>,
    ) -> Result<PartId> {
        let (manufacturer, order_number) = (manufacturer.into(), order_number.into());
        self.insert(|id| Part {
            id,
            manufacturer,
            order_number,
            type_number: String::new(),
            description: String::new(),
            properties: Default::default(),
        })
    }

    /// Assigns a part to a device (once).
    pub fn assign_part(&mut self, device: DeviceId, part: PartId) -> Result<()> {
        if !self.project.contains(part.into()) {
            return Err(ModelError::NotFound(part.into()));
        }
        self.update::<Device>(device, |d| {
            if !d.parts.contains(&part) {
                d.parts.push(part);
            }
        })
    }

    /// Renames a device.
    pub fn set_tag(&mut self, device: DeviceId, tag: DeviceTag) -> Result<()> {
        self.update::<Device>(device, |d| d.tag = tag)
    }
}

fn distance_to_segment(p: Point, a: Point, b: Point) -> f64 {
    let ab = b - a;
    let len2 = ab.x * ab.x + ab.y * ab.y;
    if len2 == 0.0 {
        return p.distance(a);
    }
    let ap = p - a;
    let t = ((ap.x * ab.x + ap.y * ab.y) / len2).clamp(0.0, 1.0);
    p.distance(Point::new(a.x + ab.x * t, a.y + ab.y * t))
}
