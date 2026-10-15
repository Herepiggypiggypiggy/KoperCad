//! Records, primitive operations and transactions.
//!
//! Every change to a project is a list of primitive [`Op`]s: insert, remove or
//! update one whole [`Record`], or change the project info. Each op carries
//! enough to be inverted exactly, so undo is "apply the inverse ops in reverse"
//! and can never drift from what the edit did.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::entity::{
    BlockDefinition, BlockInstance, Cable, Device, Dismissal, Junction, Page, Part, Potential,
    PotentialSystem, ProjectInfo, Properties, Revision, Structure, SymbolInstance, SymbolRole,
    Terminal, Text, Wire,
};
use crate::geom::Point;
use crate::id::{AnyId, EntityKind, PageId};
use crate::project::Project;

/// Any project object, as one value. In JSON the kind is the `record` key
/// (`{"record": "device", "id": 5, ...}`); it cannot be `kind`, which pages,
/// devices, potentials and terminals use for their own field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "record", rename_all = "snake_case")]
pub enum Record {
    Page(Page),
    Device(Device),
    Symbol(SymbolInstance),
    Junction(Junction),
    Wire(Wire),
    Potential(Potential),
    Terminal(Terminal),
    Cable(Cable),
    Part(Part),
    Text(Text),
    PotentialSystem(PotentialSystem),
    Structure(Structure),
    Dismissal(Dismissal),
    BlockDefinition(BlockDefinition),
    BlockInstance(BlockInstance),
    Revision(Revision),
}

impl Record {
    pub fn id(&self) -> AnyId {
        match self {
            Record::Page(r) => r.id.into(),
            Record::Device(r) => r.id.into(),
            Record::Symbol(r) => r.id.into(),
            Record::Junction(r) => r.id.into(),
            Record::Wire(r) => r.id.into(),
            Record::Potential(r) => r.id.into(),
            Record::Terminal(r) => r.id.into(),
            Record::Cable(r) => r.id.into(),
            Record::Part(r) => r.id.into(),
            Record::Text(r) => r.id.into(),
            Record::PotentialSystem(r) => r.id.into(),
            Record::Structure(r) => r.id.into(),
            Record::Dismissal(r) => r.id.into(),
            Record::BlockDefinition(r) => r.id.into(),
            Record::BlockInstance(r) => r.id.into(),
            Record::Revision(r) => r.id.into(),
        }
    }

    pub fn kind(&self) -> EntityKind {
        self.id().kind()
    }

    /// The page this object is drawn on, for objects that live on a page.
    pub fn page(&self) -> Option<PageId> {
        match self {
            Record::Page(r) => Some(r.id),
            Record::Symbol(r) => Some(r.page),
            Record::Junction(r) => Some(r.page),
            Record::Wire(r) => Some(r.page),
            Record::Text(r) => Some(r.page),
            _ => None,
        }
    }

    /// The ids this object refers to. Each id appears once.
    pub fn refs(&self) -> Vec<AnyId> {
        let mut out: Vec<AnyId> = Vec::new();
        match self {
            Record::Page(_) | Record::Potential(_) | Record::Part(_) => {}
            Record::Device(r) => out.extend(r.parts.iter().map(|&p| AnyId::from(p))),
            Record::Symbol(r) => {
                out.push(r.page.into());
                match &r.role {
                    SymbolRole::Device { device, .. } => out.push((*device).into()),
                    SymbolRole::Terminal { terminals } => {
                        out.extend(terminals.iter().map(|&t| AnyId::from(t)));
                    }
                    SymbolRole::PotentialFeed { potential } => out.push((*potential).into()),
                    SymbolRole::Unassigned | SymbolRole::Interruption { .. } => {}
                }
                out.extend(r.feeds.iter().map(|f| AnyId::from(f.potential)));
            }
            Record::Junction(r) => out.push(r.page.into()),
            Record::Wire(r) => {
                out.push(r.page.into());
                for end in r.ends() {
                    out.extend(endpoint_owner(end));
                }
                out.extend(r.potential.map(AnyId::from));
            }
            Record::Terminal(r) => {
                out.push(r.strip.into());
                out.extend(r.part.map(AnyId::from));
                out.extend(r.feed.map(AnyId::from));
            }
            Record::Cable(r) => {
                out.extend(r.part.map(AnyId::from));
                out.extend(r.cores.iter().filter_map(|c| c.wire.map(AnyId::from)));
            }
            Record::Text(r) => {
                out.push(r.page.into());
                out.extend(r.attached_to);
            }
            Record::PotentialSystem(r) => {
                out.extend(r.members.iter().map(|&p| AnyId::from(p)));
            }
            Record::Structure(_) => {}
            Record::Dismissal(r) => out.extend(r.subjects.iter().copied()),
            // Template members have ids of their own, not project ids.
            Record::BlockDefinition(_) => {}
            // Members are not references: deleting one detaches the
            // instance instead of deleting it (see `Tx::remove`).
            Record::BlockInstance(r) => {
                out.push(r.definition.into());
                out.push(r.page.into());
            }
            Record::Revision(_) => {}
        }
        out.sort();
        out.dedup();
        out
    }

