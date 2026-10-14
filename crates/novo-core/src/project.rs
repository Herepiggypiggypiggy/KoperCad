//! The project: storage, integrity rules and read queries.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::entity::{
    Cable, Device, DeviceKind, Endpoint, Junction, Page, Part, PinRef, Potential, ProjectInfo,
    SymbolInstance, SymbolRole, Terminal, Wire,
};
use crate::error::{ModelError, Result};
use crate::geom::Point;
use crate::graph::Netlist;
use crate::id::{
    AnyId, CableId, DeviceId, EntityKind, JunctionId, PageId, PartId, PotentialId, SymbolId,
    TerminalId, WireId,
};
use crate::record::{Entity, Op, Record, Transaction, sealed::Token};
use crate::tag::DeviceTag;
use crate::tx::Tx;

/// An electrical project: the single source of truth every view reads from.
///
/// Reading is direct (`project.device(id)`, `project.wires_at(end)`, ...).
/// Changing goes through [`Project::transact`] (or a [`Document`](crate::Document)
/// when undo is wanted), which validates every primitive op, so a project can
/// never hold a dangling reference.
#[derive(Debug, Clone, Default)]
pub struct Project {
    pub(crate) info: ProjectInfo,
    pub(crate) next_id: u64,
    pub(crate) revision: u64,
    pub(crate) pages: BTreeMap<PageId, Page>,
    pub(crate) devices: BTreeMap<DeviceId, Device>,
    pub(crate) symbols: BTreeMap<SymbolId, SymbolInstance>,
    pub(crate) junctions: BTreeMap<JunctionId, Junction>,
    pub(crate) wires: BTreeMap<WireId, Wire>,
    pub(crate) potentials: BTreeMap<PotentialId, Potential>,
    pub(crate) terminals: BTreeMap<TerminalId, Terminal>,
    pub(crate) cables: BTreeMap<CableId, Cable>,
    pub(crate) parts: BTreeMap<PartId, Part>,
    index: Index,
}

/// Reverse lookups kept in step with every op.
#[derive(Debug, Clone, Default)]
struct Index {
    /// For each object, the objects that refer to it.
    referrers: HashMap<AnyId, BTreeSet<AnyId>>,
    /// The object holding each raw id, to keep ids unique across kinds.
    ids: HashMap<u64, AnyId>,
    /// Wires ending on each endpoint.
    wires_at: HashMap<Endpoint, BTreeSet<WireId>>,
}

impl Index {
    fn add(&mut self, record: &Record) {
        let id = record.id();
        self.ids.insert(id.raw(), id);
        for target in record.refs() {
            self.referrers.entry(target).or_default().insert(id);
        }
        if let Record::Wire(w) = record {
            for end in w.ends() {
                self.wires_at.entry(end).or_default().insert(w.id);
            }
        }
    }

    fn remove(&mut self, record: &Record) {
        let id = record.id();
        self.ids.remove(&id.raw());
        for target in record.refs() {
            if let Some(set) = self.referrers.get_mut(&target) {
                set.remove(&id);
                if set.is_empty() {
                    self.referrers.remove(&target);
                }
            }
        }
        if let Record::Wire(w) = record {
            for end in w.ends() {
                if let Some(set) = self.wires_at.get_mut(&end) {
                    set.remove(&w.id);
                    if set.is_empty() {
                        self.wires_at.remove(&end);
                    }
                }
            }
        }
    }
}

/// What is drawn on one page.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PageContents {
    pub symbols: Vec<SymbolId>,
    pub junctions: Vec<JunctionId>,
    pub wires: Vec<WireId>,
}

/// Compares strings so that embedded numbers sort by value: `2` before `10`.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a, b);
    loop {
        match (a.chars().next(), b.chars().next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let na = a.find(|c: char| !c.is_ascii_digit()).unwrap_or(a.len());
                let nb = b.find(|c: char| !c.is_ascii_digit()).unwrap_or(b.len());
                let (da, db) = (
                    a[..na].trim_start_matches('0'),
                    b[..nb].trim_start_matches('0'),
                );
                let ord = da.len().cmp(&db.len()).then_with(|| da.cmp(db));
                if ord != Ordering::Equal {
                    return ord;
                }
                a = &a[na..];
                b = &b[nb..];
            }
            (Some(x), Some(y)) => {
                if x != y {
                    return x.cmp(&y);
                }
                a = &a[x.len_utf8()..];
                b = &b[y.len_utf8()..];
            }
        }
    }
}

