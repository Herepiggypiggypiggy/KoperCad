//! Editing: the only way to change a project.

use std::collections::BTreeSet;

use crate::entity::{
    CabinetPlacement, Cable, CableCore, Device, DeviceKind, Dismissal, Endpoint, Junction,
    MountingPlate, Page, PageFreeze, PageKind, Part, PeOverride, Pin, PinFeed, Potential,
    PotentialKind, PotentialSet, PotentialSpec, PotentialSystem, ProjectInfo, Rail, RailKind,
    Revision, Structure, SymbolInstance, SymbolKey, SymbolRole, Terminal, Text, TextContent,
    TextPlacement, Value, Wire,
};
use crate::error::{ModelError, Result};
use crate::geom::{Placement, Point};
use crate::id::{
    AnyId, CabinetPlacementId, CableId, DeviceId, DismissalId, JunctionId, PageId, PartId,
    PotentialId, PotentialSystemId, RailId, RevisionId, StructureId, SymbolId, TerminalId, TextId,
    WireId,
};
use crate::project::{PAGE_ORDER, Project, contents_scope, panel_contents_generator};
use crate::record::{Entity, Op, Record};
use crate::tag::{Aspect, DeviceTag};

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

    /// The wires this edit added so far.
    pub(crate) fn added_wires(&self) -> Vec<WireId> {
        self.ops
            .iter()
            .filter_map(|op| match op {
                Op::Insert {
                    record: Record::Wire(w),
                } => Some(w.id),
                _ => None,
            })
            .collect()
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
        if after.id() != id {
            return Err(ModelError::IdChanged(id.into()));
        }
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
        // First, so a detached block names what the user deleted rather
        // than something that went with it.
        self.forget_block_member(id)?;
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
                (AnyId::Terminal(terminal), AnyId::Part(_)) => {
                    self.update::<Terminal>(terminal, |t| t.part = None)?;
                }
                (AnyId::Wire(wire), AnyId::Potential(_)) => {
                    self.update::<Wire>(wire, |w| w.potential = None)?;
                }
                (AnyId::Terminal(terminal), AnyId::Potential(_)) => {
                    self.update::<Terminal>(terminal, |t| t.feed = None)?;
                }
                (AnyId::PotentialSystem(system), AnyId::Potential(potential)) => {
                    self.update::<PotentialSystem>(system, |s| {
                        s.members.retain(|&m| m != potential)
                    })?;
                }
                // A source symbol keeps its other feeds; a feed symbol for
                // this potential goes with it.
                (AnyId::Symbol(symbol), AnyId::Potential(potential))
                    if !matches!(
                        self.project.symbol(symbol).map(|s| &s.role),
                        Some(SymbolRole::PotentialFeed { .. })
                    ) =>
                {
                    self.update::<SymbolInstance>(symbol, |s| {
                        s.feeds.retain(|f| f.potential != potential)
                    })?;
                }
                // The device stays where it is on the plate, off the rail.
                (AnyId::CabinetPlacement(placement), AnyId::Rail(_)) => {
                    self.update::<CabinetPlacement>(placement, |p| {
                        p.rail = None;
                        p.rail_offset = None;
                    })?;
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
            structure: Default::default(),
            frozen: None,
            properties: Default::default(),
        })
    }

    /// Moves symbols and junctions to `page` (which may be their own page),
    /// shifted by `offset`, with the wires between them and every text
    /// attached to any of them; `items` may also name free texts. A wire
    /// from a moved object to one that stays is refused
    /// ([`ModelError::WireLeavesSelection`]): move both ends or delete it.
    /// Ids stay the same, so cables, cross references and dismissals keep
    /// pointing at the moved objects.
    pub fn move_to_page(&mut self, items: &[AnyId], page: PageId, offset: Point) -> Result<()> {
        if !self.project.contains(page.into()) {
            return Err(ModelError::NotFound(page.into()));
        }
        let mut moved: BTreeSet<AnyId> = BTreeSet::new();
        let mut texts: BTreeSet<TextId> = BTreeSet::new();
        for &item in items {
            match item {
                AnyId::Symbol(_) | AnyId::Junction(_) => {
                    if !self.project.contains(item) {
                        return Err(ModelError::NotFound(item));
                    }
                    moved.insert(item);
                }
                AnyId::Text(t) if self.project.text(t).is_some() => {
                    texts.insert(t);
                }
                other => return Err(ModelError::NotFound(other)),
            }
        }
        let mut wires: BTreeSet<WireId> = BTreeSet::new();
        for &item in &moved {
            for referrer in self.project.referrers(item) {
                if let AnyId::Wire(w) = referrer {
                    wires.insert(w);
                }
            }
        }
        for &w in &wires {
            let wire = self.project.wire(w).ok_or(ModelError::NotFound(w.into()))?;
            // An open end moves with the wire.
            let stays = |end: Endpoint| {
                crate::record::endpoint_owner(end).is_some_and(|o| !moved.contains(&o))
            };
            if stays(wire.from) || stays(wire.to) {
                return Err(ModelError::WireLeavesSelection(w));
            }
        }
        let carried = moved.iter().copied().chain(wires.iter().map(|&w| w.into()));
        for id in carried.collect::<Vec<_>>() {
            for referrer in self.project.referrers(id) {
                if let AnyId::Text(t) = referrer {
                    texts.insert(t);
                }
            }
        }

        // Lift out what must change page together: texts, then wires (with
        // the cable cores and dismissals that name them), then put them
        // back on the new page around the moved symbols and junctions.
        let text_records: Vec<Text> = texts
            .iter()
            .filter_map(|&t| self.project.text(t).cloned())
            .collect();
        for text in &text_records {
            self.remove_exact(text.id.into())?;
        }
        let wire_records: Vec<Wire> = wires
            .iter()
            .filter_map(|&w| self.project.wire(w).cloned())
            .collect();
        let mut cables: Vec<Cable> = Vec::new();
        let mut dismissals: Vec<Dismissal> = Vec::new();
        for &w in &wires {
            for referrer in self.project.referrers(w) {
                match referrer {
                    AnyId::Cable(c) => {
                        if let Some(cable) = self.project.cable(c)
                            && !cables.iter().any(|k| k.id == c)
                        {
                            cables.push(cable.clone());
                        }
                    }
                    AnyId::Dismissal(d) => {
                        if let Some(dismissal) = self.project.dismissals.get(&d)
                            && !dismissals.iter().any(|k| k.id == d)
                        {
                            dismissals.push(dismissal.clone());
                        }
                    }
                    _ => {}
                }
            }
        }
        for dismissal in &dismissals {
            self.remove_exact(dismissal.id.into())?;
        }
        for cable in &cables {
            self.update::<Cable>(cable.id, |c| {
                for core in &mut c.cores {
                    if core.wire.is_some_and(|w| wires.contains(&w)) {
                        core.wire = None;
                    }
                }
            })?;
        }
        for wire in &wire_records {
            self.remove_exact(wire.id.into())?;
        }
        for &item in &moved {
            match item {
                AnyId::Symbol(s) => self.update::<SymbolInstance>(s, |s| {
                    s.page = page;
                    s.placement.origin = s.placement.origin + offset;
                })?,
                AnyId::Junction(j) => self.update::<Junction>(j, |j| {
                    j.page = page;
                    j.position = j.position + offset;
                })?,
                _ => {}
            }
        }
        for mut wire in wire_records {
            wire.page = page;
            for point in &mut wire.route {
                *point = *point + offset;
            }
            for end in [&mut wire.from, &mut wire.to] {
                if let Endpoint::Free { at } = end {
                    *at = *at + offset;
                }
            }
            self.insert_record(Record::Wire(wire))?;
        }
        for cable in cables {
            self.replace(Record::Cable(cable))?;
        }
        for dismissal in dismissals {
            self.insert_record(Record::Dismissal(dismissal))?;
        }
        for mut text in text_records {
            text.page = page;
            if text.attached_to.is_none() {
                text.placement.offset = text.placement.offset + offset;
            }
            self.insert_record(Record::Text(text))?;
        }
        Ok(())
    }

    fn remove_exact(&mut self, id: AnyId) -> Result<()> {
        let record = self.project.record(id).ok_or(ModelError::NotFound(id))?;
        self.apply(Op::Remove { record })
    }

    /// Issues a revision (DESIGN §7, Revisions): records it and freezes
    /// every drawn page (all but generated sheets, which reflow) with its
    /// index, so references in the issued set never move. Issuing the next
    /// revision re-takes every freeze. An index already issued is refused
    /// ([`ModelError::DuplicateRevision`]); `date` is `YYYY-MM-DD`.
    pub fn issue_revision(
        &mut self,
        index: impl Into<String>,
        date: impl Into<String>,
        author: impl Into<String>,
        description: impl Into<String>,
    ) -> Result<RevisionId> {
        let index = index.into();
        let (date, author, description) = (date.into(), author.into(), description.into());
        let revision = self.insert(|id| Revision {
            id,
            index: index.clone(),
            date,
            author,
            description,
            properties: Default::default(),
        })?;
        let pages: Vec<PageId> = self
            .project
            .pages()
            .filter(|p| !matches!(p.kind, PageKind::Generated { .. }))
            .map(|p| p.id)
            .collect();
        for page in pages {
            self.freeze_page(page, index.clone())?;
        }
        Ok(revision)
    }

    /// Deletes a page the user chose to delete, with everything drawn on
    /// it. A generated page's kind is turned off in the project rules
    /// ([`ProjectRules::generated_pages_off`]), so the default generated
    /// pages do not bring it back on open. Use [`Tx::remove`] for pages a
    /// tool removes on its own (a report that shrank).
    ///
    /// [`ProjectRules::generated_pages_off`]: crate::ProjectRules::generated_pages_off
    pub fn delete_page(&mut self, page: PageId) -> Result<()> {
        let p = self
            .project
            .page(page)
            .ok_or(ModelError::NotFound(page.into()))?;
        if let PageKind::Generated { generator } = &p.kind {
            let generator = generator.clone();
            self.set_info(|i| {
                i.rules.generated_pages_off.insert(generator);
            })?;
        }
        self.remove(page)
    }

    /// Freezes a page as issued with `revision`: its name, structure and
    /// every symbol's column are kept for references until
    /// [`Tx::unfreeze_page`]. Freezing again re-takes the snapshot.
    pub fn freeze_page(&mut self, page: PageId, revision: impl Into<String>) -> Result<()> {
        let revision = revision.into();
        let p = self
            .project
            .page(page)
            .ok_or(ModelError::NotFound(page.into()))?;
        let mut columns: Vec<(SymbolId, u16)> = self
            .project
            .page_contents(page)
            .symbols
            .iter()
            .filter_map(|&s| Some((s, self.project.symbol(s)?)))
            .map(|(id, s)| (id, p.format.column_at(s.placement.origin.x)))
            .collect();
        columns.sort();
        let freeze = PageFreeze {
            revision,
            name: p.name.clone(),
            structure: p.structure.clone(),
            columns,
        };
        self.update::<Page>(page, |p| p.frozen = Some(freeze))
    }

    /// Lets references to a page follow its drawing again.
    pub fn unfreeze_page(&mut self, page: PageId) -> Result<()> {
        self.update::<Page>(page, |p| p.frozen = None)
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
            part: None,
            feed: None,
            pe_override: Default::default(),
            properties: Default::default(),
        })
    }

    /// Sets how several terminals count as PE terminals: by rule, or fixed
    /// by hand either way. One undo step. Only the choice is stored; the
    /// terminal's `kind` and symbol are the caller's to change with it.
    pub fn set_terminal_pe(&mut self, terminals: &[TerminalId], mode: PeOverride) -> Result<()> {
        for &terminal in terminals {
            self.update::<Terminal>(terminal, |t| t.pe_override = mode)?;
        }
        Ok(())
    }

    /// Switches the project's drawing standard and gives the listed symbols
    /// the keys of that standard's pack, as one step. The caller works out
    /// the keys (the model does not know the packs) and reports symbols
    /// that have no counterpart; those are simply not listed. Pins stay as
    /// they are. Refused as a whole for an unknown symbol or an empty key,
    /// and, where the project info is checked, for a bad standard id
    /// ([`ProjectRules::is_standard_id`](crate::ProjectRules::is_standard_id)).
    pub fn switch_standard(
        &mut self,
        standard: &str,
        symbols: &[(SymbolId, SymbolKey)],
    ) -> Result<()> {
        if let Some((id, _)) = symbols.iter().find(|(_, key)| key.0.is_empty()) {
            return Err(ModelError::InvalidValue {
                id: (*id).into(),
                field: "symbol",
            });
        }
        self.set_info(|i| i.rules.standard = standard.to_owned())?;
        for (id, key) in symbols {
            self.update::<SymbolInstance>(*id, |s| s.symbol = key.clone())?;
        }
        Ok(())
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
            pass_through: Vec::new(),
            feeds: Vec::new(),
            properties: Default::default(),
        })
    }

    /// Moves, rotates or mirrors a placed symbol.
    pub fn move_symbol(&mut self, symbol: SymbolId, placement: Placement) -> Result<()> {
        self.update::<SymbolInstance>(symbol, |s| s.placement = placement)
    }

    /// Changes what a symbol stands for: links a contact to its coil's device
    /// (`SymbolRole::Device`), a terminal block to its terminals, and so on.
    /// This is the "cross reference" action on a symbol.
    pub fn set_role(&mut self, symbol: SymbolId, role: SymbolRole) -> Result<()> {
        self.update::<SymbolInstance>(symbol, |s| s.role = role)
    }

    /// Swaps the library symbol a placed symbol is drawn with. Pins, role and
    /// placement stay; the caller picks a key whose pins match the old one
    /// (a feed that turns into the protective earth feed, say).
    pub fn set_symbol_key(&mut self, symbol: SymbolId, key: SymbolKey) -> Result<()> {
        if key.0.is_empty() {
            return Err(ModelError::InvalidValue {
                id: symbol.into(),
                field: "symbol",
            });
        }
        self.update::<SymbolInstance>(symbol, |s| s.symbol = key)
    }

    /// Adds a junction (connection dot).
    pub fn add_junction(&mut self, page: PageId, position: Point) -> Result<JunctionId> {
        self.insert(|id| Junction { id, page, position })
    }

    /// Adds a text. Attached to a symbol, junction or wire, it moves and is
    /// deleted with it; `placement.offset` is then relative to that object.
    pub fn add_text(
        &mut self,
        page: PageId,
        content: TextContent,
        placement: TextPlacement,
        attached_to: Option<AnyId>,
    ) -> Result<TextId> {
        self.insert(|id| Text {
            id,
            page,
            attached_to,
            content,
            placement,
            height_mm: Text::DEFAULT_HEIGHT_MM,
            properties: Default::default(),
        })
    }

    /// Moves, rotates or re-anchors a text without touching what it is
    /// attached to.
    pub fn move_text(&mut self, text: TextId, placement: TextPlacement) -> Result<()> {
        self.update::<Text>(text, |t| t.placement = placement)
    }

    /// Changes what a text shows and/or its character height; `None` keeps
    /// the current value. The same rules as for a new text apply (a height
    /// must be finite and above zero), so a refused edit changes nothing.
    pub fn set_text(
        &mut self,
        text: TextId,
        content: Option<TextContent>,
        height_mm: Option<f64>,
    ) -> Result<()> {
        self.update::<Text>(text, |t| {
            if let Some(content) = content {
                t.content = content;
            }
            if let Some(height_mm) = height_mm {
                t.height_mm = height_mm;
            }
        })
    }

    /// Draws a wire between two endpoints on the same page. The page is
    /// that of the pin or junction it ends on; a wire open at both ends has
    /// none and is refused: use [`Tx::connect_on`].
    pub fn connect(
        &mut self,
        from: impl Into<Endpoint>,
        to: impl Into<Endpoint>,
    ) -> Result<WireId> {
        let (from, to) = (from.into(), to.into());
        let page = match (self.endpoint_page(from)?, self.endpoint_page(to)?) {
            (Some(page), _) | (None, Some(page)) => page,
            (None, None) => {
                return Err(ModelError::Refused {
                    what: "Wire".into(),
                    reason: "a wire open at both ends needs a page".into(),
                });
            }
        };
        self.connect_on(page, from, to)
    }

    /// Like [`Tx::connect`] on a given page, which a wire open at both ends
    /// needs. A pin or junction end must be on that page.
    pub fn connect_on(
        &mut self,
        page: PageId,
        from: impl Into<Endpoint>,
        to: impl Into<Endpoint>,
    ) -> Result<WireId> {
        let (from, to) = (from.into(), to.into());
        if !self.project.contains(page.into()) {
            return Err(ModelError::NotFound(page.into()));
        }
        let wire = self.insert(|id| Wire::new(id, page, from, to))?;
        self.detach_on_connect(from, to)?;
        Ok(wire)
    }

    /// The page of the symbol or junction an endpoint is on; `None` for an
    /// open end.
    fn endpoint_page(&self, end: Endpoint) -> Result<Option<PageId>> {
        match end {
            Endpoint::Pin { symbol, .. } => self
                .project
                .symbol(symbol)
                .map(|s| Some(s.page))
                .ok_or(ModelError::NotFound(symbol.into())),
            Endpoint::Junction { junction } => self
                .project
                .junction(junction)
                .map(|j| Some(j.page))
                .ok_or(ModelError::NotFound(junction.into())),
            Endpoint::Free { .. } => Ok(None),
        }
    }

    /// Splits a wire with a new junction at the point of the wire closest to
    /// `at`, for branching off (T-junction). The original wire keeps its id,
    /// number and cable core and now ends on the junction; a new unnumbered
    /// wire with the same properties runs on from it. A split that would land
    /// on either end of the wire is refused with [`ModelError::SplitAtEnd`].
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
        let at = closest_on_segment(at, points[segment], points[segment + 1]);
        // Splitting at an end would leave a zero-length wire.
        if at.approx_eq(start, 1e-9) || at.approx_eq(end, 1e-9) {
            return Err(ModelError::SplitAtEnd(wire));
        }
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
            tail.number = None;
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

    /// The potential named `spec.name`, created from `spec` if the project
    /// has none by that name. An existing one is reused as it is. Returns
    /// the id and whether it was created.
    pub fn ensure_potential(&mut self, spec: PotentialSpec) -> Result<(PotentialId, bool)> {
        if let Some(existing) = self.project.potentials().find(|p| p.name == spec.name) {
            return Ok((existing.id, false));
        }
        let id = self.add_potential(spec.name, spec.kind)?;
        if spec.voltage.is_some() {
            self.update::<Potential>(id, |p| p.voltage = spec.voltage)?;
        }
        Ok((id, true))
    }

    /// Sets or clears the potential a wire defines on its net.
    pub fn set_wire_potential(
        &mut self,
        wire: WireId,
        potential: Option<PotentialId>,
    ) -> Result<()> {
        self.update::<Wire>(wire, |w| w.potential = potential)
    }

    /// Assigns potentials to wires in order: `wires[i]` gets `potentials[i]`,
    /// looked up by name or created ([`Tx::ensure_potential`]). Extra
    /// potentials are ignored, so three selected wires and a
    /// [`PotentialSet`](crate::PotentialSet) of L1/L2/L3/N/PE get L1, L2, L3.
    /// More wires than potentials is refused. Returns the potentials it
    /// created.
    pub fn assign_potentials(
        &mut self,
        wires: &[WireId],
        potentials: Vec<PotentialSpec>,
    ) -> Result<Vec<PotentialId>> {
        if wires.len() > potentials.len() {
            return Err(ModelError::TooFewPotentials {
                wires: wires.len(),
                potentials: potentials.len(),
            });
        }
        let mut created = Vec::new();
        for (&wire, spec) in wires.iter().zip(potentials) {
            let (potential, new) = self.ensure_potential(spec)?;
            if new {
                created.push(potential);
            }
            self.set_wire_potential(wire, Some(potential))?;
        }
        Ok(created)
    }

    /// Sets or clears the potential a symbol's pin defines on its net (the
    /// output of a supply or transformer).
    pub fn set_pin_feed(
        &mut self,
        symbol: SymbolId,
        pin: u16,
        potential: Option<PotentialId>,
    ) -> Result<()> {
        self.update::<SymbolInstance>(symbol, |s| {
            s.feeds.retain(|f| f.pin != pin);
            if let Some(potential) = potential {
                s.feeds.push(PinFeed { pin, potential });
                s.feeds.sort();
            }
        })
    }

    /// Sets or clears the potential a terminal feeds into its net.
    pub fn set_terminal_feed(
        &mut self,
        terminal: TerminalId,
        potential: Option<PotentialId>,
    ) -> Result<()> {
        self.update::<Terminal>(terminal, |t| t.feed = potential)
    }

    /// Adds a supply system from a [`PotentialSet`], reusing potentials
    /// that already exist by name ([`Tx::ensure_potential`]). Returns the
    /// system and the potentials it created.
    ///
    /// Systems that share names share members: a 400 V 3/N/PE system and
    /// a 230 V 1/N/PE system taken from it have the same `N` and `PE`,
    /// which is right in a TN network. An isolated supply (a control
    /// transformer's secondary, a second incoming feed) passes a `prefix`
    /// so its potentials are its own: `T1-` gives `T1-L` and `T1-N`. PE is
    /// never prefixed, because an isolated secondary is still earthed to
    /// the cabinet's protective conductor. A prefix with leading or
    /// trailing spaces is refused ([`ModelError::InvalidPrefix`]).
    pub fn add_potential_system(
        &mut self,
        name: impl Into<String>,
        set: PotentialSet,
        frequency_hz: Option<f64>,
        prefix: &str,
    ) -> Result<(PotentialSystemId, Vec<PotentialId>)> {
        if prefix.trim() != prefix {
            return Err(ModelError::InvalidPrefix(prefix.to_owned()));
        }
        let mut members = Vec::new();
        let mut created = Vec::new();
        for mut spec in set.specs() {
            if spec.kind != PotentialKind::ProtectiveEarth {
                spec.name.insert_str(0, prefix);
            }
            let (potential, new) = self.ensure_potential(spec)?;
            if new {
                created.push(potential);
            }
            members.push(potential);
        }
        let name = name.into();
        let system = self.insert(|id| PotentialSystem {
            id,
            name,
            voltage: Some(set.voltage()),
            frequency_hz,
            members,
            properties: Default::default(),
        })?;
        Ok((system, created))
    }

    /// Feeds a system from a source symbol: `pins[i]` defines the system's
    /// `i`-th potential (L1, L2, L3, then N and PE). Fewer pins than
    /// members is fine; more is refused.
    pub fn feed_system(
        &mut self,
        symbol: SymbolId,
        pins: &[u16],
        system: PotentialSystemId,
    ) -> Result<()> {
        let members = self
            .project
            .potential_system(system)
            .ok_or(ModelError::NotFound(system.into()))?
            .members
            .clone();
        if let Some(i) = (1..pins.len()).find(|&i| pins[..i].contains(&pins[i])) {
            return Err(ModelError::InvalidPin {
                symbol,
                pin: pins[i],
            });
        }
        if pins.len() > members.len() {
            return Err(ModelError::TooFewPotentials {
                wires: pins.len(),
                potentials: members.len(),
            });
        }
        for (&pin, potential) in pins.iter().zip(members) {
            self.set_pin_feed(symbol, pin, Some(potential))?;
        }
        Ok(())
    }

    /// Adds a structure node: a plant section (`=A1`) or location
    /// (`+CAB1`), marked as an enclosure when it is a cabinet or box.
    pub fn add_structure(
        &mut self,
        tag: DeviceTag,
        description: impl Into<String>,
        enclosure: bool,
    ) -> Result<StructureId> {
        let description = description.into();
        self.insert(|id| Structure {
            id,
            tag,
            description,
            enclosure,
            plate: None,
            properties: Default::default(),
        })
    }

    /// Sets the mounting plate of an enclosure, or removes it with `None`
    /// (refused while rails or placed devices are on it).
    pub fn set_plate(
        &mut self,
        structure: StructureId,
        plate: Option<MountingPlate>,
    ) -> Result<()> {
        if plate.is_none()
            && self
                .project
                .referrers(structure)
                .any(|r| matches!(r, AnyId::Rail(_) | AnyId::CabinetPlacement(_)))
        {
            return Err(ModelError::Refused {
                what: "Remove mounting plate".into(),
                reason: "rails or devices are still placed on it".into(),
            });
        }
        self.update::<Structure>(structure, |s| s.plate = plate)
    }

    /// Adds a rail or duct to the plate of an enclosure.
    pub fn add_rail(
        &mut self,
        structure: StructureId,
        x: f64,
        y: f64,
        length: f64,
        kind: RailKind,
    ) -> Result<RailId> {
        self.insert(|id| Rail {
            id,
            structure,
            x,
            y,
            length,
            kind,
            vertical: false,
            width: 0.0,
            height: 0.0,
            properties: Default::default(),
        })
    }

    /// Moves a rail or duct, and the devices placed on it with it.
    pub fn move_rail(&mut self, rail: RailId, x: f64, y: f64) -> Result<()> {
        let old = self
            .project
            .rails
            .get(&rail)
            .ok_or(ModelError::NotFound(rail.into()))?;
        let (dx, dy) = (x - old.x, y - old.y);
        let on_it: Vec<CabinetPlacementId> = self
            .project
            .cabinet_placements
            .values()
            .filter(|p| p.rail == Some(rail))
            .map(|p| p.id)
            .collect();
        self.update::<Rail>(rail, |r| {
            r.x = x;
            r.y = y;
        })?;
        for id in on_it {
            self.update::<CabinetPlacement>(id, |p| {
                p.x += dx;
                p.y += dy;
            })?;
        }
        Ok(())
    }

    /// Sets the length and the cross-section of a rail or duct.
    pub fn set_rail_size(
        &mut self,
        rail: RailId,
        length: f64,
        width: f64,
        height: f64,
    ) -> Result<()> {
        self.update::<Rail>(rail, |r| {
            r.length = length;
            r.width = width;
            r.height = height;
        })
    }

    /// Places a device on the plate of an enclosure, or moves it when it
    /// already has a placement. Returns the placement and whether it is new.
    pub fn place_device(
        &mut self,
        device: DeviceId,
        structure: StructureId,
        at: Point,
        rotated: bool,
        rail: Option<(RailId, f64)>,
    ) -> Result<(CabinetPlacementId, bool)> {
        let existing = self
            .project
            .cabinet_placements
            .values()
            .find(|p| p.device == device)
            .map(|p| p.id);
        let (rail, rail_offset) = match rail {
            Some((rail, offset)) => (Some(rail), Some(offset)),
            None => (None, None),
        };
        if let Some(id) = existing {
            self.update::<CabinetPlacement>(id, |p| {
                p.structure = structure;
                p.x = at.x;
                p.y = at.y;
                p.rotated = rotated;
                p.rail = rail;
                p.rail_offset = rail_offset;
            })?;
            return Ok((id, false));
        }
        let id = self.insert(|id| CabinetPlacement {
            id,
            device,
            structure,
            x: at.x,
            y: at.y,
            rotated,
            rail,
            rail_offset,
            properties: Default::default(),
        })?;
        Ok((id, true))
    }

    /// Records that the user declined a proposal about `subjects`, or
    /// returns the existing record when it was already declined.
    pub fn dismiss(
        &mut self,
        proposal: impl Into<String>,
        mut subjects: Vec<AnyId>,
    ) -> Result<DismissalId> {
        let proposal = proposal.into();
        subjects.sort();
        subjects.dedup();
        if let Some(existing) = self.project.dismissal(&proposal, &subjects) {
            return Ok(existing.id);
        }
        self.insert(|id| Dismissal {
            id,
            proposal,
            subjects,
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

    /// Puts the pages of one group and panel in the given reading order by
    /// numbering them 10, 20, 30, ... in the `order` property. `pages` must
    /// list every page that shares the first page's group, panel and
    /// section ([`Project::sibling_pages`]), each once. Page names, and so
    /// the references that print them, do not change.
    pub fn reorder_pages(&mut self, pages: &[PageId]) -> Result<()> {
        let refuse = |reason: &str| ModelError::Refused {
            what: "Reorder".into(),
            reason: reason.into(),
        };
        let Some(&first) = pages.first() else {
            return Err(refuse("there are no pages to order"));
        };
        if !self.project.contains(first.into()) {
            return Err(ModelError::NotFound(first.into()));
        }
        let siblings = self.project.sibling_pages(first);
        let listed: BTreeSet<PageId> = pages.iter().copied().collect();
        if listed.len() != pages.len() {
            return Err(refuse("a page is listed twice"));
        }
        if let Some(page) = pages.iter().find(|p| !siblings.contains(p)) {
            if !self.project.contains((*page).into()) {
                return Err(ModelError::NotFound((*page).into()));
            }
            return Err(refuse("the pages are not all in one group and panel"));
        }
        if siblings.len() != pages.len() {
            return Err(refuse("list every page of the group and panel"));
        }
        for (n, &page) in pages.iter().enumerate() {
            let order = Value::Int(10 * (n as i64 + 1));
            let unchanged = self
                .project
                .page(page)
                .is_some_and(|p| p.properties.get(PAGE_ORDER) == Some(&order));
            if !unchanged {
                self.update::<crate::Page>(page, |p| {
                    p.properties.insert(PAGE_ORDER.into(), order);
                })?;
            }
        }
        Ok(())
    }

    /// Renames a group (function aspect) or panel (location aspect): every
    /// page, device, cable and structure node whose levels for `aspect`
    /// start with `from` gets `to` in their place, so children follow
    /// (`+CAB1` also renames `+CAB1+DOOR`) and enclosure lookups give the
    /// same answer before and after. Frozen pages keep what was printed.
    ///
    /// Renaming onto an existing name merges the two, unless that would give
    /// two devices, two cables or two structure nodes the same full tag.
    pub fn rename_level(&mut self, aspect: Aspect, from: &[String], to: &[String]) -> Result<()> {
        let refuse = |reason: &str| ModelError::Refused {
            what: "Rename".into(),
            reason: reason.into(),
        };
        if aspect == Aspect::Product {
            return Err(refuse("only groups and panels can be renamed here"));
        }
        if from.is_empty() || to.is_empty() {
            return Err(refuse("the name is empty"));
        }
        if from == to {
            return Ok(());
        }
        if to.len() > from.len() && to.starts_with(from) {
            return Err(refuse("a name cannot be renamed into its own sub-level"));
        }
        let mut probe = DeviceTag::default();
        *probe.levels_mut(aspect) = to.to_vec();
        probe.validate()?;

        let rewrite = |tag: &DeviceTag| -> Option<DeviceTag> {
            let levels = tag.levels(aspect);
            if !levels.starts_with(from) {
                return None;
            }
            let mut new = tag.clone();
            *new.levels_mut(aspect) = to.iter().chain(&levels[from.len()..]).cloned().collect();
            Some(new)
        };
        // A rewritten tag may not land on a tag that stays, or on the
        // rewrite of a different tag.
        fn clash(all: &[DeviceTag], changes: &[(usize, DeviceTag)], noun: &str) -> Result<()> {
            let moved: BTreeSet<usize> = changes.iter().map(|(i, _)| *i).collect();
            for (n, (i, new)) in changes.iter().enumerate() {
                let stays = all
                    .iter()
                    .enumerate()
                    .any(|(j, t)| !moved.contains(&j) && t == new);
                let twin = changes[..n]
                    .iter()
                    .any(|(j, other)| other == new && all[*j] != all[*i]);
                if stays || twin {
                    return Err(ModelError::Refused {
                        what: "Rename".into(),
                        reason: format!("two {noun} would both be called {new}"),
                    });
                }
            }
            Ok(())
        }

        let devices: Vec<(DeviceId, DeviceTag)> = self
            .project
            .devices()
            .map(|d| (d.id, d.tag.clone()))
            .collect();
        let device_tags: Vec<DeviceTag> = devices.iter().map(|(_, t)| t.clone()).collect();
        let device_changes: Vec<(usize, DeviceTag)> = device_tags
            .iter()
            .enumerate()
            .filter_map(|(i, t)| rewrite(t).map(|n| (i, n)))
            .collect();
        clash(&device_tags, &device_changes, "devices")?;

        let cables: Vec<(CableId, DeviceTag)> = self
            .project
            .cables()
            .map(|c| (c.id, c.tag.clone()))
            .collect();
        let cable_tags: Vec<DeviceTag> = cables.iter().map(|(_, t)| t.clone()).collect();
        let cable_changes: Vec<(usize, DeviceTag)> = cable_tags
            .iter()
            .enumerate()
            .filter_map(|(i, t)| rewrite(t).map(|n| (i, n)))
            .collect();
        clash(&cable_tags, &cable_changes, "cables")?;

        let structures: Vec<(StructureId, DeviceTag)> = self
            .project
            .structures()
            .map(|s| (s.id, s.tag.clone()))
            .collect();
        let structure_tags: Vec<DeviceTag> = structures.iter().map(|(_, t)| t.clone()).collect();
        let structure_changes: Vec<(usize, DeviceTag)> = structure_tags
            .iter()
            .enumerate()
            .filter_map(|(i, t)| rewrite(t).map(|n| (i, n)))
            .collect();
        clash(&structure_tags, &structure_changes, "structure nodes")?;

        let pages: Vec<(PageId, DeviceTag)> = self
            .project
            .pages()
            .filter_map(|p| rewrite(&p.structure).map(|n| (p.id, n)))
            .collect();
        // The contents list of a panel names the panel in its generator id.
        let generators: Vec<(PageId, String)> = self
            .project
            .pages()
            .filter_map(|p| match &p.kind {
                PageKind::Generated { generator } => contents_scope(generator)
                    .and_then(|scope| rewrite(&scope))
                    .map(|n| (p.id, panel_contents_generator(&n.location))),
                _ => None,
            })
            .collect();

        for (i, new) in device_changes {
            self.update::<Device>(devices[i].0, |d| d.tag = new)?;
        }
        for (i, new) in cable_changes {
            self.update::<Cable>(cables[i].0, |c| c.tag = new)?;
        }
        for (i, new) in structure_changes {
            self.update::<Structure>(structures[i].0, |s| s.tag = new)?;
        }
        for (id, new) in pages {
            self.update::<Page>(id, |p| p.structure = new)?;
        }
        for (id, generator) in generators {
            self.update::<Page>(id, |p| p.kind = PageKind::Generated { generator })?;
        }
        Ok(())
    }

    /// Renames a device.
    pub fn set_tag(&mut self, device: DeviceId, tag: DeviceTag) -> Result<()> {
        self.update::<Device>(device, |d| d.tag = tag)
    }
}

fn closest_on_segment(p: Point, a: Point, b: Point) -> Point {
    let ab = b - a;
    let len2 = ab.x * ab.x + ab.y * ab.y;
    if len2 == 0.0 {
        return a;
    }
    let ap = p - a;
    let t = ((ap.x * ab.x + ap.y * ab.y) / len2).clamp(0.0, 1.0);
    Point::new(a.x + ab.x * t, a.y + ab.y * t)
}

fn distance_to_segment(p: Point, a: Point, b: Point) -> f64 {
    p.distance(closest_on_segment(p, a, b))
}