    pub fn properties(&self) -> &Properties {
        match self {
            Record::Page(r) => &r.properties,
            Record::Device(r) => &r.properties,
            Record::Symbol(r) => &r.properties,
            Record::Junction(_) => &EMPTY_PROPERTIES,
            Record::Wire(r) => &r.properties,
            Record::Potential(r) => &r.properties,
            Record::Terminal(r) => &r.properties,
            Record::Cable(r) => &r.properties,
            Record::Part(r) => &r.properties,
            Record::Text(r) => &r.properties,
            Record::PotentialSystem(r) => &r.properties,
            Record::Structure(r) => &r.properties,
            Record::Dismissal(r) => &r.properties,
            Record::BlockDefinition(r) => &r.properties,
            Record::BlockInstance(r) => &r.properties,
            Record::Revision(r) => &r.properties,
        }
    }

    /// Mutable properties, or `None` for objects without properties.
    pub fn properties_mut(&mut self) -> Option<&mut Properties> {
        Some(match self {
            Record::Page(r) => &mut r.properties,
            Record::Device(r) => &mut r.properties,
            Record::Symbol(r) => &mut r.properties,
            Record::Junction(_) => return None,
            Record::Wire(r) => &mut r.properties,
            Record::Potential(r) => &mut r.properties,
            Record::Terminal(r) => &mut r.properties,
            Record::Cable(r) => &mut r.properties,
            Record::Part(r) => &mut r.properties,
            Record::Text(r) => &mut r.properties,
            Record::PotentialSystem(r) => &mut r.properties,
            Record::Structure(r) => &mut r.properties,
            Record::Dismissal(r) => &mut r.properties,
            Record::BlockDefinition(r) => &mut r.properties,
            Record::BlockInstance(r) => &mut r.properties,
            Record::Revision(r) => &mut r.properties,
        })
    }

    /// The first number in this record that is NaN or infinite, named by
    /// field. Such values cannot be saved (JSON and the project file have
    /// no representation for them), so the model refuses them.
    pub fn non_finite_field(&self) -> Option<&'static str> {
        fn point(p: &Point) -> bool {
            p.x.is_finite() && p.y.is_finite()
        }
        fn opt(v: Option<f64>) -> bool {
            v.is_none_or(f64::is_finite)
        }
        let bad = match self {
            Record::Symbol(r) => {
                if !point(&r.placement.origin) {
                    Some("placement")
                } else if !r.pins.iter().all(|p| point(&p.offset)) {
                    Some("pins")
                } else {
                    None
                }
            }
            Record::Junction(r) => (!point(&r.position)).then_some("position"),
            Record::Wire(r) => {
                if !r.route.iter().all(point) {
                    Some("route")
                } else if !opt(r.cross_section_mm2) {
                    Some("cross_section_mm2")
                } else {
                    None
                }
            }
            Record::Potential(r) => (!opt(r.voltage)).then_some("voltage"),
            Record::PotentialSystem(r) => {
                if !opt(r.voltage) {
                    Some("voltage")
                } else if !opt(r.frequency_hz) {
                    Some("frequency_hz")
                } else {
                    None
                }
            }
            Record::Text(r) => {
                if !point(&r.placement.offset) {
                    Some("placement")
                } else if !r.height_mm.is_finite() {
                    Some("height_mm")
                } else {
                    None
                }
            }
            Record::Cable(r) => {
                if !opt(r.length_m) {
                    Some("length_m")
                } else if !r.cores.iter().all(|c| opt(c.cross_section_mm2)) {
                    Some("cores")
                } else {
                    None
                }
            }
            Record::BlockDefinition(r) => {
                if r.members.iter().any(|m| m.non_finite_field().is_some()) {
                    Some("members")
                } else if !r.parameters.iter().all(|p| match p.default {
                    crate::entity::Value::Float(f) => f.is_finite(),
                    _ => true,
                }) {
                    Some("parameters")
                } else {
                    None
                }
            }
            Record::BlockInstance(r) => {
                if !point(&r.origin) {
                    Some("origin")
                } else if !r.values.iter().all(|(_, v)| match v {
                    crate::entity::Value::Float(f) => f.is_finite(),
                    _ => true,
                }) {
                    Some("values")
                } else {
                    None
                }
            }
            Record::Page(_)
            | Record::Device(_)
            | Record::Terminal(_)
            | Record::Part(_)
            | Record::Structure(_)
            | Record::Dismissal(_)
            | Record::Revision(_) => None,
        };
        bad.or_else(|| (!properties_finite(self.properties())).then_some("properties"))
    }

    /// Load order: a record only refers to kinds with a lower rank.
    pub(crate) fn rank(kind: EntityKind) -> u8 {
        match kind {
            EntityKind::Part => 0,
            EntityKind::Potential => 1,
            EntityKind::Page => 2,
            EntityKind::Device => 3,
            EntityKind::Terminal => 4,
            EntityKind::Symbol => 5,
            EntityKind::Junction => 6,
            EntityKind::Wire => 7,
            EntityKind::Cable => 8,
            EntityKind::Text => 9,
            EntityKind::PotentialSystem => 10,
            EntityKind::Structure => 11,
            EntityKind::Dismissal => 12,
            EntityKind::BlockDefinition => 13,
            EntityKind::BlockInstance => 14,
            EntityKind::Revision => 15,
        }
    }
}