impl Project {
    /// An empty project.
    pub fn new(name: impl Into<String>) -> Self {
        Project {
            info: ProjectInfo {
                name: name.into(),
                ..ProjectInfo::default()
            },
            next_id: 1,
            ..Project::default()
        }
    }

    // ----- editing ---------------------------------------------------------

    /// Runs `edit` as one atomic edit. If `edit` returns an error, every op it
    /// applied is rolled back and the project is unchanged. On success the
    /// returned [`Transaction`] can be inverted for undo.
    ///
    /// This records no history. Interactive edits, scripts and AI edits go
    /// through [`Document`](crate::Document) so they can be undone; use this
    /// directly only where no undo applies (loaders, storage, tests).
    pub fn transact<R>(
        &mut self,
        label: impl Into<String>,
        edit: impl FnOnce(&mut Tx<'_>) -> Result<R>,
    ) -> Result<(R, Transaction)> {
        let mut tx = Tx::new(self);
        let result = edit(&mut tx);
        let ops = tx.finish(result.is_ok());
        let value = result?;
        if !ops.is_empty() {
            self.revision += 1;
        }
        Ok((
            value,
            Transaction {
                label: label.into(),
                ops,
            },
        ))
    }

    /// Applies a recorded transaction (undo, redo, replay). All or nothing.
    pub fn apply(&mut self, transaction: &Transaction) -> Result<()> {
        for (done, op) in transaction.ops.iter().enumerate() {
            if let Err(err) = self.apply_op(op) {
                self.rollback(&transaction.ops[..done]);
                return Err(err);
            }
        }
        if !transaction.is_empty() {
            self.revision += 1;
        }
        Ok(())
    }

    /// Undoes ops that were just applied, newest first.
    pub(crate) fn rollback(&mut self, applied: &[Op]) {
        for op in applied.iter().rev() {
            self.apply_op(&op.inverse())
                .expect("inverting an op that was just applied cannot fail");
        }
    }

    /// Hands out the next id. Once the counter reaches the reserved
    /// `u64::MAX` it stays there, and inserting with it is refused.
    pub(crate) fn alloc_id(&mut self) -> u64 {
        let id = self.next_id.max(1);
        self.next_id = id.saturating_add(1);
        id
    }

    /// Applies one primitive op after checking every integrity rule.
    pub(crate) fn apply_op(&mut self, op: &Op) -> Result<()> {
        match op {
            Op::Insert { record } => {
                let id = record.id();
                // 0 is never allocated and u64::MAX would overflow the counter.
                if id.raw() == 0 || id.raw() == u64::MAX {
                    return Err(ModelError::InvalidId(id.raw()));
                }
                if let Some(&existing) = self.index.ids.get(&id.raw()) {
                    return Err(ModelError::AlreadyExists(existing));
                }
                self.check(record, None)?;
                self.index.add(record);
                self.next_id = self.next_id.max(id.raw() + 1);
                self.store(record.clone());
            }
            Op::Remove { record } => {
                let id = record.id();
                let current = self.record(id).ok_or(ModelError::NotFound(id))?;
                if current != *record {
                    return Err(ModelError::Conflict(id));
                }
                if let Some(by) = self.index.referrers.get(&id) {
                    return Err(ModelError::StillReferenced {
                        id,
                        by: by.iter().copied().collect(),
                    });
                }
                self.index.remove(&current);
                self.unstore(id);
            }
            Op::Update { before, after } => {
                let id = before.id();
                if after.id() != id {
                    return Err(ModelError::IdChanged(id));
                }
                let current = self.record(id).ok_or(ModelError::NotFound(id))?;
                if current != *before {
                    return Err(ModelError::Conflict(id));
                }
                self.check(after, Some(&current))?;
                self.index.remove(&current);
                self.index.add(after);
                self.store(after.clone());
            }
            Op::SetInfo { before, after } => {
                if self.info != *before {
                    return Err(ModelError::InfoConflict);
                }
                if !crate::record::properties_finite(&after.properties) {
                    return Err(ModelError::NonFiniteInfo);
                }
                self.info = after.clone();
            }
        }
        Ok(())
    }

    fn store(&mut self, record: Record) {
        fn put<E: Entity>(p: &mut Project, e: E) {
            E::table_mut(p, Token).insert(e.id(), e);
        }
        match record {
            Record::Page(r) => put(self, r),
            Record::Device(r) => put(self, r),
            Record::Symbol(r) => put(self, r),
            Record::Junction(r) => put(self, r),
            Record::Wire(r) => put(self, r),
            Record::Potential(r) => put(self, r),
            Record::Terminal(r) => put(self, r),
            Record::Cable(r) => put(self, r),
            Record::Part(r) => put(self, r),
        }
    }

    fn unstore(&mut self, id: AnyId) {
        fn take<E: Entity>(p: &mut Project, id: E::Id) {
            E::table_mut(p, Token).remove(&id);
        }
        match id {
            AnyId::Page(id) => take::<Page>(self, id),
            AnyId::Device(id) => take::<Device>(self, id),
            AnyId::Symbol(id) => take::<SymbolInstance>(self, id),
            AnyId::Junction(id) => take::<Junction>(self, id),
            AnyId::Wire(id) => take::<Wire>(self, id),
            AnyId::Potential(id) => take::<Potential>(self, id),
            AnyId::Terminal(id) => take::<Terminal>(self, id),
            AnyId::Cable(id) => take::<Cable>(self, id),
            AnyId::Part(id) => take::<Part>(self, id),
        }
    }

    /// Checks that `record` may be stored, replacing `current` if given.
    fn check(&self, record: &Record, current: Option<&Record>) -> Result<()> {
        for target in record.refs() {
            if !self.contains(target) {
                return Err(ModelError::NotFound(target));
            }
        }
        let referrers = || {
            self.index
                .referrers
                .get(&record.id())
                .into_iter()
                .flatten()
                .copied()
        };
        if let Some(field) = record.non_finite_field() {
            return Err(ModelError::NonFinite {
                id: record.id(),
                field,
            });
        }
        match record {
            Record::Device(device) => device.tag.validate()?,
            Record::Cable(cable) => cable.tag.validate()?,
            _ => {}
        }
        match record {
            Record::Symbol(symbol) => {
                if let SymbolRole::Terminal { terminals } = &symbol.role
                    && (terminals.is_empty()
                        || ((terminals.len() > 1 || symbol.bridges.len() > 1)
                            && terminals.len() != symbol.bridges.len())
                        || terminals
                            .iter()
                            .enumerate()
                            .any(|(i, t)| terminals[..i].contains(t)))
                {
                    return Err(ModelError::TerminalLevels {
                        symbol: symbol.id,
                        terminals: terminals.len(),
                        bridges: symbol.bridges.len(),
                    });
                }
                for group in symbol.bridges.iter().chain(&symbol.pass_through) {
                    for &pin in group {
                        if usize::from(pin) >= symbol.pins.len() {
                            return Err(ModelError::InvalidPin {
                                symbol: symbol.id,
                                pin,
                            });
                        }
                    }
                }
                if current.is_some() {
                    // Wires already on this symbol must stay valid.
                    for wire in referrers().filter_map(|r| match r {
                        AnyId::Wire(w) => self.wires.get(&w),
                        _ => None,
                    }) {
                        if wire.page != symbol.page {
                            return Err(ModelError::WrongPage {
                                id: symbol.id.into(),
                                expected: wire.page,
                                found: symbol.page,
                            });
                        }
                        for end in wire.ends() {
                            if let Endpoint::Pin { symbol: s, pin } = end
                                && s == symbol.id
                                && usize::from(pin) >= symbol.pins.len()
                            {
                                return Err(ModelError::InvalidPin { symbol: s, pin });
                            }
                        }
                    }
                }
            }
            Record::Junction(junction) => {
                if let Some(wire) = referrers().find_map(|r| match r {
                    AnyId::Wire(w) => self.wires.get(&w).filter(|w| w.page != junction.page),
                    _ => None,
                }) {
                    return Err(ModelError::WrongPage {
                        id: junction.id.into(),
                        expected: wire.page,
                        found: junction.page,
                    });
                }
            }
            Record::Wire(wire) => {
                if wire.from == wire.to {
                    return Err(ModelError::SelfConnection(wire.id));
                }
                for end in wire.ends() {
                    let (id, page): (AnyId, PageId) = match end {
                        Endpoint::Pin { symbol, pin } => {
                            let s = &self.symbols[&symbol];
                            if usize::from(pin) >= s.pins.len() {
                                return Err(ModelError::InvalidPin { symbol, pin });
                            }
                            (symbol.into(), s.page)
                        }
                        Endpoint::Junction { junction } => {
                            (junction.into(), self.junctions[&junction].page)
                        }
                    };
                    if page != wire.page {
                        return Err(ModelError::WrongPage {
                            id,
                            expected: wire.page,
                            found: page,
                        });
                    }
                }
            }
            Record::Terminal(terminal) => {
                if self.devices[&terminal.strip].kind != DeviceKind::TerminalStrip {
                    return Err(ModelError::NotATerminalStrip(terminal.strip));
                }
            }
            Record::Device(device) => {
                if device.kind != DeviceKind::TerminalStrip
                    && referrers().any(|r| r.kind() == EntityKind::Terminal)
                {
                    return Err(ModelError::NotATerminalStrip(device.id));
                }
            }
            Record::Cable(cable) => {
                let mut seen = BTreeSet::new();
                for wire in cable.cores.iter().filter_map(|c| c.wire) {
                    if !seen.insert(wire) {
                        return Err(ModelError::WireAlreadyOnCore {
                            wire,
                            cable: cable.id,
                        });
                    }
                    if let Some(other) = self.cable_of_wire(wire)
                        && other != cable.id
                    {
                        return Err(ModelError::WireAlreadyOnCore { wire, cable: other });
                    }
                }
            }
            Record::Page(_) | Record::Potential(_) | Record::Part(_) => {}
        }
        Ok(())
    }

    // ----- generic reads ---------------------------------------------------

    pub fn info(&self) -> &ProjectInfo {
        &self.info
    }

    /// Increases by one with every committed change (including undo and
    /// redo). Views cache derived data, such as a [`Netlist`], against it.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The id the next new object will get.
    pub fn next_id(&self) -> u64 {
        self.next_id
    }

    /// Any object by typed id.
    pub fn get<E: Entity>(&self, id: E::Id) -> Option<&E> {
        E::table(self).get(&id)
    }

    /// All objects of one kind, in id order.
    pub fn all<E: Entity>(&self) -> impl Iterator<Item = &E> {
        E::table(self).values()
    }

    pub fn contains(&self, id: AnyId) -> bool {
        match id {
            AnyId::Page(id) => self.pages.contains_key(&id),
            AnyId::Device(id) => self.devices.contains_key(&id),
            AnyId::Symbol(id) => self.symbols.contains_key(&id),
            AnyId::Junction(id) => self.junctions.contains_key(&id),
            AnyId::Wire(id) => self.wires.contains_key(&id),
            AnyId::Potential(id) => self.potentials.contains_key(&id),
            AnyId::Terminal(id) => self.terminals.contains_key(&id),
            AnyId::Cable(id) => self.cables.contains_key(&id),
            AnyId::Part(id) => self.parts.contains_key(&id),
        }
    }

    /// A copy of any object as a [`Record`].
    pub fn record(&self, id: AnyId) -> Option<Record> {
        match id {
            AnyId::Page(id) => self.pages.get(&id).cloned().map(Record::Page),
            AnyId::Device(id) => self.devices.get(&id).cloned().map(Record::Device),
            AnyId::Symbol(id) => self.symbols.get(&id).cloned().map(Record::Symbol),
            AnyId::Junction(id) => self.junctions.get(&id).cloned().map(Record::Junction),
            AnyId::Wire(id) => self.wires.get(&id).cloned().map(Record::Wire),
            AnyId::Potential(id) => self.potentials.get(&id).cloned().map(Record::Potential),
            AnyId::Terminal(id) => self.terminals.get(&id).cloned().map(Record::Terminal),
            AnyId::Cable(id) => self.cables.get(&id).cloned().map(Record::Cable),
            AnyId::Part(id) => self.parts.get(&id).cloned().map(Record::Part),
        }
    }

    /// Every object, in an order where each record only refers to records
    /// before it (parts, potentials, pages, devices, terminals, symbols,
    /// junctions, wires, cables). Storage can save these and load them back
    /// with [`Project::from_records`].
    pub fn records(&self) -> impl Iterator<Item = Record> + '_ {
        let parts = self.parts.values().cloned().map(Record::Part);
        let potentials = self.potentials.values().cloned().map(Record::Potential);
        let pages = self.pages.values().cloned().map(Record::Page);
        let devices = self.devices.values().cloned().map(Record::Device);
        let terminals = self.terminals.values().cloned().map(Record::Terminal);
        let symbols = self.symbols.values().cloned().map(Record::Symbol);
        let junctions = self.junctions.values().cloned().map(Record::Junction);
        let wires = self.wires.values().cloned().map(Record::Wire);
        let cables = self.cables.values().cloned().map(Record::Cable);
        parts
            .chain(potentials)
            .chain(pages)
            .chain(devices)
            .chain(terminals)
            .chain(symbols)
            .chain(junctions)
            .chain(wires)
            .chain(cables)
    }

