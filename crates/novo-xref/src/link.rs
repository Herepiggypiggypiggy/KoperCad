//! Giving a symbol its owner. References follow ownership (DESIGN.md §2):
//! a contact refers to its coil because it belongs to the coil's device, so
//! "linking" is only ever setting that owner. The editor does it when the
//! user types the tag in place or drops a contact on a coil; the "Link
//! cross reference" command (pick the partner with the next click) is the
//! fallback for symbols placed before their device existed.

use novo_core::{
    AnyId, Command, DeviceId, Project, Record, SymbolId, SymbolInstance, SymbolRole, natural_cmp,
};

use crate::function::{ContactKind, Functions};

/// Undo label of a link.
pub const LINK_LABEL: &str = "Link cross reference";

/// Why two symbols cannot be linked.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LinkError {
    #[error("symbol {0} does not exist")]
    NotFound(SymbolId),
    #[error("pick a different symbol to link to")]
    SameSymbol,
    #[error("only contacts, coils and interruption points can be linked")]
    NotLinkable(SymbolId),
    #[error("an interruption point links only to another interruption point")]
    Mixed,
    #[error("both symbols are main functions (coils); one of them must be a contact")]
    BothMain,
    #[error("neither symbol belongs to a device yet; tag one of them first")]
    NoDevice,
    #[error("give one of the interruption points a name first")]
    NoName,
    #[error("these are already linked")]
    AlreadyLinked,
}

/// True when the context menu should offer "Link cross reference" on this
/// symbol: a device function (coil or contact), an unassigned symbol, or an
/// interruption point.
pub fn can_link(project: &Project, symbol: SymbolId) -> bool {
    project.symbol(symbol).is_some_and(|s| {
        matches!(
            s.role,
            SymbolRole::Device { .. } | SymbolRole::Unassigned | SymbolRole::Interruption { .. }
        )
    })
}

fn device_has_main(project: &Project, device: DeviceId) -> bool {
    project
        .symbols_of_device(device)
        .first()
        .is_some_and(|s| matches!(s.role, SymbolRole::Device { main: true, .. }))
}

/// The edit that links `first` (the symbol right-clicked) to `partner` (the
/// symbol clicked next), as one undoable command.
///
/// - Two interruption points: `first` takes the partner's name (or the
///   partner takes `first`'s, when the partner has none).
/// - A contact and a coil, in either order: the contact joins the coil's
///   device. A contact whose old device is left with nothing drawn and no
///   data of its own is deleted with it. If the contact's connection
///   designations clash with a contact already on the device, it gets the
///   next free ones (`13-14`, then `23-24`).
/// - Two contacts: `first` joins the device of the partner (or of whichever
///   has a coil).
pub fn link_command(
    project: &Project,
    functions: &dyn Functions,
    first: SymbolId,
    partner: SymbolId,
) -> Result<Command, LinkError> {
    if first == partner {
        return Err(LinkError::SameSymbol);
    }
    let a = project.symbol(first).ok_or(LinkError::NotFound(first))?;
    let b = project
        .symbol(partner)
        .ok_or(LinkError::NotFound(partner))?;
    for s in [a, b] {
        if !can_link(project, s.id) {
            return Err(LinkError::NotLinkable(s.id));
        }
    }

    match (&a.role, &b.role) {
        (SymbolRole::Interruption { name: na }, SymbolRole::Interruption { name: nb }) => {
            return link_interruptions(a, b, na, nb);
        }
        (SymbolRole::Interruption { .. }, _) | (_, SymbolRole::Interruption { .. }) => {
            return Err(LinkError::Mixed);
        }
        _ => {}
    }

    let role = |s: &SymbolInstance| match s.role {
        SymbolRole::Device { device, main } => (Some(device), main),
        _ => (None, false),
    };
    let ((dev_a, main_a), (dev_b, main_b)) = (role(a), role(b));
    if dev_a.is_some() && dev_a == dev_b {
        return Err(LinkError::AlreadyLinked);
    }
    let has_main = |d: Option<DeviceId>| d.is_some_and(|d| device_has_main(project, d));
    // (the symbol that moves, the device it joins)
    let (aux, target) = match (main_a, main_b) {
        (true, true) => return Err(LinkError::BothMain),
        (true, false) => (b, dev_a),
        (false, true) => (a, dev_b),
        (false, false) if has_main(dev_b) => (a, dev_b),
        (false, false) if has_main(dev_a) => (b, dev_a),
        (false, false) if dev_b.is_some() => (a, dev_b),
        (false, false) => (b, dev_a),
    };
    let target = target.ok_or(LinkError::NoDevice)?;

    let role = SymbolRole::Device {
        device: target,
        main: false,
    };
    let mut moved = aux.clone();
    moved.role = role.clone();
    // A plain SetRole, unless the contact numbers had to change too.
    let mut commands = vec![
        if renumber_if_clashing(project, functions, &mut moved, target) {
            Command::Replace {
                record: Record::Symbol(moved),
            }
        } else {
            Command::SetRole {
                symbol: aux.id,
                role,
            }
        },
    ];
    if let SymbolRole::Device { device: old, .. } = aux.role
        && is_disposable(project, old, aux.id)
    {
        commands.push(Command::Remove {
            id: AnyId::Device(old),
        });
    }
    Ok(Command::Batch {
        label: LINK_LABEL.into(),
        commands,
    })
}