static EMPTY_PROPERTIES: Properties = BTreeMap::new();

/// Whether every float property is finite.
pub(crate) fn properties_finite(properties: &Properties) -> bool {
    properties.values().all(|v| match v {
        crate::entity::Value::Float(f) => f.is_finite(),
        _ => true,
    })
}

/// The symbol or junction an endpoint is on; `None` for an open end.
pub(crate) fn endpoint_owner(end: crate::entity::Endpoint) -> Option<AnyId> {
    match end {
        crate::entity::Endpoint::Pin { symbol, .. } => Some(symbol.into()),
        crate::entity::Endpoint::Junction { junction } => Some(junction.into()),
        crate::entity::Endpoint::Free { .. } => None,
    }
}

/// One primitive, exactly invertible change.
// `Update` holds two records and the others one, so the size gap grows with
// the largest record. Ops are short-lived and matched by value across
// crates; boxing them would cost an allocation per op and an API change.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    Insert {
        record: Record,
    },
    Remove {
        record: Record,
    },
    Update {
        before: Record,
        after: Record,
    },
    SetInfo {
        before: ProjectInfo,
        after: ProjectInfo,
    },
}

impl Op {
    pub fn inverse(&self) -> Op {
        match self {
            Op::Insert { record } => Op::Remove {
                record: record.clone(),
            },
            Op::Remove { record } => Op::Insert {
                record: record.clone(),
            },
            Op::Update { before, after } => Op::Update {
                before: after.clone(),
                after: before.clone(),
            },
            Op::SetInfo { before, after } => Op::SetInfo {
                before: after.clone(),
                after: before.clone(),
            },
        }
    }
}

/// A committed edit: one undo step.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Transaction {
    /// What the edit was, for the undo menu ("Place symbol").
    pub label: String,
    pub ops: Vec<Op>,
}

impl Transaction {
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// The transaction that undoes this one.
    pub fn inverse(&self) -> Transaction {
        Transaction {
            label: self.label.clone(),
            ops: self.ops.iter().rev().map(Op::inverse).collect(),
        }
    }

    /// What this transaction changed, net of intermediate steps.
    pub fn changes(&self) -> ChangeSet {
        let mut set = ChangeSet::default();
        for op in &self.ops {
            set.record(op);
        }
        set
    }
}

/// What an edit changed, so views can refresh only what they need.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ChangeSet {
    pub added: BTreeSet<AnyId>,
    pub removed: BTreeSet<AnyId>,
    pub modified: BTreeSet<AnyId>,
    /// Pages whose drawing may look different.
    pub pages: BTreeSet<PageId>,
    /// Whether the project info changed.
    pub info: bool,
}

