//! Machine options on the drawing: the documents follow the current choice
//! (cut list, reports, generated pages, PDF, bench, 3D), the status bar says
//! which options are on, and `option`, `tie` and `untie` switch and tag.

use novo_core::{DeviceTag, Endpoint, Pin, Placement, Point, SymbolId, SymbolRole, Value};
use novo_workflow::options::{self, OptionDef};

use super::gate1_tests::Drive;
use crate::actions::Action;

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

/// What the tests need to find again.
struct Ids {
    main: novo_core::PageId,
    k9: SymbolId,
    m1: SymbolId,
}

/// A machine with a cabinet `+CAB1`: page 1 has contactors K1 and K9 wired
/// together, page 2 (the Conveyor, only with the conveyor option) has K2
/// and the belt motor M1 wired together. The contactors carry a part with
/// a loss of 4 W.
fn machine(d: &mut Drive) -> Ids {
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let ids = d
        .app
        .backend
        .document_mut()
        .unwrap()
        .transact("Machine", |tx| {
            tx.add_structure(tag("+CAB1"), "Cabinet", true)?;
            let part = tx.add_part("ACME", "C1")?;
            tx.set_property(part, "power_loss_w", Some(Value::Float(4.0)))?;
            let main = tx.add_page("1")?;
            let conv = tx.add_page("2")?;
            tx.update::<novo_core::Page>(conv, |p| p.title = "Conveyor".into())?;
            let pins = || vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 10.0)];
            let place = |tx: &mut novo_core::Tx<'_>, page, t: &str, y: f64, part| {
                let dev = tx.add_device(tag(t), "")?;
                if let Some(part) = part {
                    tx.assign_part(dev, part)?;
                }
                tx.place_symbol(
                    page,
                    "x",
                    Placement::at(Point::new(30.0, y)),
                    pins(),
                    SymbolRole::Device {
                        device: dev,
                        main: true,
                    },
                )
            };
            let k1 = place(tx, main, "=A1+CAB1-K1", 40.0, Some(part))?;
            let k9 = place(tx, main, "=A1+CAB1-K9", 80.0, Some(part))?;
            let k2 = place(tx, conv, "=A1+CAB1-K2", 40.0, Some(part))?;
            let m1 = place(tx, conv, "=A1+FIELD-M1", 80.0, None)?;
            tx.connect(Endpoint::pin(k1, 1), Endpoint::pin(k9, 0))?;
            tx.connect(Endpoint::pin(k2, 1), Endpoint::pin(m1, 0))?;
            options::define_option(tx, OptionDef::yes_no("conveyor", "Conveyor", false))?;
            options::define_option(tx, OptionDef::yes_no("fan", "Fan", false))?;
            options::set_condition(tx, conv, Some("opt.conveyor"))?;
            Ok(Ids { main, k9, m1 })
        })
        .unwrap()
        .0;
    d.app.show_page(ids.main);
    d.frame();
    ids
}

fn lines(path: &std::path::Path) -> usize {
    std::fs::read_to_string(path).unwrap().lines().count()
}

fn say(d: &Drive, text: &str) -> bool {
    d.app.messages.iter().any(|m| m.text.contains(text))
}

#[test]
fn the_cut_list_follows_the_switch_and_the_working_project_keeps_everything() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    machine(&mut d);
    let (wires, pages) = {
        let p = d.app.backend.document().unwrap().project();
        (p.wires().count(), p.pages().count())
    };

    let off = dir.path().join("off.csv");
    d.app.export_wire_cut_to(&off).unwrap();
    d.app
        .run_command(&ctx, "project.option", Some("conveyor on"));
    d.frame();
    let on = dir.path().join("on.csv");
    d.app.export_wire_cut_to(&on).unwrap();
    assert!(
        lines(&on) > lines(&off),
        "the conveyor's wire is cut only with the conveyor: {} vs {}",
        lines(&on),
        lines(&off)
    );
    // Switching back loses nothing.
    d.app
        .run_command(&ctx, "project.option", Some("conveyor off"));
    d.frame();
    let p = d.app.backend.document().unwrap().project();
    assert_eq!((p.wires().count(), p.pages().count()), (wires, pages));
    let again = dir.path().join("again.csv");
    d.app.export_wire_cut_to(&again).unwrap();
    assert_eq!(lines(&again), lines(&off));
}

