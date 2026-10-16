//! New terminal, cable and core rows: what the import adds beyond new
//! devices (see [`super::create`]).
//!
//! - A **Terminals** row without an Id whose strip and name no terminal has
//!   is a new terminal. The strip is a terminal strip the project has, or a
//!   new device the same sheet adds.
//! - A **Cables** row without an Id whose tag no cable has is a new cable.
//! - A **Cores** row beyond the last core of a cable (`Core No` one past the
//!   end, no gaps) or with an empty `Core No` and the cable's tag is a new
//!   core; the cores of a new cable are rows with its tag. A core's wire is
//!   given by `Wire Id` only if the wire is free; wire numbers are edited on
//!   the Wires sheet.
//!
//! Every row is its own entry in the preview, selected by default; a row
//! that cannot be used is rejected with the reasons, never half applied.

use std::collections::BTreeMap;

use novo_core::{CableId, DeviceId, DeviceKind, DeviceTag, Project, TerminalKind, Value, WireId};
use novo_parts::normalize_code;

use super::fields::{Field, Slot, parse};
use super::grid::{Cell, TableKind};
use super::plan::{ImportPlan, Rejected};

/// Where a new terminal sits.
#[derive(Debug, Clone, PartialEq)]
pub enum StripRef {
    /// A terminal strip the project has.
    Existing(DeviceId),
    /// A strip device the same import adds (by tag).
    New(DeviceTag),
}

/// A terminal the sheet adds.
#[derive(Debug, Clone)]
pub struct NewTerminal {
    /// The row in the sheet (CSV: the line).
    pub line: usize,
    pub strip: StripRef,
    pub strip_tag: DeviceTag,
    pub name: String,
    pub level: u8,
    /// `None`: after the last terminal of the strip.
    pub order: Option<i64>,
    pub jumper: Option<u32>,
    pub kind: TerminalKind,
    pub properties: Vec<(String, Value)>,
    pub selected: bool,
}

/// A cable the sheet adds. Its cores are the [`NewCore`]s with its tag.
#[derive(Debug, Clone)]
pub struct NewCable {
    pub line: usize,
    pub tag: DeviceTag,
    pub description: String,
    pub length_m: Option<f64>,
    pub properties: Vec<(String, Value)>,
    pub selected: bool,
}

/// The cable a new core goes on.
#[derive(Debug, Clone, PartialEq)]
pub enum CoreTarget {
    Existing(CableId),
    /// A cable the same import adds (by tag).
    New(DeviceTag),
}

/// A core the sheet adds.
#[derive(Debug, Clone)]
pub struct NewCore {
    pub line: usize,
    pub target: CoreTarget,
    /// The cable's tag, for display.
    pub cable_tag: DeviceTag,
    /// The 1-based position the core gets in its cable.
    pub no: usize,
    pub name: String,
    pub cross_section_mm2: Option<f64>,
    pub shield: bool,
    pub spare: bool,
    pub wire: Option<WireId>,
    pub selected: bool,
}

/// One added row, whatever its kind, for a preview list.
#[derive(Debug, Clone, PartialEq)]
pub struct AddedRow {
    pub table: TableKind,
    pub line: usize,
    /// `terminal -X1:5`, `cable -W3`, `core 2 (BN) of -W3`.
    pub label: String,
    pub selected: bool,
    /// Set when the row only comes along with another added row that is
    /// left out (a terminal on a new strip, a core of a new cable).
    pub left_out_with: Option<String>,
}

fn text_at(columns: &[Option<Field>], cells: &[Cell], slot: &Slot) -> String {
    columns
        .iter()
        .position(|f| f.as_ref().is_some_and(|f| &f.slot == slot))
        .and_then(|i| cells.get(i))
        .map(|c| c.as_text().trim().to_owned())
        .unwrap_or_default()
}

fn tag_text(text: &str) -> &str {
    text.strip_prefix('\'').unwrap_or(text)
}

fn properties(
    columns: &[Option<Field>],
    cells: &[Cell],
    errors: &mut Vec<String>,
) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    for (field, cell) in columns.iter().zip(cells) {
        let Some(field) = field else { continue };
        if let Slot::Property(key) = &field.slot {
            match parse(field, cell, None) {
                Ok(p) => out.extend(p.value().map(|v| (key.clone(), v))),
                Err(e) => errors.push(e),
            }
        }
    }
    out
}