    /// Rebuilds a project from stored records, in any order, checking every
    /// integrity rule. `next_id` is raised past the largest id if needed.
    pub fn from_records(
        info: ProjectInfo,
        next_id: u64,
        records: impl IntoIterator<Item = Record>,
    ) -> Result<Project> {
        if !crate::record::properties_finite(&info.properties) {
            return Err(ModelError::NonFiniteInfo);
        }
        let mut records: Vec<Record> = records.into_iter().collect();
        records.sort_by_key(|r| (Record::rank(r.kind()), r.id()));
        let mut project = Project {
            info,
            next_id: next_id.max(1),
            ..Project::default()
        };
        for record in records {
            project.apply_op(&Op::Insert { record })?;
        }
        Ok(project)
    }

    /// Objects that refer to `id` (a page's symbols, a device's symbols, a
    /// symbol's wires, ...), in id order.
    pub fn referrers(&self, id: impl Into<AnyId>) -> impl Iterator<Item = AnyId> + '_ {
        self.index
            .referrers
            .get(&id.into())
            .into_iter()
            .flatten()
            .copied()
    }

    // ----- typed reads -----------------------------------------------------

    pub fn page(&self, id: PageId) -> Option<&Page> {
        self.pages.get(&id)
    }
    pub fn pages(&self) -> impl Iterator<Item = &Page> {
        self.pages.values()
    }
    pub fn device(&self, id: DeviceId) -> Option<&Device> {
        self.devices.get(&id)
    }
    pub fn devices(&self) -> impl Iterator<Item = &Device> {
        self.devices.values()
    }
    pub fn symbol(&self, id: SymbolId) -> Option<&SymbolInstance> {
        self.symbols.get(&id)
    }
    pub fn symbols(&self) -> impl Iterator<Item = &SymbolInstance> {
        self.symbols.values()
    }
    pub fn junction(&self, id: JunctionId) -> Option<&Junction> {
        self.junctions.get(&id)
    }
    pub fn junctions(&self) -> impl Iterator<Item = &Junction> {
        self.junctions.values()
    }
    pub fn wire(&self, id: WireId) -> Option<&Wire> {
        self.wires.get(&id)
    }
    pub fn wires(&self) -> impl Iterator<Item = &Wire> {
        self.wires.values()
    }
    pub fn potential(&self, id: PotentialId) -> Option<&Potential> {
        self.potentials.get(&id)
    }
    pub fn potentials(&self) -> impl Iterator<Item = &Potential> {
        self.potentials.values()
    }
    pub fn terminal(&self, id: TerminalId) -> Option<&Terminal> {
        self.terminals.get(&id)
    }
    pub fn terminals(&self) -> impl Iterator<Item = &Terminal> {
        self.terminals.values()
    }
    pub fn cable(&self, id: CableId) -> Option<&Cable> {
        self.cables.get(&id)
    }
    pub fn cables(&self) -> impl Iterator<Item = &Cable> {
        self.cables.values()
    }
    pub fn part(&self, id: PartId) -> Option<&Part> {
        self.parts.get(&id)
    }
    pub fn parts(&self) -> impl Iterator<Item = &Part> {
        self.parts.values()
    }

    // ----- queries ---------------------------------------------------------

    /// Pages in reading order: natural order of their names (`2` before `10`).
    pub fn pages_in_order(&self) -> Vec<&Page> {
        let mut pages: Vec<&Page> = self.pages.values().collect();
        pages.sort_by(|a, b| natural_cmp(&a.name, &b.name).then(a.id.cmp(&b.id)));
        pages
    }

    /// Everything drawn on a page.
    pub fn page_contents(&self, page: PageId) -> PageContents {
        let mut contents = PageContents::default();
        for id in self.referrers(page) {
            match id {
                AnyId::Symbol(s) => contents.symbols.push(s),
                AnyId::Junction(j) => contents.junctions.push(j),
                AnyId::Wire(w) => contents.wires.push(w),
                _ => {}
            }
        }
        contents
    }

    /// The symbols that draw a device, main function first.
    pub fn symbols_of_device(&self, device: DeviceId) -> Vec<&SymbolInstance> {
        let mut symbols: Vec<&SymbolInstance> = self
            .referrers(device)
            .filter_map(|id| match id {
                AnyId::Symbol(s) => self.symbols.get(&s),
                _ => None,
            })
            .collect();
        symbols.sort_by_key(|s| {
            (
                !matches!(s.role, SymbolRole::Device { main: true, .. }),
                s.id,
            )
        });
        symbols
    }

    /// The symbols that draw a terminal.
    pub fn symbols_of_terminal(&self, terminal: TerminalId) -> Vec<&SymbolInstance> {
        self.referrers(terminal)
            .filter_map(|id| match id {
                AnyId::Symbol(s) => self.symbols.get(&s),
                _ => None,
            })
            .collect()
    }

    /// Terminals of a strip in strip order (by `order`, then name).
    pub fn terminals_of_strip(&self, strip: DeviceId) -> Vec<&Terminal> {
        let mut terminals: Vec<&Terminal> = self
            .referrers(strip)
            .filter_map(|id| match id {
                AnyId::Terminal(t) => self.terminals.get(&t),
                _ => None,
            })
            .collect();
        terminals.sort_by(|a, b| {
            a.order
                .cmp(&b.order)
                .then_with(|| natural_cmp(&a.name, &b.name))
                .then(a.level.cmp(&b.level))
                .then(a.id.cmp(&b.id))
        });
        terminals
    }

    /// Wires ending on an endpoint.
    pub fn wires_at(&self, end: impl Into<Endpoint>) -> impl Iterator<Item = WireId> + '_ {
        self.index
            .wires_at
            .get(&end.into())
            .into_iter()
            .flatten()
            .copied()
    }

    /// Page position of an endpoint.
    pub fn endpoint_position(&self, end: Endpoint) -> Option<Point> {
        match end {
            Endpoint::Pin { symbol, pin } => self.symbols.get(&symbol)?.pin_position(pin),
            Endpoint::Junction { junction } => self.junctions.get(&junction).map(|j| j.position),
        }
    }

    /// Pins that no wire ends on.
    pub fn unconnected_pins(&self) -> Vec<PinRef> {
        let mut out = Vec::new();
        for symbol in self.symbols.values() {
            for pin in 0..symbol.pins.len() {
                let pin = u16::try_from(pin).expect("pin count fits u16");
                let end = Endpoint::pin(symbol.id, pin);
                if !self.index.wires_at.contains_key(&end) {
                    out.push(PinRef::new(symbol.id, pin));
                }
            }
        }
        out
    }

    /// Devices with exactly this tag.
    pub fn devices_by_tag(&self, tag: &DeviceTag) -> Vec<DeviceId> {
        self.devices
            .values()
            .filter(|d| &d.tag == tag)
            .map(|d| d.id)
            .collect()
    }

    /// Tags used by more than one device or cable. Empty tags are ignored.
    pub fn duplicate_tags(&self) -> Vec<(DeviceTag, Vec<AnyId>)> {
        let mut by_tag: BTreeMap<&DeviceTag, Vec<AnyId>> = BTreeMap::new();
        for d in self.devices.values() {
            by_tag.entry(&d.tag).or_default().push(d.id.into());
        }
        for c in self.cables.values() {
            by_tag.entry(&c.tag).or_default().push(c.id.into());
        }
        by_tag
            .into_iter()
            .filter(|(tag, ids)| !tag.is_empty() && ids.len() > 1)
            .map(|(tag, ids)| (tag.clone(), ids))
            .collect()
    }

    /// The next free device number for a letter code within the structure
    /// (function and location) of `structure`: one more than the highest used.
    /// `next_tag_number(&"=A1".parse()?, "K")` is 3 when `=A1-K1` and `=A1-K2` exist.
    pub fn next_tag_number(&self, structure: &DeviceTag, letter_code: &str) -> u32 {
        self.devices
            .values()
            .map(|d| &d.tag)
            .chain(self.cables.values().map(|c| &c.tag))
            .filter(|t| t.same_structure(structure) && t.letter_code() == letter_code)
            .filter_map(DeviceTag::number)
            .max()
            .map_or(1, |n| n + 1)
    }

    /// The cable whose core carries a wire, if any.
    pub fn cable_of_wire(&self, wire: WireId) -> Option<CableId> {
        self.referrers(wire).find_map(|id| match id {
            AnyId::Cable(c) => Some(c),
            _ => None,
        })
    }

    /// The cable core carrying a wire: cable and core index.
    pub fn core_of_wire(&self, wire: WireId) -> Option<(CableId, usize)> {
        let cable = self.cable_of_wire(wire)?;
        let core = self.cables[&cable]
            .cores
            .iter()
            .position(|c| c.wire == Some(wire))?;
        Some((cable, core))
    }

    /// Full designation of a terminal, e.g. `=A1-X1:3`.
    pub fn terminal_designation(&self, terminal: TerminalId) -> Option<String> {
        let t = self.terminals.get(&terminal)?;
        let strip = self.devices.get(&t.strip)?;
        Some(format!("{}:{}", strip.tag, t.name))
    }

    /// Full designation of a connection point, e.g. `-K1:13` or `-X1:3`.
    /// Pins of symbols without a device or terminal give just the pin name.
    pub fn pin_designation(&self, pin: PinRef) -> Option<String> {
        let symbol = self.symbols.get(&pin.symbol)?;
        let name = &symbol.pins.get(usize::from(pin.pin))?.name;
        Some(match &symbol.role {
            SymbolRole::Device { device, .. } => format!("{}:{}", self.devices[device].tag, name),
            SymbolRole::Terminal { .. } => {
                self.terminal_designation(symbol.terminal_of_pin(pin.pin)?)?
            }
            _ => name.clone(),
        })
    }

    /// Builds the connectivity graph of the current state.
    pub fn netlist(&self) -> Netlist {
        Netlist::build(self)
    }
}

#[cfg(test)]
mod tests {
    use super::natural_cmp;
    use std::cmp::Ordering;

    #[test]
    fn natural_order() {
        assert_eq!(natural_cmp("2", "10"), Ordering::Less);
        assert_eq!(natural_cmp("1.2", "1.10"), Ordering::Less);
        assert_eq!(natural_cmp("a10", "a9"), Ordering::Greater);
        assert_eq!(natural_cmp("007", "7"), Ordering::Equal);
        assert_eq!(natural_cmp("X", "X1"), Ordering::Less);
    }
}
