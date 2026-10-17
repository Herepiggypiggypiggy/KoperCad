//! The Connection points block: rows, summary, applying and saving.

use novo_interop::part3d::pins::{Fix, PinMapping, TerminalFix};
use novo_parts::{Catalog, CatalogPart, PartTerminal, terminal_position};

use super::parts_pins::{Detection, apply, rows, save_to_user_catalog, summary};

fn part() -> CatalogPart {
    let mut p = CatalogPart::new("ACME", "C1");
    p.terminals = ["A1", "A2", "A3"]
        .into_iter()
        .map(|n| PartTerminal {
            name: n.into(),
            ..Default::default()
        })
        .collect();
    p
}

fn detection() -> Detection {
    let fix = |name: &str, x: f64, fix| TerminalFix {
        name: name.into(),
        position_mm: [x, 60.0, 20.0],
        fix,
    };
    Detection {
        mapping: PinMapping {
            fixes: vec![
                fix("A1", 5.0, Fix::Measured),
                fix("A2", 15.0, Fix::Estimated),
            ],
            unplaced: vec!["A3".into()],
            unused_candidates: 0,
        },
        openings: 4,
        pins: 0,
    }
}

#[test]
fn rows_list_every_terminal_with_how_it_was_found() {
    let r = rows(&detection());
    let how: Vec<_> = r.iter().map(|r| (r.name.as_str(), r.how)).collect();
    assert_eq!(
        how,
        [
            ("A1", "Measured"),
            ("A2", "Estimated"),
            ("A3", "Not placed")
        ]
    );
    assert_eq!(r[0].position, "5.0 / 60.0 / 20.0 mm");
    assert!(summary(&detection()).contains("1 measured, 0 partial, 1 estimated, 1 not placed"));
}

#[test]
fn estimated_points_are_saved_only_when_asked() {
    let mut p = part();
    assert_eq!(apply(&mut p, &detection(), false), 1);
    assert!(p.terminals[0].position_mm.is_some() && p.terminals[1].position_mm.is_none());
    let mut p = part();
    assert_eq!(apply(&mut p, &detection(), true), 2);
}

#[test]
fn a_saved_part_reaches_the_catalog_file_and_the_project_part() {
    let dir = std::env::temp_dir().join(format!("kopercad-pins-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("user.json");
    let mut p = part();
    apply(&mut p, &detection(), false);
    save_to_user_catalog(&path, &p).unwrap();
    let back = Catalog::load(&path).unwrap();
    let saved = back.get(&p.key()).unwrap();
    assert_eq!(saved.terminals[0].position_mm, Some([5.0, 60.0, 20.0]));
    let model = saved.to_model(novo_core::PartId::from(1));
    assert_eq!(
        terminal_position(&model.properties, "A1"),
        Some([5.0, 60.0, 20.0])
    );
    std::fs::remove_dir_all(&dir).ok();
}