fn field_of<'a>(columns: &'a [Option<Field>], slot: &Slot) -> Option<(&'a Field, usize)> {
    columns
        .iter()
        .enumerate()
        .find_map(|(i, f)| f.as_ref().filter(|f| &f.slot == slot).map(|f| (f, i)))
}

/// Parses the cell of `slot`, pushing the problem if there is one.
fn parsed(
    columns: &[Option<Field>],
    cells: &[Cell],
    slot: &Slot,
    errors: &mut Vec<String>,
) -> Option<super::fields::Parsed> {
    let (field, i) = field_of(columns, slot)?;
    match parse(field, cells.get(i).unwrap_or(&Cell::Empty), None) {
        Ok(p) => Some(p),
        Err(e) => {
            errors.push(e);
            None
        }
    }
}

fn number_of(p: Option<super::fields::Parsed>) -> Option<f64> {
    match p {
        Some(super::fields::Parsed::Number(n)) => n,
        _ => None,
    }
}

/// A Terminals row with no Id whose strip and name nothing has. `None`
/// when it is not that (the caller reports it as it always did).
pub(crate) fn new_terminal(
    project: &Project,
    columns: &[Option<Field>],
    cells: &[Cell],
    new_devices: &[super::create::NewDevice],
    claimed: &[NewTerminal],
) -> Option<Result<NewTerminal, Vec<String>>> {
    let strip_text = text_at(columns, cells, &Slot::TerminalStrip);
    let strip_text = tag_text(&strip_text).to_owned();
    let name = text_at(columns, cells, &Slot::TerminalName);
    if strip_text.is_empty() || name.is_empty() {
        return None;
    }
    let mut errors = Vec::new();
    let Ok(strip_tag) = DeviceTag::parse(&strip_text) else {
        return Some(Err(vec![format!("Strip: '{strip_text}' is not a tag")]));
    };
    let existing: Vec<_> = project
        .devices()
        .filter(|d| d.tag == strip_tag)
        .map(|d| (d.id, d.kind))
        .collect();
    if existing.iter().any(|(id, _)| {
        project
            .terminals_of_strip(*id)
            .iter()
            .any(|t| t.name == name)
    }) {
        return None; // matched by key, not new
    }
    let strip = match existing.as_slice() {
        [(id, DeviceKind::TerminalStrip)] => Some(StripRef::Existing(*id)),
        [(_, _)] => {
            errors.push(format!("Strip: {strip_tag} is not a terminal strip"));
            None
        }
        [] => match new_devices.iter().find(|n| n.tag == strip_tag) {
            Some(n) if n.kind == DeviceKind::TerminalStrip => {
                Some(StripRef::New(strip_tag.clone()))
            }
            Some(_) => {
                errors.push(format!(
                    "Strip: {strip_tag} is added as a device, not a terminal strip"
                ));
                None
            }
            None => {
                errors.push(format!(
                    "Strip: no terminal strip {strip_tag} in the project or the sheet"
                ));
                None
            }
        },
        _ => {
            errors.push(format!(
                "Strip: more than one device has the tag {strip_tag}"
            ));
            None
        }
    };
    // Empty Level, Order and Jumper cells take the defaults.
    let mut optional = |slot: &Slot| {
        (!text_at(columns, cells, slot).is_empty())
            .then(|| number_of(parsed(columns, cells, slot, &mut errors)))
            .flatten()
    };
    let level = optional(&Slot::TerminalLevel).map_or(1, |n| n as u8);
    let order = optional(&Slot::TerminalOrder).map(|n| n as i64);
    let jumper = optional(&Slot::TerminalJumper).map(|n| n as u32);
    if let Some(first) = claimed
        .iter()
        .find(|t| t.strip_tag == strip_tag && t.name == name && t.level == level)
    {
        errors.push(format!(
            "{strip_tag}:{name} is already added on row {}; the first row counts",
            first.line
        ));
    }
    let kind_text = text_at(columns, cells, &Slot::TerminalKind);
    let kind = match normalize_code(&kind_text).as_str() {
        "" | "feedthrough" | "terminal" => TerminalKind::Feedthrough,
        "pe" | "protectiveearth" | "earth" => TerminalKind::ProtectiveEarth,
        "n" | "neutral" => TerminalKind::Neutral,
        "fuse" => TerminalKind::Fuse,
        "disconnect" => TerminalKind::Disconnect,
        _ => {
            errors.push(format!("Kind: '{kind_text}' is not a terminal kind"));
            TerminalKind::Feedthrough
        }
    };
    let properties = properties(columns, cells, &mut errors);
    if !errors.is_empty() {
        return Some(Err(errors));
    }
    Some(Ok(NewTerminal {
        line: 0,
        strip: strip?,
        strip_tag,
        name,
        level,
        order,
        jumper,
        kind,
        properties,
        selected: true,
    }))
}