fn link_interruptions(
    a: &SymbolInstance,
    b: &SymbolInstance,
    na: &str,
    nb: &str,
) -> Result<Command, LinkError> {
    if na == nb {
        return if na.is_empty() {
            Err(LinkError::NoName)
        } else {
            Err(LinkError::AlreadyLinked)
        };
    }
    let (renamed, name) = if nb.is_empty() { (b, na) } else { (a, nb) };
    Ok(Command::Batch {
        label: LINK_LABEL.into(),
        commands: vec![Command::SetRole {
            symbol: renamed.id,
            role: SymbolRole::Interruption {
                name: name.to_owned(),
            },
        }],
    })
}

/// A device that only existed for the moved symbol: nothing else draws or
/// refers to it, and it carries no description, parts or properties.
fn is_disposable(project: &Project, device: DeviceId, leaving: SymbolId) -> bool {
    let Some(d) = project.device(device) else {
        return false;
    };
    d.description.is_empty()
        && d.parts.is_empty()
        && d.properties.is_empty()
        && project
            .referrers(device)
            .all(|r| r == AnyId::Symbol(leaving))
}

/// EN 50005 auxiliary contact numbers: the ordinal digit (1 to 9) followed by
/// the function digit. Normally closed contacts end in 1-2, normally open in
/// 3-4, changeovers in 1-2-4.
fn function_digits(kind: ContactKind) -> Option<&'static [char]> {
    match kind {
        ContactKind::NormallyOpen => Some(&['3', '4']),
        ContactKind::NormallyClosed => Some(&['1', '2']),
        ContactKind::Changeover => Some(&['1', '2', '4']),
        _ => None,
    }
}

/// The ordinal of a two-digit auxiliary contact number, `2` for `23`.
fn ordinal(pin: &str) -> Option<u32> {
    let mut chars = pin.chars();
    let (o, f) = (chars.next()?, chars.next()?);
    if chars.next().is_some() || !f.is_ascii_digit() {
        return None;
    }
    o.to_digit(10).filter(|&o| o > 0)
}

/// Gives `moved` the next free contact numbers when its own clash with
/// the device's; true when it changed anything.
fn renumber_if_clashing(
    project: &Project,
    functions: &dyn Functions,
    moved: &mut SymbolInstance,
    target: DeviceId,
) -> bool {
    let kind = ContactKind::classify(functions.function(&moved.symbol), &moved.symbol);
    let Some(digits) = function_digits(kind) else {
        return false;
    };
    let existing: Vec<&SymbolInstance> = project
        .symbols_of_device(target)
        .into_iter()
        .filter(|s| s.id != moved.id)
        .collect();
    let taken: Vec<&str> = existing
        .iter()
        .flat_map(|s| s.pins.iter().map(|p| p.name.as_str()))
        .collect();
    if !moved.pins.iter().any(|p| taken.contains(&p.name.as_str())) {
        return false;
    }
    // Only renumber pins that already follow the scheme.
    let mut ours: Vec<usize> = (0..moved.pins.len()).collect();
    ours.sort_by(|&x, &y| natural_cmp(&moved.pins[x].name, &moved.pins[y].name));
    if ours.len() != digits.len()
        || !ours.iter().zip(digits).all(|(&i, d)| {
            moved.pins[i].name.ends_with(*d) && ordinal(&moved.pins[i].name).is_some()
        })
    {
        return false;
    }
    let used: Vec<u32> = taken.iter().filter_map(|p| ordinal(p)).collect();
    let Some(free) = (1..=9).find(|o| !used.contains(o)) else {
        return false;
    };
    for (&i, d) in ours.iter().zip(digits) {
        moved.pins[i].name = format!("{free}{d}");
    }
    true
}