#[test]
fn a_switch_says_what_it_added_and_a_wrong_name_says_what_exists() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    machine(&mut d);
    d.app
        .run_command(&ctx, "project.option", Some("conveyor on"));
    assert!(
        say(&d, "Conveyor on: adds 1 page and 2 devices"),
        "{:?}",
        d.app.messages.iter().map(|m| &m.text).collect::<Vec<_>>()
    );
    // One undo step.
    let depth = d.app.document().unwrap().undo_depth();
    d.app.run_command(&ctx, "project.option", Some("fan"));
    assert_eq!(d.app.document().unwrap().undo_depth(), depth + 1);
    let now = options::current(d.app.backend.document().unwrap().project());
    assert_eq!(
        now["fan"],
        Value::Bool(true),
        "no value flips a yes/no option"
    );
    d.app.run_command(&ctx, "project.option", Some("moat on"));
    assert!(say(
        &d,
        "No option 'moat on'. This project has: conveyor, fan"
    ));
    d.app
        .run_command(&ctx, "project.option", Some("conveyor maybe"));
    assert!(say(&d, "say on or off"));
}

#[test]
fn the_status_bar_chip_names_the_choice_and_counts_what_is_left_out() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    let before = d.app.machine_info();
    assert!(before.is_none(), "no chip without options");
    machine(&mut d);
    let info = d.app.machine_info().unwrap();
    assert_eq!(info.summary, "no Conveyor, no Fan");
    assert_eq!((info.off.pages, info.off.devices), (1, 2));
    assert!(info.left_out().contains("1 page, 2 devices"));
    d.app
        .run_command(&ctx, "project.option", Some("conveyor on"));
    let info = d.app.machine_info().unwrap();
    assert_eq!(info.summary, "Conveyor, no Fan");
    assert!(info.off.nothing(), "{info:?}");
    // The chip is drawn in every workspace: a frame in Reports does not panic and keeps it.
    d.app.show_report("wires");
    d.frame();
    assert!(d.app.machine_info().is_some());
}

#[test]
fn reports_and_pages_follow_the_choice_and_say_which_machine() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    machine(&mut d);
    let wires = |d: &mut Drive| {
        let (reports, project) = d.app.machine_reports().unwrap();
        let out = reports
            .live(&project)
            .get(&project, "wires")
            .unwrap()
            .clone();
        (
            novo_reports::headline_count("wires", &out).0,
            project.info().name.clone(),
        )
    };
    let (off, off_name) = wires(&mut d);
    d.app
        .run_command(&ctx, "project.option", Some("conveyor on"));
    let (on, on_name) = wires(&mut d);
    assert_eq!(on, off + 1);
    assert!(off_name.ends_with("[no Conveyor, no Fan]"), "{off_name}");
    assert!(on_name.ends_with("[Conveyor, no Fan]"), "{on_name}");
    // The title the documents print carries the choice too.
    assert!(
        d.app
            .machine_title()
            .unwrap()
            .ends_with("[Conveyor, no Fan]")
    );
}

#[test]
fn the_pdf_of_the_project_is_the_machine_of_the_choice() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    machine(&mut d);
    let pages = |p: &std::path::Path| {
        let bytes = std::fs::read(p).unwrap();
        bytes.windows(11).filter(|w| w == b"/Type /Page").count()
    };
    let off = dir.path().join("off.pdf");
    assert!(d.app.export_pdf_to(&off).is_some());
    d.app
        .run_command(&ctx, "project.option", Some("conveyor on"));
    let on = dir.path().join("on.pdf");
    assert!(d.app.export_pdf_to(&on).is_some());
    assert!(
        pages(&on) > pages(&off),
        "{} vs {}",
        pages(&on),
        pages(&off)
    );
}

