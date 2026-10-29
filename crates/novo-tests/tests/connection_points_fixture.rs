//! The connection-point QA fixture: a terminal -X1:1 whose part states a
//! conductor range of 0.5-2.5 mm² and a torque of 0.4-0.5 N·m, with a 6 mm²
//! wire on its field side (too large) and a 1.5 mm² wire on its cabinet side.
//!
//! Regenerate with `KOPERCAD_WRITE_FIXTURES=1 cargo test -p novo-tests --test connection_points_fixture`
//! and commit `qa/fixtures/connection-points.novo`.

use novo_core::*;

fn tag(t: &str) -> DeviceTag {
    DeviceTag::parse(t).unwrap()
}

fn build() -> Project {
    let mut doc = Document::default();
    doc.transact("Draw", |tx| {
        let page = tx.add_page("1")?;
        let m1 = tx.add_device(tag("-M1"), "Pump motor")?;
        let k1 = tx.add_device(tag("-K1"), "Contactor")?;
        let motor = tx.place_symbol(
            page,
            "iec/motors/motor_1ph",
            Placement::at(Point::new(40.0, 100.0)),
            vec![Pin::new("U", 0.0, 0.0)],
            SymbolRole::Device {
                device: m1,
                main: true,
            },
        )?;
        let contact = tx.place_symbol(
            page,
            "iec/contacts/no",
            Placement::at(Point::new(40.0, 20.0)),
            vec![Pin::new("13", 0.0, 0.0), Pin::new("14", 0.0, 10.0)],
            SymbolRole::Device {
                device: k1,
                main: false,
            },
        )?;
        let wire = tx.connect(Endpoint::pin(contact, 1), Endpoint::pin(motor, 0))?;
        let strip = novo_terminals::strip_in_use(tx, page)?;
        let placed = novo_terminals::place_terminals(
            tx,
            strip,
            &[novo_terminals::Drop {
                wire,
                at: Point::new(40.0, 60.0),
            }],
        )?;
        let part = tx.add_part("Acme", "CLAMP-2.5")?;
        tx.update::<Part>(part, |p| {
            for (k, v) in [
                ("pin.1.wire_min_mm2", 0.5),
                ("pin.1.wire_max_mm2", 2.5),
                ("pin.1.torque_min_nm", 0.4),
                ("pin.1.torque_max_nm", 0.5),
                ("pin.2.wire_min_mm2", 0.5),
                ("pin.2.wire_max_mm2", 2.5),
                ("pin.2.torque_min_nm", 0.4),
                ("pin.2.torque_max_nm", 0.5),
            ] {
                p.properties.insert(k.into(), Value::Float(v));
            }
            p.properties
                .insert("pin_data_source".into(), Value::Text("maker".into()));
        })?;
        tx.update::<Terminal>(placed[0].terminal, |t| t.part = Some(part))?;
        tx.update::<Wire>(placed[0].internal_wire, |w| {
            w.number = Some("101".into());
            w.cross_section_mm2 = Some(1.5);
        })?;
        tx.update::<Wire>(placed[0].external_wire, |w| {
            w.number = Some("102".into());
            w.cross_section_mm2 = Some(6.0);
        })?;
        Ok(())
    })
    .unwrap();
    doc.project().clone()
}

fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../qa/fixtures/connection-points.novo")
}

#[test]
fn the_committed_fixture_has_a_wire_too_large_for_its_terminal() {
    if std::env::var_os("KOPERCAD_WRITE_FIXTURES").is_some() {
        let path = fixture_path();
        let _ = std::fs::remove_file(&path);
        novo_storage::ProjectFile::create(&path, &build()).expect("fixture is written");
    }
    let project = novo_storage::ProjectFile::open(fixture_path())
        .and_then(|f| f.load())
        .expect("fixture opens");
    let found: Vec<_> = novo_checks::check(&project)
        .into_iter()
        .filter(|p| p.kind == novo_checks::ProblemKind::WireRange)
        .collect();
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].message.contains("102"), "{}", found[0].message);
}
