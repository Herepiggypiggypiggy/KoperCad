//! What a part of a crane model is, guessed from its name.
//!
//! A STEP file of a crane names its solids in the language of whoever drew it. The guess looks for
//! common English, Dutch and German words; anything it does not recognise is `Other` and drawn as
//! part of the frame. A name is a hint, never a measurement: the caller can always override it.

/// A part's job on the crane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CraneRole {
    /// A wheel of an end truck or the trolley: turns with travel.
    Wheel,
    /// The rope drum: turns with the hook.
    Drum,
    /// The hook block (the housing with the sheaves): moves with the hook.
    HookBlock,
    /// The hook itself: moves with the hook.
    Hook,
    /// A girder or runway beam.
    Girder,
    /// A rail.
    Rail,
    /// A drive motor.
    Motor,
    /// A gearbox.
    Gearbox,
    /// Anything else.
    Other,
}

impl CraneRole {
    /// A short label for lists.
    pub fn label(self) -> &'static str {
        match self {
            CraneRole::Wheel => "wheel",
            CraneRole::Drum => "drum",
            CraneRole::HookBlock => "hook block",
            CraneRole::Hook => "hook",
            CraneRole::Girder => "girder",
            CraneRole::Rail => "rail",
            CraneRole::Motor => "motor",
            CraneRole::Gearbox => "gearbox",
            CraneRole::Other => "other",
        }
    }

    /// Whether the part moves with the hook (the block, hook and sheaves), not with the frame.
    pub fn follows_hook(self) -> bool {
        matches!(self, CraneRole::HookBlock | CraneRole::Hook)
    }
}

/// Words that name each role, checked in order: the first role with a hit wins, so the more
/// specific ones (hook block before hook, gearbox before motor) come first.
const WORDS: &[(CraneRole, &[&str])] = &[
    (
        CraneRole::HookBlock,
        &[
            "hook block",
            "hookblock",
            "hook_block",
            "hook-block",
            "haakblok",
            "haakblock",
            "onderblok",
            "unterflasche",
            "lasthaken block",
            "bottom block",
        ],
    ),
    (CraneRole::Hook, &["hook", "haak", "haken"]),
    (
        CraneRole::Gearbox,
        &[
            "gearbox",
            "gear box",
            "getriebe",
            "reductor",
            "reductie",
            "tandwielkast",
        ],
    ),
    (CraneRole::Motor, &["motor", "moteur"]),
    (
        CraneRole::Drum,
        &[
            "drum",
            "trommel",
            "seiltrommel",
            "kabeltrommel",
            "touwtrommel",
        ],
    ),
    (
        CraneRole::Wheel,
        &["wheel", "wiel", "laufrad", "kranrad", "runner"],
    ),
    (CraneRole::Rail, &["rail", "schiene"]),
    (
        CraneRole::Girder,
        &[
            "girder",
            "beam",
            "ligger",
            "bruggirder",
            "träger",
            "traeger",
            "brücke",
            "bruecke",
        ],
    ),
];

/// The role a part's name suggests.
pub fn role_of(name: &str) -> CraneRole {
    let n = name.to_lowercase();
    WORDS
        .iter()
        .find(|(_, words)| words.iter().any(|w| n.contains(w)))
        .map_or(CraneRole::Other, |(role, _)| *role)
}

/// The parts of a model as a Markdown table: name, the role the name suggests, size in mm and
/// triangle count, then how many parts each role has. Reads like the report of `parts step-health`.
pub fn parts_report(parts: &[super::build::PlacedPart]) -> String {
    let mut out =
        String::from("| Part | Role | Size mm (x y z) | Triangles |\n| --- | --- | --- | --- |\n");
    let mut counts: Vec<(CraneRole, usize)> = Vec::new();
    for p in parts {
        let role = role_of(&p.name);
        match counts.iter_mut().find(|(r, _)| *r == role) {
            Some((_, n)) => *n += 1,
            None => counts.push((role, 1)),
        }
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        for v in &p.soup.pos {
            for k in 0..3 {
                lo[k] = lo[k].min(v[k]);
                hi[k] = hi[k].max(v[k]);
            }
        }
        let size = if p.soup.pos.is_empty() {
            "-".to_owned()
        } else {
            format!(
                "{:.0} x {:.0} x {:.0}",
                hi[0] - lo[0],
                hi[1] - lo[1],
                hi[2] - lo[2]
            )
        };
        let name = if p.name.is_empty() {
            "(no name)"
        } else {
            p.name.as_str()
        };
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            name.replace('|', "/"),
            role.label(),
            size,
            p.soup.tris.len()
        ));
    }
    out.push('\n');
    for (role, n) in counts {
        out.push_str(&format!("- {}: {}\n", role.label(), n));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_in_three_languages_get_their_role() {
        assert_eq!(role_of("Wheel_LH_01"), CraneRole::Wheel);
        assert_eq!(role_of("Loopwiel A"), CraneRole::Wheel);
        assert_eq!(role_of("Laufrad 2"), CraneRole::Wheel);
        assert_eq!(role_of("Rope drum"), CraneRole::Drum);
        assert_eq!(role_of("Seiltrommel"), CraneRole::Drum);
        assert_eq!(role_of("Haakblok"), CraneRole::HookBlock);
        assert_eq!(role_of("HOOK BLOCK assy"), CraneRole::HookBlock);
        assert_eq!(role_of("Lasthaken"), CraneRole::Hook);
        assert_eq!(role_of("main girder"), CraneRole::Girder);
        assert_eq!(role_of("Crane rail 40x40"), CraneRole::Rail);
    }

    #[test]
    fn the_more_specific_role_wins() {
        // A hook block is not just a hook, and a gear motor's name holds both words.
        assert_eq!(role_of("hook block"), CraneRole::HookBlock);
        assert_eq!(role_of("Gearbox for motor"), CraneRole::Gearbox);
        assert_eq!(role_of("Motor 4kW"), CraneRole::Motor);
    }

    #[test]
    fn unknown_and_empty_names_are_other() {
        assert_eq!(role_of(""), CraneRole::Other);
        assert_eq!(role_of("Bracket 17"), CraneRole::Other);
        assert!(!CraneRole::Other.follows_hook());
        assert!(CraneRole::Hook.follows_hook() && CraneRole::HookBlock.follows_hook());
    }

    #[test]
    fn the_report_lists_each_part_with_its_role_and_size() {
        use crate::part3d::build::PlacedPart;
        use crate::part3d::tess::Soup;
        let soup = Soup {
            pos: vec![[0.0, 0.0, 0.0], [10.0, 20.0, 30.0], [10.0, 0.0, 0.0]],
            tris: vec![[0, 1, 2]],
            ..Default::default()
        };
        let parts = vec![
            PlacedPart {
                name: "Wheel 1".into(),
                soup: soup.clone(),
            },
            PlacedPart {
                name: String::new(),
                soup,
            },
        ];
        let md = parts_report(&parts);
        assert!(
            md.contains("| Wheel 1 | wheel | 10 x 20 x 30 | 1 |"),
            "{md}"
        );
        assert!(
            md.contains("| (no name) | other | 10 x 20 x 30 | 1 |"),
            "{md}"
        );
        assert!(
            md.contains("- wheel: 1") && md.contains("- other: 1"),
            "{md}"
        );
    }
}
