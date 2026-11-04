//! Problem triangles, wiring lane (T5).

use crate::actions::Action;
use crate::app::flags_wiring::{JobRetry, job};
use crate::app::gate1_tests::Drive;
use novo_core::{DeviceTag, Endpoint, Part, PinRef, Placement, Point, SymbolRole, Value, WireId};
use novo_flags::{Anchor, Source};

/// A contactor wired to a terminal whose part takes 0.5-2.5 mm2 on point 2,
/// with a 6 mm2 wire on it. Returns the too-large wire.
fn project_with_wire_too_large(d: &mut Drive) -> WireId {
    use novo_core::Pin;
    use novo_terminals::{Drop, place_terminals, strip_in_use};
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let page = d.app.current_page().expect("a page is shown");
    let (wire, _) = d
        .app
        .backend
        .document_mut()
        .unwrap()
        .transact("Wire with a part", |tx| {
            let m1 = tx.add_device(DeviceTag::parse("-M1")?, "")?;
            let k1 = tx.add_device(DeviceTag::parse("-K1")?, "")?;
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
            let strip = strip_in_use(tx, page)?;
            let placed = place_terminals(
                tx,
                strip,
                &[Drop {
                    wire,
                    at: Point::new(40.0, 60.0),
                }],
            )?;
            let part = tx.add_part("Acme", "CLAMP")?;
            tx.update::<Part>(part, |p| {
                for (k, v) in [
                    ("pin.1.wire_min_mm2", 0.5),
                    ("pin.1.wire_max_mm2", 2.5),
                    ("pin.2.wire_min_mm2", 0.5),
                    ("pin.2.wire_max_mm2", 2.5),
                ] {
                    p.properties.insert(k.into(), Value::Float(v));
                }
                p.properties
                    .insert("pin_data_source".into(), Value::Text("maker".into()));
            })?;
            tx.update::<novo_core::Terminal>(placed[0].terminal, |t| t.part = Some(part))?;
            tx.update::<novo_core::Wire>(placed[0].external_wire, |w| {
                w.cross_section_mm2 = Some(6.0)
            })?;
            Ok(placed[0].external_wire)
        })
        .unwrap();
    d.frame();
    d.frame();
    wire
}

#[test]
fn a_wire_too_large_for_its_terminal_is_flagged_on_the_pin_it_lands_on() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let wire = project_with_wire_too_large(&mut d);
    // Let the checks finish.
    for _ in 0..30 {
        d.frame();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    d.app.sync_flags();
    let flags: Vec<_> = d
        .app
        .wb
        .flags
        .store
        .list(false)
        .into_iter()
        .filter(|f| f.title == "Wire size at terminal")
        .cloned()
        .collect();
    assert_eq!(flags.len(), 1, "{:?}", d.app.wb.flags.store.list(false));
    let flag = &flags[0];
    assert_eq!(flag.severity, novo_flags::Severity::Red);
    assert!(
        flag.anchors.iter().all(|a| matches!(a, Anchor::Pin(..))),
        "{:?}",
        flag.anchors
    );
    // The flag names the pin of the symbol that carries the wire.
    let project = d.app.backend.document().unwrap().project();
    let w = project.wire(wire).unwrap();
    let ends: Vec<PinRef> = [w.from, w.to]
        .iter()
        .filter_map(|e| match e {
            Endpoint::Pin { symbol, pin } => Some(PinRef::new(*symbol, *pin)),
            _ => None,
        })
        .collect();
    let raws: Vec<String> = ends
        .iter()
        .map(|p| novo_core::AnyId::from(p.symbol).raw().to_string())
        .collect();
    assert!(flag.anchors.iter().all(|a| {
        match a {
            Anchor::Pin(raw, pin) => ends
                .iter()
                .zip(&raws)
                .any(|(p, r)| r == raw && u32::from(p.pin) == *pin),
            _ => false,
        }
    }));
    // The triangle is drawn on the page, at a point: a mark with an area.
    let marks = &d.app.canvas.schematic_ref().unwrap().canvas.marks.list;
    assert!(
        marks
            .iter()
            .any(|m| m.key == flag.id.0 && matches!(m.target, novo_canvas::MarkTarget::Area(_))),
        "no mark for the pin"
    );
    // The waiver key of the old check is kept.
    assert_eq!(flag.waiver_key.as_deref(), Some("check.wire_range"));
}

#[test]
fn a_failed_export_is_a_job_flag_until_the_same_job_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    project_with_wire_too_large(&mut d);
    // A path below a plain file cannot be created.
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, b"x").unwrap();
    let toasts = d.app.messages.len();
    let bad = blocker.join("cut.csv");
    assert!(d.app.export_wire_cut_to(&bad).is_none());
    let anchor = Anchor::Job(job::WIRE_CUT.to_owned());
    let flag = d
        .app
        .wb
        .flags
        .store
        .at(&anchor, false)
        .first()
        .map(|f| (*f).clone())
        .expect("the failure is a flag on the job");
    assert_eq!(flag.source, Source::Job);
    assert_eq!(flag.severity, novo_flags::Severity::Red);
    assert!(flag.detail.0[0].contains("cut.csv"), "{:?}", flag.detail);
    assert_eq!(
        d.app.wb.flags.retry.get(&flag.id),
        Some(&JobRetry::WireCut),
        "Retry runs the same export"
    );
    assert_eq!(d.app.messages.len(), toasts, "no red toast beside the flag");
    // It shows in the top bar and keeps showing across frames.
    d.frame();
    assert!(d.app.wb.flags.store.at(&anchor, false).len() == 1);
    // The same job, to a good path, clears it.
    let good = dir.path().join("cut.csv");
    assert!(d.app.export_wire_cut_to(&good).is_some());
    assert!(d.app.wb.flags.store.at(&anchor, false).is_empty());
    assert!(d.app.wb.flags.retry.is_empty());
}

#[test]
fn an_export_with_nothing_to_do_is_a_yellow_flag() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    assert!(d.app.export_wire_it().is_none());
    let flag = d
        .app
        .wb
        .flags
        .store
        .at(&Anchor::Job(job::WIRE_IT.to_owned()), false)
        .first()
        .map(|f| (*f).clone())
        .expect("a yellow flag");
    assert_eq!(flag.severity, novo_flags::Severity::Yellow);
    assert!(flag.title.chars().count() <= 28);
}

#[test]
fn a_refused_wire_panel_edit_flags_its_field_and_clears_when_it_takes() {
    use crate::app::wire_panel::WireEdit;
    assert_eq!(WireEdit::Section(String::new()).field(), "wire_section");
    assert_eq!(WireEdit::TerminationB(String::new()).field(), "wire_size_b");
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    d.app.wire_field_flag(
        "wire_length",
        "Wire edit refused",
        "Length must be a number".into(),
    );
    let anchor = Anchor::Control("wire_panel.wire_length".to_owned());
    assert_eq!(d.app.wb.flags.store.at(&anchor, false).len(), 1);
    // The same refusal again renews, not stacks.
    d.app.wire_field_flag(
        "wire_length",
        "Wire edit refused",
        "Length must be a number".into(),
    );
    assert_eq!(d.app.wb.flags.store.at(&anchor, false).len(), 1);
    d.app.wire_field_clear("wire_length");
    assert!(d.app.wb.flags.store.at(&anchor, false).is_empty());
}
