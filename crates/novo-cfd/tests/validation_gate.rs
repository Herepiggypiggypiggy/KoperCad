//! The rule for `ENCLOSURE_VALIDATED` (lane H6): the flag may only be true
//! when the reference register holds a passed `measured` or `licensed`
//! reference. Analytic cases, the standard hand calculation, comparisons with
//! the 2D engine and third-party models (RiTherm) are listed too, but they
//! never count towards the flag, however many there are.

use std::path::Path;

const REGISTER: &str = include_str!("../validation/references.txt");
const KINDS: [&str; 6] = [
    "analytic",
    "standard",
    "third_party_model",
    "model_vs_model",
    "measured",
    "licensed",
];
const STATUS: [&str; 4] = ["passed", "failed", "planned", "manual"];

struct Reference {
    id: String,
    kind: String,
    status: String,
    test: String,
}

fn register() -> Vec<Reference> {
    REGISTER
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| {
            let f: Vec<&str> = l.split('|').map(str::trim).collect();
            assert!(f.len() == 5, "register line needs 5 fields: {l}");
            Reference {
                id: f[0].into(),
                kind: f[1].into(),
                status: f[2].into(),
                test: f[3].into(),
            }
        })
        .collect()
}

#[test]
fn the_register_is_well_formed_and_points_at_real_tests() {
    let refs = register();
    assert!(refs.len() >= 8);
    let mut ids: Vec<&str> = refs.iter().map(|r| r.id.as_str()).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), refs.len(), "duplicate id");
    let here = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let thermal = Path::new(env!("CARGO_MANIFEST_DIR")).join("../novo-thermal/tests");
    for r in &refs {
        assert!(
            KINDS.contains(&r.kind.as_str()),
            "{}: kind {}",
            r.id,
            r.kind
        );
        assert!(
            STATUS.contains(&r.status.as_str()),
            "{}: status {}",
            r.id,
            r.status
        );
        assert!(
            here.join(&r.test).exists() || thermal.join(&r.test).exists(),
            "{}: test file {} is missing",
            r.id,
            r.test
        );
    }
}

#[test]
fn only_a_passed_measured_or_licensed_reference_can_validate_the_enclosure() {
    let counts = register()
        .iter()
        .filter(|r| (r.kind == "measured" || r.kind == "licensed") && r.status == "passed")
        .count();
    let flag = std::hint::black_box(novo_cfd::cabinet::ENCLOSURE_VALIDATED);
    if flag {
        assert!(
            counts >= 1,
            "ENCLOSURE_VALIDATED is true without a passed measured or licensed reference"
        );
    }
    // Today none exists: the flag must say so.
    if counts == 0 {
        assert!(!flag);
    }
}
