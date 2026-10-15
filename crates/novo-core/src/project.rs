//! The project: storage, integrity rules and read queries.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::entity::{
    BlockDefinition, BlockInstance, Cable, Device, DeviceKind, Dismissal, Endpoint, Junction, Page,
    Part, PinRef, Potential, PotentialSystem, ProjectInfo, SameName, Structure, SymbolInstance,
    SymbolRole, Terminal, Text, Wire,
};
use crate::error::{ModelError, Result};
use crate::geom::Point;
use crate::graph::Netlist;
use crate::id::{
    AnyId, BlockDefinitionId, BlockInstanceId, CableId, DeviceId, DismissalId, EntityKind,
    JunctionId, PageId, PartId, PotentialId, PotentialSystemId, StructureId, SymbolId, TerminalId,
    TextId, WireId,
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
///
/// `Project::default()` has every rule off, which is what a file without
/// rules loads as. The app and scripts create projects with
/// [`Project::new`], which starts from the recommended rules.
#[derive(Debug, Clone, Default)]
pub struct Project {
    pub(crate) info: ProjectInfo,
    pub(crate) next_id: u64,
    pub(crate) revision: u64,
    pub(crate) pages: BTreeMap<PageId, Page>,
    pub(crate) devices: BTreeMap<DeviceId, Device>,
    pub(crate) symbols: BTreeMap<SymbolId, SymbolInstance>,
    pub(crate) junctions: BTreeMap<JunctionId, Junction>,
    pub(crate) texts: BTreeMap<TextId, Text>,
    pub(crate) potential_systems: BTreeMap<PotentialSystemId, PotentialSystem>,
    pub(crate) structures: BTreeMap<StructureId, Structure>,
    pub(crate) dismissals: BTreeMap<DismissalId, Dismissal>,
    pub(crate) block_definitions: BTreeMap<BlockDefinitionId, BlockDefinition>,
    pub(crate) block_instances: BTreeMap<BlockInstanceId, BlockInstance>,
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
    pub texts: Vec<TextId>,
    pub wires: Vec<WireId>,
}

/// Reading order of pages: structure, then name; see
/// [`Project::pages_in_order`].
pub(crate) fn page_cmp(a: &Page, b: &Page) -> Ordering {
    let levels = |a: &[String], b: &[String]| {
        a.iter()
            .zip(b)
            .map(|(a, b)| natural_cmp(a, b))
            .find(|o| o.is_ne())
            .unwrap_or_else(|| a.len().cmp(&b.len()))
    };
    levels(&a.structure.function, &b.structure.function)
        .then_with(|| levels(&a.structure.location, &b.structure.location))
        .then_with(|| natural_cmp(&a.name, &b.name))
        .then(a.id.cmp(&b.id))
}

/// Property key for a rated current in amperes, on a part or a device.
pub const RATED_CURRENT_A: &str = "rated_current_a";

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
    /// A new, empty project with the recommended rules
    /// ([`ProjectRules::recommended`](crate::ProjectRules::recommended)).
    pub fn new(name: impl Into<String>) -> Self {
        Project {
            info: ProjectInfo {
                name: name.into(),
                rules: crate::entity::ProjectRules::recommended(),
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
        let result = edit(&mut tx).and_then(|value| {
            tx.detach_enclosed_devices()?;
            Ok(value)
        });
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
                if after.rules.terminals.same_name == SameName::Refuse
                    && let Some(group) = self.terminal_designation_groups().first()
                {
                    return Err(ModelError::DuplicateTerminal(group[1]));
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
            Record::Text(r) => put(self, r),
            Record::PotentialSystem(r) => put(self, r),
            Record::Structure(r) => put(self, r),
            Record::Dismissal(r) => put(self, r),
            Record::BlockDefinition(r) => put(self, r),
            Record::BlockInstance(r) => put(self, r),
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
            AnyId::Text(id) => take::<Text>(self, id),
            AnyId::PotentialSystem(id) => take::<PotentialSystem>(self, id),
            AnyId::Structure(id) => take::<Structure>(self, id),
            AnyId::Dismissal(id) => take::<Dismissal>(self, id),
            AnyId::BlockDefinition(id) => take::<BlockDefinition>(self, id),
            AnyId::BlockInstance(id) => take::<BlockInstance>(self, id),
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
                let feed_pins = symbol.feeds.iter().map(|f| f.pin);
                for pin in symbol
                    .bridges
                    .iter()
                    .chain(&symbol.pass_through)
                    .flatten()
                    .copied()
                    .chain(feed_pins)
                {
                    if usize::from(pin) >= symbol.pins.len() {
                        return Err(ModelError::InvalidPin {
                            symbol: symbol.id,
                            pin,
                        });
                    }
                }
                if symbol.feeds.windows(2).any(|w| w[0].pin >= w[1].pin) {
                    // One potential per pin, kept sorted for stable files.
                    return Err(ModelError::InvalidValue {
                        id: symbol.id.into(),
                        field: "feeds",
                    });
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
                if self.info.rules.terminals.same_name == SameName::Refuse
                    && self.referrers(terminal.strip).any(|id| match id {
                        AnyId::Terminal(t) => self.terminals.get(&t).is_some_and(|other| {
                            other.id != terminal.id
                                && other.name == terminal.name
                                && other.level == terminal.level
                        }),
                        _ => false,
                    })
                {
                    return Err(ModelError::DuplicateTerminal(terminal.id));
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
            Record::Text(text) => {
                if text.height_mm <= 0.0 {
                    return Err(ModelError::InvalidValue {
                        id: text.id.into(),
                        field: "height_mm",
                    });
                }
                if let Some(target) = text.attached_to {
                    if !matches!(
                        target,
                        AnyId::Symbol(_) | AnyId::Junction(_) | AnyId::Wire(_)
                    ) {
                        return Err(ModelError::InvalidAttachment {
                            text: text.id,
                            target,
                        });
                    }
                    let page = self.record(target).and_then(|r| r.page());
                    if let Some(page) = page
                        && page != text.page
                    {
                        return Err(ModelError::WrongPage {
                            id: text.id.into(),
                            expected: page,
                            found: text.page,
                        });
                    }
                }
            }
            Record::Page(page) => {
                page.structure.validate()?;
                if !page.structure.product.is_empty() {
                    return Err(ModelError::InvalidValue {
                        id: page.id.into(),
                        field: "structure",
                    });
                }
                if let Some(freeze) = &page.frozen
                    && (freeze.revision.trim().is_empty()
                        || freeze.columns.windows(2).any(|w| w[0].0 >= w[1].0))
                {
                    return Err(ModelError::InvalidValue {
                        id: page.id.into(),
                        field: "frozen",
                    });
                }
            }
            Record::PotentialSystem(system) => {
                let members = &system.members;
                if members
                    .iter()
                    .enumerate()
                    .any(|(i, m)| members[..i].contains(m))
                {
                    return Err(ModelError::InvalidValue {
                        id: system.id.into(),
                        field: "members",
                    });
                }
            }
            Record::Structure(structure) => {
                structure.tag.validate()?;
                if !structure.tag.product.is_empty() || structure.tag.is_empty() {
                    return Err(ModelError::InvalidValue {
                        id: structure.id.into(),
                        field: "tag",
                    });
                }
                if self
                    .structures
                    .values()
                    .any(|s| s.id != structure.id && s.tag == structure.tag)
                {
                    return Err(ModelError::DuplicateStructure(structure.tag.to_string()));
                }
            }
            Record::Dismissal(dismissal) => {
                if dismissal.proposal.is_empty()
                    || dismissal.subjects.is_empty()
                    || dismissal.subjects.windows(2).any(|w| w[0] >= w[1])
                {
                    return Err(ModelError::InvalidValue {
                        id: dismissal.id.into(),
                        field: "subjects",
                    });
                }
            }
            Record::BlockDefinition(definition) => self.check_block_definition(definition)?,
            Record::BlockInstance(instance) => {
                let bad = |field| ModelError::InvalidValue {
                    id: instance.id.into(),
                    field,
                };
                let definition = self
                    .block_definitions
                    .get(&instance.definition)
                    .ok_or(ModelError::NotFound(instance.definition.into()))?;
                if instance.values.windows(2).any(|w| w[0].0 >= w[1].0)
                    || instance
                        .values
                        .iter()
                        .any(|(name, _)| !definition.parameters.iter().any(|p| &p.name == name))
                {
                    return Err(bad("values"));
                }
                if instance.members.iter().any(|m| {
                    m.overrides.windows(2).any(|w| w[0] >= w[1])
                        || !definition.members.iter().any(|t| t.id() == m.template)
                }) {
                    return Err(bad("members"));
                }
            }
            Record::Potential(_) | Record::Part(_) => {}
        }
        // Texts attached to this object must stay on its page.
        if current.is_some()
            && let Some(page) = record.page()
            && !matches!(record, Record::Page(_))
            && let Some(text) = referrers().find_map(|r| match r {
                AnyId::Text(t) => self.texts.get(&t).filter(|t| t.page != page),
                _ => None,
            })
        {
            return Err(ModelError::WrongPage {
                id: record.id(),
                expected: text.page,
                found: page,
            });
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
            AnyId::Text(id) => self.texts.contains_key(&id),
            AnyId::PotentialSystem(id) => self.potential_systems.contains_key(&id),
            AnyId::Structure(id) => self.structures.contains_key(&id),
            AnyId::Dismissal(id) => self.dismissals.contains_key(&id),
            AnyId::BlockDefinition(id) => self.block_definitions.contains_key(&id),
            AnyId::BlockInstance(id) => self.block_instances.contains_key(&id),
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
            AnyId::Text(id) => self.texts.get(&id).cloned().map(Record::Text),
            AnyId::PotentialSystem(id) => self
                .potential_systems
                .get(&id)
                .cloned()
                .map(Record::PotentialSystem),
            AnyId::Structure(id) => self.structures.get(&id).cloned().map(Record::Structure),
            AnyId::Dismissal(id) => self.dismissals.get(&id).cloned().map(Record::Dismissal),
            AnyId::BlockDefinition(id) => self
                .block_definitions
                .get(&id)
                .cloned()
                .map(Record::BlockDefinition),
            AnyId::BlockInstance(id) => self
                .block_instances
                .get(&id)
                .cloned()
                .map(Record::BlockInstance),
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
        let texts = self.texts.values().cloned().map(Record::Text);
        let systems = self
            .potential_systems
            .values()
            .cloned()
            .map(Record::PotentialSystem);
        let structures = self.structures.values().cloned().map(Record::Structure);
        let dismissals = self.dismissals.values().cloned().map(Record::Dismissal);
        let definitions = self
            .block_definitions
            .values()
            .cloned()
            .map(Record::BlockDefinition);
        let instances = self
            .block_instances
            .values()
            .cloned()
            .map(Record::BlockInstance);
        parts
            .chain(structures)
            .chain(potentials)
            .chain(pages)
            .chain(devices)
            .chain(terminals)
            .chain(symbols)
            .chain(junctions)
            .chain(wires)
            .chain(cables)
            .chain(texts)
            .chain(systems)
            .chain(dismissals)
            .chain(definitions)
            .chain(instances)
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

    /// A device's rated current in amperes: its own
    /// [`RATED_CURRENT_A`] property (an override), else the first of its
    /// parts that states one (catalog data). Checks compare it with the
    /// cross-section of the wires it protects.
    pub fn rated_current_a(&self, device: DeviceId) -> Option<f64> {
        fn amps(properties: &crate::entity::Properties) -> Option<f64> {
            match properties.get(RATED_CURRENT_A)? {
                crate::entity::Value::Int(i) => Some(*i as f64),
                crate::entity::Value::Float(f) => Some(*f),
                _ => None,
            }
        }
        let device = self.devices.get(&device)?;
        amps(&device.properties).or_else(|| {
            device
                .parts
                .iter()
                .filter_map(|p| self.parts.get(p))
                .find_map(|p| amps(&p.properties))
        })
    }
    pub fn symbol(&self, id: SymbolId) -> Option<&SymbolInstance> {
        self.symbols.get(&id)
    }
    /// Every placed symbol in ascending id order (creation order). The
    /// order is guaranteed and stable across saves; use
    /// [`Project::cross_references`] or page contents for reading order.
    pub fn symbols(&self) -> impl Iterator<Item = &SymbolInstance> {
        self.symbols.values()
    }
    pub fn junction(&self, id: JunctionId) -> Option<&Junction> {
        self.junctions.get(&id)
    }
    pub fn text(&self, id: TextId) -> Option<&Text> {
        self.texts.get(&id)
    }
    pub fn texts(&self) -> impl Iterator<Item = &Text> {
        self.texts.values()
    }
    pub fn potential_system(&self, id: PotentialSystemId) -> Option<&PotentialSystem> {
        self.potential_systems.get(&id)
    }
    pub fn potential_systems(&self) -> impl Iterator<Item = &PotentialSystem> {
        self.potential_systems.values()
    }
    fn check_block_definition(&self, definition: &BlockDefinition) -> Result<()> {
        let bad = |field| ModelError::InvalidValue {
            id: definition.id.into(),
            field,
        };
        if definition.name.trim().is_empty() {
            return Err(bad("name"));
        }
        if self
            .block_definitions
            .values()
            .any(|d| d.id != definition.id && d.name == definition.name)
        {
            return Err(ModelError::DuplicateBlock(definition.name.clone()));
        }
        let names: Vec<&str> = definition
            .parameters
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        if names
            .iter()
            .enumerate()
            .any(|(i, n)| n.trim().is_empty() || names[..i].contains(n))
        {
            return Err(bad("parameters"));
        }
        let ids: BTreeSet<AnyId> = definition.members.iter().map(Record::id).collect();
        if ids.len() != definition.members.len() {
            return Err(bad("members"));
        }
        for member in &definition.members {
            let allowed = matches!(
                member,
                Record::Device(_)
                    | Record::Symbol(_)
                    | Record::Junction(_)
                    | Record::Wire(_)
                    | Record::Text(_)
            );
            // A template is self-contained: it refers only to its own
            // members (its page id is a placeholder).
            let closed = member
                .refs()
                .into_iter()
                .all(|r| matches!(r, AnyId::Page(_)) || ids.contains(&r));
            if !allowed || !closed {
                return Err(bad("members"));
            }
        }
        Ok(())
    }

    pub fn structures(&self) -> impl Iterator<Item = &Structure> {
        self.structures.values()
    }

    /// The structure node with exactly this tag.
    pub fn structure_by_tag(&self, tag: &DeviceTag) -> Option<&Structure> {
        self.structures.values().find(|s| s.tag == *tag)
    }

    /// The enclosure a tag's location is in: the structure node marked
    /// [`Structure::enclosure`] for the tag's location or the nearest of its
    /// parents (`+CAB1` for `+CAB1+DOOR`), preferring one under the same
    /// function. `None` when the location is not a known enclosure (field).
    pub fn enclosure_of(&self, tag: &DeviceTag) -> Option<&Structure> {
        (1..=tag.location.len()).rev().find_map(|n| {
            let location = &tag.location[..n];
            let candidates = || {
                self.structures
                    .values()
                    .filter(move |s| s.enclosure && s.tag.location == location)
            };
            candidates()
                .find(|s| s.tag.function == tag.function)
                .or_else(|| candidates().find(|s| s.tag.function.is_empty()))
        })
    }

    pub fn dismissals(&self) -> impl Iterator<Item = &Dismissal> {
        self.dismissals.values()
    }

    /// The record of a declined proposal about exactly these subjects
    /// (in any order).
    pub fn dismissal(&self, proposal: &str, subjects: &[AnyId]) -> Option<&Dismissal> {
        let mut wanted = subjects.to_vec();
        wanted.sort();
        wanted.dedup();
        let first = *wanted.first()?;
        self.referrers(first).find_map(|id| match id {
            AnyId::Dismissal(d) => self
                .dismissals
                .get(&d)
                .filter(|d| d.proposal == proposal && d.subjects == wanted),
            _ => None,
        })
    }

    pub fn block_definition(&self, id: BlockDefinitionId) -> Option<&BlockDefinition> {
        self.block_definitions.get(&id)
    }
    pub fn block_definitions(&self) -> impl Iterator<Item = &BlockDefinition> {
        self.block_definitions.values()
    }
    pub fn block_definition_by_name(&self, name: &str) -> Option<&BlockDefinition> {
        self.block_definitions.values().find(|d| d.name == name)
    }
    pub fn block_instance(&self, id: BlockInstanceId) -> Option<&BlockInstance> {
        self.block_instances.get(&id)
    }
    pub fn block_instances(&self) -> impl Iterator<Item = &BlockInstance> {
        self.block_instances.values()
    }

    /// The placed block a drawn object belongs to, if any.
    pub fn block_of(&self, id: impl Into<AnyId>) -> Option<&BlockInstance> {
        let id = id.into();
        self.block_instances
            .values()
            .find(|b| b.members.iter().any(|m| m.actual == id))
    }

    /// Every parameter of a placed block with its value: the one set on
    /// the instance, else the definition's default. In definition order.
    pub fn block_values(&self, instance: BlockInstanceId) -> Vec<(String, crate::Value)> {
        let Some(instance) = self.block_instances.get(&instance) else {
            return Vec::new();
        };
        let Some(definition) = self.block_definitions.get(&instance.definition) else {
            return Vec::new();
        };
        definition
            .parameters
            .iter()
            .map(|p| {
                let value = instance.value(&p.name).unwrap_or(&p.default);
                (p.name.clone(), value.clone())
            })
            .collect()
    }

    /// Whether the user declined this proposal about these subjects.
    pub fn is_dismissed(&self, proposal: &str, subjects: &[AnyId]) -> bool {
        self.dismissal(proposal, subjects).is_some()
    }

    /// The supply system a potential belongs to, if any. Systems can share
    /// members (N and PE in a TN network); then this is the one with the
    /// lowest id, and [`Project::potential_systems`] lists them all.
    pub fn system_of(&self, potential: PotentialId) -> Option<&PotentialSystem> {
        self.referrers(potential).find_map(|id| match id {
            AnyId::PotentialSystem(s) => self.potential_systems.get(&s),
            _ => None,
        })
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

    /// Pages in reading order: by structure (function levels, then
    /// location levels, each in natural order; pages without a structure
    /// first), then by name in natural order (`2` before `10`).
    pub fn pages_in_order(&self) -> Vec<&Page> {
        let mut pages: Vec<&Page> = self.pages.values().collect();
        pages.sort_by(|a, b| page_cmp(a, b));
        pages
    }

    /// Everything drawn on a page.
    pub fn page_contents(&self, page: PageId) -> PageContents {
        let mut contents = PageContents::default();
        for id in self.referrers(page) {
            match id {
                AnyId::Symbol(s) => contents.symbols.push(s),
                AnyId::Junction(j) => contents.junctions.push(j),
                AnyId::Text(t) => contents.texts.push(t),
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

    /// Groups of two or more terminals on one strip with the same name and
    /// level (two `-X2:1` on level 1), each in strip order, groups by strip,
    /// name and level. Levels of a multi-level terminal may share a name and
    /// are never grouped. With [`SameName::Off`](crate::SameName) these are
    /// design check findings; under `Join` each group is one point, and
    /// `Refuse` keeps the list empty.
    pub fn terminal_designation_groups(&self) -> Vec<Vec<TerminalId>> {
        let mut by_name: BTreeMap<(DeviceId, &str, u8), Vec<&Terminal>> = BTreeMap::new();
        for terminal in self.terminals.values() {
            by_name
                .entry((terminal.strip, terminal.name.as_str(), terminal.level))
                .or_default()
                .push(terminal);
        }
        type Key<'a> = (DeviceId, &'a str, u8);
        let mut groups: Vec<(Key<'_>, Vec<TerminalId>)> = by_name
            .into_iter()
            .filter(|(_, ts)| ts.len() > 1)
            .map(|(key, mut ts)| {
                ts.sort_by_key(|t| (t.order, t.id));
                (key, ts.into_iter().map(|t| t.id).collect())
            })
            .collect();
        groups.sort_by(|(a, _), (b, _)| {
            a.0.cmp(&b.0)
                .then_with(|| natural_cmp(a.1, b.1))
                .then(a.2.cmp(&b.2))
        });
        groups.into_iter().map(|(_, g)| g).collect()
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

    /// The wires with one end on the given symbols and junctions and the
    /// other end elsewhere: what a move or a block capture of `items`
    /// would cut. Sorted by id.
    pub fn wires_leaving(&self, items: &[AnyId]) -> Vec<WireId> {
        let inside: BTreeSet<AnyId> = items.iter().copied().collect();
        let mut out: Vec<WireId> = inside
            .iter()
            .flat_map(|&id| self.referrers(id))
            .filter_map(|r| match r {
                AnyId::Wire(w) => self.wires.get(&w),
                _ => None,
            })
            .filter(|w| {
                let [a, b] = w.ends().map(crate::record::endpoint_owner);
                inside.contains(&a) != inside.contains(&b)
            })
            .map(|w| w.id)
            .collect();
        out.sort();
        out.dedup();
        out
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
