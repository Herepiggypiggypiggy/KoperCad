//! Placing a macro (docs/design/macros.md): copies a block definition loaded
//! from a macro file into the project when it is not there yet, places it
//! on a page and settles the wire numbers, as one undo step in the caller's
//! transaction. [`preview`] runs the same placement on a copy of the project
//! and reports what it would do, so the ghost can say it before anything
//! changes.
//!
//! Tags need no work here: `Tx::place_block` gives each device the next
//! free number for its letter code. Wire numbers in a macro are a
//! convenience of the source project, so they are cleared by default
//! ([`WireNumbers::Clear`]); the project has no wire numbering engine yet,
//! so cleared wires stay unnumbered until one numbers them. A macro can
//! keep its numbers ([`WireNumbers::Keep`]); a number that another wire
//! already carries is cleared and reported as a clash, never kept twice.

use std::collections::BTreeSet;

use novo_core::{
    AnyId, BlockDefinition, BlockDefinitionId, BlockInstanceId, DeviceTag, ModelError, PageId,
    Point, Project, Result, Tx, Value, Wire, WireId,
};

/// What happens to the wire numbers stored in a macro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WireNumbers {
    /// The macro's numbers are dropped and the placed wires are numbered by
    /// the project's numbering scheme (`novo-numbering`), continuing the
    /// project's series without touching any existing number.
    #[default]
    Auto,
    /// Placed wires keep the macro's numbers, except where the project
    /// already uses the number; those are numbered by the scheme.
    Keep,
    /// Placed wires carry no number.
    Clear,
}

/// How to place.
#[derive(Debug, Clone, Default)]
pub struct PlaceOptions {
    /// Parameter values; unset ones take the definition's defaults.
    pub values: Vec<(String, Value)>,
    pub wire_numbers: WireNumbers,
}

/// What a placement did or would do.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Report {
    /// Tags the placed devices took, in tag order.
    pub tags: Vec<DeviceTag>,
    /// Wire numbers the placed wires carry.
    pub kept_wire_numbers: Vec<String>,
    /// Number of wires whose macro number was dropped.
    pub cleared_wires: usize,
    /// Numbers the scheme gave to placed wires, in reading order.
    pub numbered_wires: Vec<String>,
    /// Macro wire numbers dropped because another wire already has them.
    pub clashes: Vec<String>,
    /// The project already had this definition, so it was not copied again.
    pub reused_definition: bool,
}

/// The result of a placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placed {
    pub definition: BlockDefinitionId,
    pub instance: BlockInstanceId,
    pub report: Report,
}

/// Places `definition` (as read from a macro file) on `page` with its
/// origin at `origin`. Use within `Document::transact` for one undo step.
///
/// A definition of the same name that is already in the project is reused
/// when its parameters and members are equal. A different one under the same
/// name is refused with `DuplicateBlock`: updating from the library is a
/// separate step.
pub fn place_macro(
    tx: &mut Tx<'_>,
    definition: BlockDefinition,
    page: PageId,
    origin: Point,
    options: &PlaceOptions,
) -> Result<Placed> {
    let existing = tx
        .project()
        .block_definition_by_name(&definition.name)
        .map(|d| {
            (
                d.id,
                d.parameters == definition.parameters && d.members == definition.members,
            )
        });
    let (id, reused_definition) = match existing {
        Some((id, true)) => (id, true),
        Some((_, false)) => return Err(ModelError::DuplicateBlock(definition.name)),
        None => (tx.adopt_block_definition(definition)?, false),
    };
    let instance = tx.place_block(id, page, origin, options.values.clone())?;
    let mut report = Report {
        reused_definition,
        ..Report::default()
    };
    settle_wire_numbers(tx, instance, options.wire_numbers, &mut report)?;
    report.tags = placed_tags(tx.project(), instance);
    Ok(Placed {
        definition: id,
        instance,
        report,
    })
}

/// What [`place_macro`] would do, computed on a copy of the project; the
/// project is not changed.
pub fn preview(
    project: &Project,
    definition: &BlockDefinition,
    page: PageId,
    origin: Point,
    options: &PlaceOptions,
) -> Result<Report> {
    let mut copy = project.clone();
    let (placed, _) = copy.transact("Preview macro", |tx| {
        place_macro(tx, definition.clone(), page, origin, options)
    })?;
    Ok(placed.report)
}

fn placed_wires(project: &Project, instance: BlockInstanceId) -> Vec<WireId> {
    project
        .block_instance(instance)
        .map(|i| {
            i.members
                .iter()
                .filter_map(|m| match m.actual {
                    AnyId::Wire(w) => Some(w),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

fn placed_tags(project: &Project, instance: BlockInstanceId) -> Vec<DeviceTag> {
    let mut tags: Vec<DeviceTag> = project
        .block_instance(instance)
        .map(|i| {
            i.members
                .iter()
                .filter_map(|m| match m.actual {
                    AnyId::Device(d) => project.device(d).map(|d| d.tag.clone()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    tags.sort();
    tags
}

/// Numbers the placed wires that have no number, with the project's scheme.
fn number_placed(tx: &mut Tx<'_>, placed: &BTreeSet<WireId>, report: &mut Report) -> Result<()> {
    let scheme = novo_numbering::Scheme::load(tx.project());
    let plan = novo_numbering::plan(
        tx.project(),
        &scheme,
        novo_numbering::Mode::Fill,
        &novo_numbering::Scope::Wires(placed.iter().copied().collect()),
    );
    plan.apply(tx)?;
    let mut numbers: Vec<String> = Vec::new();
    for g in plan.changed_groups() {
        if let Some(n) = &g.new
            && !numbers.contains(n)
        {
            numbers.push(n.clone());
        }
    }
    report.numbered_wires = numbers;
    Ok(())
}

fn settle_wire_numbers(
    tx: &mut Tx<'_>,
    instance: BlockInstanceId,
    mode: WireNumbers,
    report: &mut Report,
) -> Result<()> {
    let wires = placed_wires(tx.project(), instance);
    let own: BTreeSet<WireId> = wires.iter().copied().collect();
    let used: BTreeSet<String> = tx
        .project()
        .wires()
        .filter(|w| !own.contains(&w.id))
        .filter_map(|w| w.number.clone())
        .collect();
    for wire in wires {
        let number = tx.project().wire(wire).and_then(|w| w.number.clone());
        let Some(number) = number else { continue };
        let keep = mode == WireNumbers::Keep && !used.contains(&number);
        if keep {
            report.kept_wire_numbers.push(number);
            continue;
        }
        if mode == WireNumbers::Keep {
            report.clashes.push(number);
        }
        report.cleared_wires += 1;
        tx.update::<Wire>(wire, |w| w.number = None)?;
    }
    report.kept_wire_numbers.sort();
    report.clashes.sort();
    if mode != WireNumbers::Clear {
        number_placed(tx, &own, report)?;
    }
    Ok(())
}