impl ChangeSet {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.removed.is_empty()
            && self.modified.is_empty()
            && self.pages.is_empty()
            && !self.info
    }

    /// Whether anything of this kind was added, removed or modified.
    pub fn touches(&self, kind: EntityKind) -> bool {
        self.added
            .iter()
            .chain(&self.removed)
            .chain(&self.modified)
            .any(|id| id.kind() == kind)
    }

    /// Folds a later change set into this one.
    pub fn merge(&mut self, later: &ChangeSet) {
        for &id in &later.added {
            if self.removed.remove(&id) {
                self.modified.insert(id);
            } else {
                self.added.insert(id);
            }
        }
        for &id in &later.removed {
            self.modified.remove(&id);
            if !self.added.remove(&id) {
                self.removed.insert(id);
            }
        }
        for &id in &later.modified {
            if !self.added.contains(&id) {
                self.modified.insert(id);
            }
        }
        self.pages.extend(&later.pages);
        self.info |= later.info;
    }

    fn record(&mut self, op: &Op) {
        let mut single = ChangeSet::default();
        match op {
            Op::Insert { record } => {
                single.added.insert(record.id());
                single.pages.extend(record.page());
            }
            Op::Remove { record } => {
                single.removed.insert(record.id());
                single.pages.extend(record.page());
            }
            Op::Update { before, after } => {
                single.modified.insert(after.id());
                single.pages.extend(before.page());
                single.pages.extend(after.page());
            }
            Op::SetInfo { .. } => single.info = true,
        }
        self.merge(&single);
    }
}

/// A kind of project object stored in its own table. Implemented for every
/// entity type; lets generic code (`project.get::<Device>(id)`,
/// `tx.update::<Wire>(id, ..)`) work over all kinds. Sealed: only the model's
/// own types implement it.
pub trait Entity: Clone + Into<Record> + sealed::Sealed + 'static {
    type Id: Copy + Ord + Into<AnyId> + From<u64> + 'static;
    fn id(&self) -> Self::Id;
    /// Unwraps a record of this kind.
    fn from_record(record: Record) -> Option<Self>;
    /// All objects of this kind, by id.
    fn table(project: &Project) -> &BTreeMap<Self::Id, Self>;
    #[doc(hidden)]
    fn table_mut(project: &mut Project, _: sealed::Token) -> &mut BTreeMap<Self::Id, Self>;
}

pub(crate) mod sealed {
    pub trait Sealed {}
    /// Only this crate can name it, so only this crate can call `table_mut`.
    pub struct Token;
}

macro_rules! impl_entity {
    ($($ty:ident => $variant:ident, $id:ty, $field:ident;)*) => {
        $(
            impl From<$ty> for Record {
                fn from(r: $ty) -> Record {
                    Record::$variant(r)
                }
            }

            impl Entity for $ty {
                type Id = $id;
                fn id(&self) -> $id {
                    self.id
                }
                fn from_record(record: Record) -> Option<Self> {
                    match record {
                        Record::$variant(r) => Some(r),
                        _ => None,
                    }
                }
                fn table(project: &Project) -> &BTreeMap<$id, Self> {
                    &project.$field
                }
                fn table_mut(project: &mut Project, _: sealed::Token) -> &mut BTreeMap<$id, Self> {
                    &mut project.$field
                }
            }

            impl sealed::Sealed for $ty {}
        )*
    };
}

impl_entity! {
    Page => Page, crate::id::PageId, pages;
    Device => Device, crate::id::DeviceId, devices;
    SymbolInstance => Symbol, crate::id::SymbolId, symbols;
    Junction => Junction, crate::id::JunctionId, junctions;
    Wire => Wire, crate::id::WireId, wires;
    Potential => Potential, crate::id::PotentialId, potentials;
    Terminal => Terminal, crate::id::TerminalId, terminals;
    Cable => Cable, crate::id::CableId, cables;
    Part => Part, crate::id::PartId, parts;
    Text => Text, crate::id::TextId, texts;
    PotentialSystem => PotentialSystem, crate::id::PotentialSystemId, potential_systems;
    Structure => Structure, crate::id::StructureId, structures;
    Dismissal => Dismissal, crate::id::DismissalId, dismissals;
    BlockDefinition => BlockDefinition, crate::id::BlockDefinitionId, block_definitions;
    BlockInstance => BlockInstance, crate::id::BlockInstanceId, block_instances;
    Revision => Revision, crate::id::RevisionId, revisions;
}