/// A Cables row with no Id whose tag no cable has.
pub(crate) fn new_cable(
    project: &Project,
    columns: &[Option<Field>],
    cells: &[Cell],
    claimed: &[NewCable],
) -> Option<Result<NewCable, Vec<String>>> {
    let text = text_at(columns, cells, &Slot::CableTag);
    let text = tag_text(&text);
    if text.is_empty() {
        return None;
    }
    let tag = DeviceTag::parse(text).ok()?;
    if project.cables().any(|c| c.tag == tag) {
        return None;
    }
    let mut errors = Vec::new();
    if tag.levels(novo_core::Aspect::Product).is_empty() {
        errors.push(format!("Tag: {tag} has no cable part (-W1)"));
    }
    if tag.validate().is_err() {
        errors.push(format!("Tag: '{text}' is not a valid tag"));
    }
    if let Some(first) = claimed.iter().find(|n| n.tag == tag) {
        errors.push(format!(
            "{tag} is already added on row {}; the first row counts",
            first.line
        ));
    }
    let length_m = number_of(parsed(columns, cells, &Slot::CableLength, &mut errors));
    let properties = properties(columns, cells, &mut errors);
    if !errors.is_empty() {
        return Some(Err(errors));
    }
    Some(Ok(NewCable {
        line: 0,
        tag,
        description: text_at(columns, cells, &Slot::CableDescription),
        length_m,
        properties,
        selected: true,
    }))
}

/// A core row waiting to be numbered: its position is only known once all
/// rows of the cable are seen.
pub(crate) struct PendingCore {
    pub core: NewCore,
    /// The `Core No` the row gave; `None` appends.
    pub explicit: Option<usize>,
    pub item: String,
}

/// Which cable a Cores row without an Id names by its Cable tag: a cable
/// the sheet adds, else the one cable of that tag.
pub(crate) fn core_target_by_tag(
    project: &Project,
    columns: &[Option<Field>],
    cells: &[Cell],
    new_cables: &[NewCable],
) -> Option<(CoreTarget, DeviceTag)> {
    let text = text_at(columns, cells, &Slot::CoreCable);
    let tag = DeviceTag::parse(tag_text(&text)).ok()?;
    if new_cables.iter().any(|c| c.tag == tag) {
        return Some((CoreTarget::New(tag.clone()), tag));
    }
    let found: Vec<_> = project.cables().filter(|c| c.tag == tag).collect();
    match found.as_slice() {
        [one] => Some((CoreTarget::Existing(one.id), tag)),
        _ => None,
    }
}

/// Reads the cells of a core row to add.
pub(crate) fn pending_core(
    columns: &[Option<Field>],
    cells: &[Cell],
    line: usize,
    target: CoreTarget,
    cable_tag: DeviceTag,
    explicit: Option<usize>,
) -> Result<PendingCore, Vec<String>> {
    let mut errors = Vec::new();
    let name = match parsed(columns, cells, &Slot::CoreName, &mut errors) {
        Some(super::fields::Parsed::Text(t)) => t,
        _ => String::new(),
    };
    if field_of(columns, &Slot::CoreName).is_none() {
        errors.push("Core: a new core needs a name".into());
    }
    let cross_section_mm2 = number_of(parsed(columns, cells, &Slot::CoreCrossSection, &mut errors));
    let flag = |slot: &Slot, errors: &mut Vec<String>| {
        matches!(
            parsed(columns, cells, slot, errors),
            Some(super::fields::Parsed::Flag(true))
        )
    };
    let shield = flag(&Slot::CoreShield, &mut errors);
    let spare = flag(&Slot::CoreSpare, &mut errors);
    let wire = number_of(parsed(columns, cells, &Slot::CoreWireId, &mut errors))
        .map(|n| WireId::from(n as u64));
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(PendingCore {
        item: format!("{cable_tag} core ({name})"),
        explicit,
        core: NewCore {
            line,
            target,
            cable_tag,
            no: 0,
            name,
            cross_section_mm2,
            shield,
            spare,
            wire,
            selected: true,
        },
    })
}

