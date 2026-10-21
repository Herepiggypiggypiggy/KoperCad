//! Every route to the issue list gives the same problems: the app, the PDF
//! report and `kopercad-cli check` read `novo_reports::issues`, so the house
//! wire rules (which only novo-reports can read) show up in all of them, and
//! a waiver hides a problem in all of them.

use novo_checks::standards::set_info_property;
use novo_checks::{ProblemKind, waive};
use novo_core::{Document, Value, Wire};
use novo_reports::{issues, wire_rules};
use novo_storage::ProjectFile;
use novo_tests::motor_starter::motor_starter;

/// The motor starter with every wire BK and 1.5 mm², under house rules that
/// want violet power wires of at least 4 mm².
fn house_project() -> Document {
    let mut doc = Document::default();
    motor_starter(&mut doc).unwrap();
    let wires: Vec<_> = doc.project().wires().map(|w| w.id).collect();
    doc.transact("wires", |tx| {
        for id in &wires {
            tx.update::<Wire>(*id, |w| {
                w.color = Some("BK".into());
                w.cross_section_mm2 = Some(1.5);
            })?;
        }
        for (key, value) in [
            (wire_rules::PRESET.to_owned(), "house"),
            (wire_rules::ColourClass::AcPower.key(), "VT"),
            (wire_rules::MIN_POWER_MM2.to_owned(), "4"),
        ] {
            let value = match value {
                "4" => Value::Float(4.0),
                v => Value::Text(v.into()),
            };
            set_info_property(tx, &key, Some(value))?;
        }
        Ok(())
    })
    .unwrap();
    doc
}

fn cli_messages(path: &std::path::Path) -> Vec<String> {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    novo_cli::run(
        ["kopercad-cli", "check", "--json", path.to_str().unwrap()],
        &mut out,
        &mut err,
    );
    let list: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let mut messages: Vec<String> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["message"].as_str().unwrap().to_owned())
        .collect();
    messages.sort();
    messages
}

/// The issue list as the Python bindings (`ScriptProject.problems()`) give it.
fn script_messages(path: &std::path::Path) -> Vec<String> {
    let project = novo_py::ScriptProject::open(path).unwrap();
    let mut messages: Vec<String> = project
        .problems()
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["message"].as_str().unwrap().to_owned())
        .collect();
    messages.sort();
    messages
}

fn sorted(problems: &[novo_checks::Problem]) -> Vec<String> {
    let mut messages: Vec<String> = problems.iter().map(|p| p.message.clone()).collect();
    messages.sort();
    messages
}

#[test]
fn the_cli_the_report_and_the_app_list_the_same_problems() {
    let mut doc = house_project();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("house.novo");
    ProjectFile::create(&path, doc.project()).unwrap();

    let open = issues::open(doc.project());
    let rule: Vec<_> = open
        .iter()
        .filter(|p| p.kind == ProblemKind::WireRule)
        .cloned()
        .collect();
    assert!(!rule.is_empty(), "the house rules are broken");
    assert_eq!(cli_messages(&path), sorted(&open), "the CLI lists them");
    assert_eq!(script_messages(&path), sorted(&open), "Python lists them");
    let report = issues::report_lines(doc.project());
    for p in &rule {
        assert!(report.iter().any(|(_, t)| t.contains(&p.message)), "{p:?}");
    }

    // A waiver hides the problem on every route and lists it as waived.
    let first = rule[0].clone();
    doc.transact("waive", |tx| waive(tx, &first, "checked").map(|_| ()))
        .unwrap();
    ProjectFile::create(dir.path().join("waived.novo"), doc.project()).unwrap();
    let open = issues::open(doc.project());
    assert!(
        open.iter()
            .all(|p| p.at != first.at || p.kind != first.kind)
    );
    assert_eq!(cli_messages(&dir.path().join("waived.novo")), sorted(&open));
    let waived = issues::waived(doc.project());
    assert!(waived.iter().any(|w| w.problem.at == first.at));
    let report = issues::report_lines(doc.project());
    assert!(report.iter().any(|(_, t)| t.contains("Reason: checked")));
}
