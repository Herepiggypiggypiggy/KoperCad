//! The project-level drawing standard (the symbol pack id).

use novo_core::*;

fn set(doc: &mut Document, standard: &str) -> Result<Outcome> {
    let mut info = doc.project().info().clone();
    info.rules.standard = standard.into();
    doc.execute(Command::SetProjectInfo { info })
}

#[test]
fn the_standard_is_set_undone_and_checked() {
    let mut doc = Document::default();
    assert_eq!(doc.project().info().rules.standard, "");
    set(&mut doc, "nfpa").unwrap();
    assert_eq!(doc.project().info().rules.standard, "nfpa");
    doc.undo().unwrap();
    assert_eq!(doc.project().info().rules.standard, "");
    for bad in ["NFPA", "iec 60617", "iec/1", "é"] {
        assert!(set(&mut doc, bad).is_err(), "{bad}");
    }
    set(&mut doc, "iec").unwrap();
    set(&mut doc, "").unwrap();
}

#[test]
fn only_a_set_standard_reaches_the_file() {
    assert!(ProjectRules::recommended().standard.is_empty());
    let plain = Project::new("A").to_json();
    assert!(!plain.contains("standard"));

    let mut doc = Document::default();
    set(&mut doc, "nfpa").unwrap();
    let json = doc.project().to_json();
    assert!(json.contains("\"standard\": \"nfpa\""));
    let back = Project::from_json(&json).unwrap();
    assert_eq!(back.info().rules.standard, "nfpa");
    assert_eq!(back.to_json(), json);
}
