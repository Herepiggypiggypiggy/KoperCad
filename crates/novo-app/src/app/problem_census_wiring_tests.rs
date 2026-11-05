//! Problem triangles census, wiring area (docs/design/problem-triangles.md
//! section 12). Counts the ways these files still say "wrong" in words: a
//! red or yellow toast, a banner, or a sentence coloured with the warning or
//! error colour. The ceiling only goes down; the end state is zero (lane T15).
//! This file is the wiring area's own, so no other lane edits its number.

/// The files of the wiring area. `terminals.rs` is shared with the terminals
/// lane (T8) and counted there; its Accepts column has no sentence left.
const FILES: [(&str, &str); 6] = [
    ("view3d_wire_it.rs", include_str!("view3d_wire_it.rs")),
    ("wire_it_export.rs", include_str!("wire_it_export.rs")),
    ("wire_production.rs", include_str!("wire_production.rs")),
    ("wire_panel.rs", include_str!("wire_panel.rs")),
    ("workshop.rs", include_str!("workshop.rs")),
    ("flags_wiring.rs", include_str!("flags_wiring.rs")),
];

/// Sites left. Was 15 on main 4ac0057 (sites/wiring.md): 8 toasts that were
/// failures or empty exports became job flags and 1 wire panel toast became a
/// field flag. What is left is the Wire-it strip's own coloured text, owned by
/// the routing lane (T4), and the information toasts.
const CEILING: usize = 5;

fn count(source: &str) -> usize {
    let production = source.split("#[cfg(test)]\nmod ").next().unwrap_or(source);
    production
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .filter(|l| {
            [
                "Level::Error",
                "Level::Warning",
                "banner(",
                ".color(p.warning)",
                ".color(p.error)",
                "=> p.warning",
                "=> p.error",
            ]
            .iter()
            .any(|m| l.contains(m))
                || l.trim() == "p.error"
        })
        .count()
}

#[test]
fn problem_census_wiring() {
    let per_file: Vec<(&str, usize)> = FILES.iter().map(|(n, s)| (*n, count(s))).collect();
    let total: usize = per_file.iter().map(|(_, n)| n).sum();
    assert!(
        total <= CEILING,
        "wiring sites went up to {total} (ceiling {CEILING}): {per_file:?}. Say it with a flag \
         (flags_wiring.rs) instead of a toast or a coloured sentence"
    );
    assert!(
        total >= CEILING,
        "wiring sites are down to {total}: lower CEILING in problem_census_wiring_tests.rs \
         ({per_file:?})"
    );
}