#[test]
fn tying_a_device_or_a_wire_to_an_option_from_the_drawing() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    let ids = machine(&mut d);
    // K9 on page 1 is tied to the fan: it is a contactor of the fan.
    d.select(ids.k9);
    d.app.run_command(&ctx, "project.tie", Some("fan"));
    let device = {
        let p = d.app.backend.document().unwrap().project();
        let dev = p
            .devices()
            .find(|x| x.tag.to_string().ends_with("-K9"))
            .unwrap()
            .id;
        assert_eq!(options::describe_condition(p, dev).as_deref(), Some("Fan"));
        dev
    };
    assert!(say(&d, "-K9 now appear only with: Fan"));
    let machine_has_k9 = |d: &Drive| d.app.machine().unwrap().device(device).is_some();
    assert!(!machine_has_k9(&d), "the fan is off");
    d.app.run_command(&ctx, "project.option", Some("fan on"));
    assert!(machine_has_k9(&d));
    d.select(ids.k9);
    d.app.run_command(&ctx, "project.untie", None);
    d.app.run_command(&ctx, "project.option", Some("fan off"));
    assert!(machine_has_k9(&d), "untied, it always appears");
    // A choice that is not an option is explained.
    d.select(ids.m1);
    d.app.run_command(&ctx, "project.tie", Some("moat"));
    assert!(say(&d, "No option 'moat'"));
}

#[test]
fn the_cabinet_heat_of_a_choice_is_the_sum_of_the_device_losses() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    machine(&mut d);
    let project = d.app.backend.document().unwrap().project().clone();
    let off = options::current(&project);
    let mut on = off.clone();
    on.insert("conveyor".into(), Value::Bool(true));
    let heat_off = super::variants::cabinet_heat(&project, &off).unwrap();
    let heat_on = super::variants::cabinet_heat(&project, &on).unwrap();
    assert_eq!(heat_off.watts, 8.0, "K1 and K9, 4 W each");
    assert_eq!(heat_on.watts, 12.0, "the conveyor adds K2");
    assert_eq!(heat_on.without_figure, 0);
}

// ----- the QA fixture ------------------------------------------------------

/// The committed project the QA scenario opens: the example motor starter
/// plus a cooling-fan page (breaker and fan motor, only with the Cooling fan
/// option) and a 480 V control transformer on page 1 (only with Supply 480V).
fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../qa/fixtures/machine-options.novo")
}

