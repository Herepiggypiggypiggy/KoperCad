//! Problem triangles census for the EMC, safety, protection, calculations and
//! standards area (lane T11). Counts the places that still put a problem in
//! front of the person as a toast, a coloured sentence or a banner. The
//! ceiling lives in `handoff/problem-triangles/ceilings/safety.txt` (one file
//! per area, so two lane PRs never edit the same number) and only goes down.

const FILES: &[&str] = &[
    "emc",
    "emc_adapter",
    "emc_view",
    "emc_schematic",
    "emc_record",
    "emc_plate",
    "safety",
    "safety_calc",
    "safety_functions",
    "safety_report",
    "protection",
    "calculations",
    "calculations_mechanical",
    "standards",
    "standards_report",
    "electrical",
];

fn census() -> usize {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app");
    FILES
        .iter()
        .map(|f| {
            let text = std::fs::read_to_string(dir.join(format!("{f}.rs"))).unwrap_or_default();
            text.lines()
                .filter(|l| {
                    l.contains("Level::Error")
                        || l.contains("Level::Warning")
                        || l.contains(".error")
                        || l.contains(".warning")
                        || l.contains("banner(")
                })
                .count()
        })
        .sum()
}

#[test]
fn problem_census_safety_stays_under_its_ceiling() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../handoff/problem-triangles/ceilings/safety.txt");
    let ceiling: usize = std::fs::read_to_string(path)
        .expect("ceiling file")
        .trim()
        .parse()
        .expect("a number");
    let now = census();
    assert!(
        now <= ceiling,
        "{now} sites against a ceiling of {ceiling}: show the new problem as a flag, or lower the ceiling when sites go"
    );
}
