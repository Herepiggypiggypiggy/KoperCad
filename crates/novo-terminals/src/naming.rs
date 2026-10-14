//! Terminal names, numbers and the kinds they imply.

use std::collections::HashSet;

use novo_core::{DeviceId, Potential, PotentialKind, Project, TerminalKind, natural_cmp};

/// The supply potential a terminal name defines under novo-core's rule
/// `rules.potentials.from_terminal_names`: a potential of the project with
/// exactly that name (`-X4:PE` and potential `PE`). `None` when the rule is
/// off, so this never guesses more than the netlist does.
pub fn named_potential<'p>(project: &'p Project, name: &str) -> Option<&'p Potential> {
    if !project.info().rules.potentials.from_terminal_names {
        return None;
    }
    project
        .potentials()
        .filter(|p| p.kind.is_supply() && p.name == name)
        .min_by_key(|p| p.id)
}

/// The terminal kind a name implies through [`named_potential`]: PE for a
/// PE or PEN potential, N for a neutral one. Other names imply nothing.
pub fn kind_for_name(project: &Project, name: &str) -> Option<TerminalKind> {
    let kind = kind_for_potential(named_potential(project, name)?.kind);
    (kind != TerminalKind::Feedthrough).then_some(kind)
}

/// The terminal kind that suits a wire of this potential kind. A PEN
/// conductor lands on a PE terminal: it must never be disconnected or
/// fused, and it carries the protective function first (IEC 60364-5-54).
pub fn kind_for_potential(potential: PotentialKind) -> TerminalKind {
    match potential {
        PotentialKind::ProtectiveEarth | PotentialKind::Pen => TerminalKind::ProtectiveEarth,
        PotentialKind::Neutral => TerminalKind::Neutral,
        PotentialKind::Line
        | PotentialKind::DcPositive
        | PotentialKind::DcNegative
        | PotentialKind::Signal => TerminalKind::Feedthrough,
    }
}

/// Whether a wire of this potential kind may land on a terminal of this
/// kind: PE terminals take PE and PEN, N terminals take N, and every other
/// terminal takes anything but PE and PEN.
pub fn potential_fits(kind: TerminalKind, potential: PotentialKind) -> bool {
    let protective = matches!(
        potential,
        PotentialKind::ProtectiveEarth | PotentialKind::Pen
    );
    match kind {
        TerminalKind::ProtectiveEarth => protective,
        TerminalKind::Neutral => potential == PotentialKind::Neutral,
        TerminalKind::Feedthrough | TerminalKind::Fuse | TerminalKind::Disconnect => !protective,
    }
}

/// The name a new terminal gets on a wire of this potential kind: `PE`,
/// `PEN` or `N` (numbered `PE2`, ... when taken, see [`next_free_name`]),
/// else `None` for the next free number.
pub fn base_name_for_potential(potential: PotentialKind) -> Option<&'static str> {
    match potential {
        PotentialKind::ProtectiveEarth => Some("PE"),
        PotentialKind::Pen => Some("PEN"),
        PotentialKind::Neutral => Some("N"),
        _ => None,
    }
}

/// Stable lower-case key of a terminal kind, as used in property keys.
pub fn kind_key(kind: TerminalKind) -> &'static str {
    match kind {
        TerminalKind::Feedthrough => "feedthrough",
        TerminalKind::ProtectiveEarth => "protective_earth",
        TerminalKind::Neutral => "neutral",
        TerminalKind::Fuse => "fuse",
        TerminalKind::Disconnect => "disconnect",
    }
}

fn names(project: &Project, strip: DeviceId) -> HashSet<String> {
    project
        .terminals_of_strip(strip)
        .into_iter()
        .map(|t| t.name.clone())
        .collect()
}

/// The lowest positive number not yet used as a terminal name on the strip.
/// Filling gaps keeps numbers unique and continuous without user input.
pub fn next_free_number(project: &Project, strip: DeviceId) -> String {
    let used = names(project, strip);
    (1u32..)
        .map(|n| n.to_string())
        .find(|n| !used.contains(n))
        .expect("some number is free")
}

/// `base` when no terminal on the strip uses it, else `base2`, `base3`, ...
/// (`PE`, `PE2`, ...).
pub fn next_free_name(project: &Project, strip: DeviceId, base: &str) -> String {
    let used = names(project, strip);
    if !used.contains(base) {
        return base.to_owned();
    }
    (2u32..)
        .map(|n| format!("{base}{n}"))
        .find(|n| !used.contains(n))
        .expect("some name is free")
}

/// Sort position for a new terminal: after every terminal already on the
/// strip.
pub fn next_order(project: &Project, strip: DeviceId) -> i64 {
    project
        .terminals_of_strip(strip)
        .iter()
        .map(|t| t.order)
        .max()
        .map_or(10, |o| o + 10)
}

/// Compares terminal names the way the strip sorts them.
pub fn compare_names(a: &str, b: &str) -> std::cmp::Ordering {
    natural_cmp(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pen_is_mapped_like_pe() {
        assert_eq!(
            kind_for_potential(PotentialKind::Pen),
            TerminalKind::ProtectiveEarth
        );
        assert!(potential_fits(
            TerminalKind::ProtectiveEarth,
            PotentialKind::Pen
        ));
        assert!(!potential_fits(
            TerminalKind::Feedthrough,
            PotentialKind::Pen
        ));
        assert!(!potential_fits(TerminalKind::Neutral, PotentialKind::Pen));
        assert!(potential_fits(TerminalKind::Fuse, PotentialKind::Line));
        assert_eq!(base_name_for_potential(PotentialKind::Pen), Some("PEN"));
    }
}