fn build_fixture(doc: &mut novo_core::Document) {
    super::example::build_example_with(doc, |tx| {
        let pins = |list: &[(&str, f64, f64)]| -> Vec<Pin> {
            list.iter().map(|&(n, x, y)| Pin::new(n, x, y)).collect()
        };
        let part_of = |tx: &novo_core::Tx<'_>, order: &str| {
            tx.project()
                .parts()
                .find(|p| p.order_number == order)
                .map(|p| p.id)
        };
        let breaker = part_of(tx, "5SY6106-7");
        let contactor = part_of(tx, "3RT2016-1AP01");
        let fan = tx.add_page("3")?;
        tx.update::<novo_core::Page>(fan, |p| {
            p.title = "Cooling fan".into();
            p.structure = tag("=A1+CAB");
        })?;
        let q9 = tx.add_device(tag("=A1+CAB-Q9"), "Fan breaker")?;
        let m9 = tx.add_device(tag("=A1+CAB-M9"), "Cabinet fan")?;
        if let Some(part) = breaker {
            tx.assign_part(q9, part)?;
        }
        let at = |x, y| Placement::at(Point::new(x, y));
        let three = || {
            pins(&[
                ("1", 0.0, 0.0),
                ("2", 0.0, 16.0),
                ("3", 8.0, 0.0),
                ("4", 8.0, 16.0),
                ("5", 16.0, 0.0),
                ("6", 16.0, 16.0),
            ])
        };
        let q = tx.place_symbol(
            fan,
            "iec/protection/fuse_3p",
            at(40.0, 40.0),
            three(),
            SymbolRole::Device {
                device: q9,
                main: true,
            },
        )?;
        let m = tx.place_symbol(
            fan,
            "iec/motors/motor_3ph",
            at(40.0, 90.0),
            pins(&[
                ("U1", 0.0, 0.0),
                ("V1", 8.0, 0.0),
                ("W1", 16.0, 0.0),
                ("PE", 24.0, 0.0),
            ]),
            SymbolRole::Device {
                device: m9,
                main: true,
            },
        )?;
        for (from, to) in [(1, 0), (3, 1), (5, 2)] {
            tx.connect(Endpoint::pin(q, from), Endpoint::pin(m, to))?;
        }
        novo_numbering::number_new_wires(tx)?;
        // A 480 V control transformer on the main circuit page.
        let page1 = tx
            .project()
            .pages_in_order()
            .first()
            .map(|p| p.id)
            .expect("the example has pages");
        let t9 = tx.add_device(tag("=A1+CAB-T9"), "Control transformer 480 V")?;
        if let Some(part) = contactor {
            tx.assign_part(t9, part)?;
        }
        tx.place_symbol(
            page1,
            "iec/contactors/main_3p",
            at(120.0, 72.0),
            three(),
            SymbolRole::Device {
                device: t9,
                main: true,
            },
        )?;
        options::define_option(tx, OptionDef::yes_no("fan", "Cooling fan", false))?;
        options::define_option(
            tx,
            OptionDef::choice("supply", "Supply", &["400V", "480V"], "400V"),
        )?;
        options::set_condition(tx, fan, Some("opt.fan"))?;
        options::set_condition(tx, t9, Some("opt.supply == '480V'"))?;
        options::save_variant(tx, "Standard", "")?;
        options::set_option(tx, "fan", Value::Bool(true))?;
        options::set_option(tx, "supply", Value::Text("480V".into()))?;
        options::save_variant(tx, "Export with fan", "")?;
        options::set_option(tx, "fan", Value::Bool(false))?;
        options::set_option(tx, "supply", Value::Text("400V".into()))
    })
    .unwrap();
}

/// Rewrites the fixture: `cargo test -p novo-app write_machine_fixture -- --ignored`.
#[test]
#[ignore = "writes qa/fixtures/machine-options.novo"]
fn write_machine_fixture() {
    use crate::backend::ProjectBackend;
    let mut backend = crate::backend::StorageBackend::default();
    backend.new_project("Machine options (example)").unwrap();
    build_fixture(backend.document_mut().unwrap());
    let path = fixture_path();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let _ = std::fs::remove_file(&path);
    backend.save(&path).unwrap();
}

#[test]
fn the_committed_fixture_opens_with_its_options() {
    use crate::backend::ProjectBackend;
    let path = fixture_path();
    assert!(path.exists(), "run write_machine_fixture");
    // Open a copy: opening takes a lock and may write beside the file.
    let dir = tempfile::tempdir().unwrap();
    let copy = dir.path().join("fixture.novo");
    std::fs::copy(&path, &copy).unwrap();
    let mut backend = crate::backend::StorageBackend::default();
    backend.open(&copy).unwrap();
    let project = backend.document().unwrap().project();
    let defs = options::options(project);
    assert_eq!(
        defs.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(),
        ["fan", "supply"]
    );
    assert_eq!(options::variants(project).len(), 2);
    let standard = options::variant_assignment(project, "Standard").unwrap();
    let off = options::variant_project(project, &standard).unwrap();
    let export = options::variant_assignment(project, "Export with fan").unwrap();
    let on = options::variant_project(project, &export).unwrap();
    assert_eq!(on.pages().count(), off.pages().count() + 1);
    assert_eq!(on.devices().count(), off.devices().count() + 3);
    assert!(options::lint(project).is_empty());
}