/// Numbers and checks the core rows collected from the Cores sheet, then
/// adds the usable ones to the plan. `claimed_wires` are the wires rows of
/// the sheet already give to cores.
pub(crate) fn place_cores(
    project: &Project,
    pending: Vec<PendingCore>,
    claimed_wires: &BTreeMap<WireId, (CableId, usize)>,
    plan: &mut ImportPlan,
) {
    let mut groups: Vec<(CoreTarget, Vec<PendingCore>)> = Vec::new();
    for p in pending {
        match groups.iter_mut().find(|(t, _)| *t == p.core.target) {
            Some((_, list)) => list.push(p),
            None => groups.push((p.core.target.clone(), vec![p])),
        }
    }
    let reject = |plan: &mut ImportPlan, p: &PendingCore, why: String| {
        plan.rejected.push(Rejected {
            table: TableKind::Cores,
            line: p.core.line,
            item: p.item.clone(),
            reasons: vec![why],
        });
    };
    for (target, list) in groups {
        let existing = match &target {
            CoreTarget::Existing(id) => project
                .cable(*id)
                .map(|c| c.cores.as_slice())
                .unwrap_or(&[]),
            CoreTarget::New(_) => &[],
        };
        let base = existing.len();
        // Names and wires first: a refused row must not leave a gap.
        let mut names: Vec<String> = existing.iter().map(|c| c.name.clone()).collect();
        let mut wires: Vec<WireId> = Vec::new();
        let mut usable: Vec<PendingCore> = Vec::new();
        for p in list {
            if names.contains(&p.core.name) {
                let why = format!(
                    "Core: {} already has a core called {}",
                    p.core.cable_tag, p.core.name
                );
                reject(plan, &p, why);
                continue;
            }
            if let Some(w) = p.core.wire {
                let why = if project.wire(w).is_none() {
                    Some(format!("Wire Id: wire {} does not exist", w.raw()))
                } else if project.core_of_wire(w).is_some() || claimed_wires.contains_key(&w) {
                    Some(format!(
                        "Wire Id: wire {} is already on a core; free it there first",
                        w.raw()
                    ))
                } else if wires.contains(&w) {
                    Some(format!(
                        "Wire Id: wire {} is already given to another new core",
                        w.raw()
                    ))
                } else {
                    None
                };
                if let Some(why) = why {
                    reject(plan, &p, why);
                    continue;
                }
                wires.push(w);
            }
            names.push(p.core.name.clone());
            usable.push(p);
        }
        // Rows that give a position first, in order; then the appended ones.
        let (mut explicit, appended): (Vec<_>, Vec<_>) =
            usable.into_iter().partition(|p| p.explicit.is_some());
        explicit.sort_by_key(|p| (p.explicit, p.core.line));
        let mut next = base + 1;
        let mut seen: Vec<usize> = Vec::new();
        let mut done: Vec<PendingCore> = Vec::new();
        for mut p in explicit {
            let no = p.explicit.unwrap_or(0);
            if seen.contains(&no) {
                reject(
                    plan,
                    &p,
                    format!("core {no} is already on an earlier row; the first row counts"),
                );
            } else if no != next {
                reject(
                    plan,
                    &p,
                    format!(
                        "core {no} would leave a gap: {} has {} core{} and the import adds \
                         them in order (next is {next})",
                        p.core.cable_tag,
                        next - 1,
                        if next == 2 { "" } else { "s" }
                    ),
                );
            } else {
                seen.push(no);
                p.core.no = no;
                next += 1;
                done.push(p);
            }
        }
        for mut p in appended {
            p.core.no = next;
            next += 1;
            done.push(p);
        }
        done.sort_by_key(|p| p.core.no);
        plan.new_cores.extend(done.into_iter().map(|p| p.core));
    }
}

/// Appends `core` to `cable`; a spare mark is kept by core name.
pub(crate) fn add_core(cable: &mut novo_core::Cable, core: &NewCore) {
    cable.cores.push(novo_core::CableCore {
        name: core.name.clone(),
        cross_section_mm2: core.cross_section_mm2,
        shield: core.shield,
        wire: core.wire,
    });
    if core.spare {
        super::fields::set_spare(cable, &core.name, true);
    }
}
