//! Terminal names, numbers and the kinds they imply.

use novo_core::{DeviceId, PotentialKind, Project, TerminalKind, natural_cmp};

/// The terminal kind a name implies: `PE`, `PE2` and `PEN` are protective
/// earth, `N` and `N2` neutral. Case does not matter. Other names imply
/// nothing.
pub fn kind_for_name(name: &str) -> Option<TerminalKind> {
    let upper = name.trim().to_ascii_uppercase();
    let numbered = |prefix: &str| {
        upper
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.chars().all(|c| c.is_ascii_digit()))
    };
    if numbered("PEN") || numbered("PE") {
        Some(TerminalKind::ProtectiveEarth)
    } else if numbered("N") {
        Some(TerminalKind::Neutral)
    } else {
        None
    }
}

/// The potential kind a terminal name implies, so a wire on `-X4:PE` is PE
/// without asking (WORKFLOW.md J10).
pub fn potential_kind_for_name(name: &str) -> Option<PotentialKind> {
    match kind_for_name(name)? {
        TerminalKind::ProtectiveEarth => Some(PotentialKind::ProtectiveEarth),
        TerminalKind::Neutral => Some(PotentialKind::Neutral),
        _ => None,
    }
}

/// The terminal kind that suits a wire of this potential kind.
pub fn kind_for_potential(potential: PotentialKind) -> TerminalKind {
    match potential {
        PotentialKind::ProtectiveEarth => TerminalKind::ProtectiveEarth,
        PotentialKind::Neutral => TerminalKind::Neutral,
        _ => TerminalKind::Feedthrough,
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

/// Plain name of a terminal kind for screens.
pub fn kind_label(kind: TerminalKind) -> &'static str {
    match kind {
        TerminalKind::Feedthrough => "feed-through",
        TerminalKind::ProtectiveEarth => "PE",
        TerminalKind::Neutral => "N",
        TerminalKind::Fuse => "fuse",
        TerminalKind::Disconnect => "disconnect",
    }
}

fn names(project: &Project, strip: DeviceId) -> Vec<String> {
    project
        .terminals_of_strip(strip)
        .into_iter()
        .map(|t| t.name.to_ascii_uppercase())
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
    let upper = base.to_ascii_uppercase();
    if !used.contains(&upper) {
        return base.to_owned();
    }
    (2u32..)
        .map(|n| format!("{base}{n}"))
        .find(|n| !used.contains(&n.to_ascii_uppercase()))
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
    fn names_imply_kinds() {
        assert_eq!(kind_for_name("PE"), Some(TerminalKind::ProtectiveEarth));
        assert_eq!(kind_for_name("pe3"), Some(TerminalKind::ProtectiveEarth));
        assert_eq!(kind_for_name("PEN"), Some(TerminalKind::ProtectiveEarth));
        assert_eq!(kind_for_name("N"), Some(TerminalKind::Neutral));
        assert_eq!(kind_for_name("N2"), Some(TerminalKind::Neutral));
        assert_eq!(kind_for_name("1"), None);
        assert_eq!(kind_for_name("NO"), None);
        assert_eq!(kind_for_name("PEX"), None);
        assert_eq!(
            potential_kind_for_name("PE"),
            Some(PotentialKind::ProtectiveEarth)
        );
    }
}
