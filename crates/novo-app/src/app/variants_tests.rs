//! Project options, variants and the spec-sheet generator in the app.

use novo_core::{DeviceTag, Endpoint, Pin, Placement, Point, SymbolRole, Value};
use novo_workflow::options::{self, OptionDef};

use super::gate1_tests::Drive;
use crate::actions::Action;

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

/// A project with a main page and a Conveyor page that exists only with the
/// conveyor option, as a window user would set it up.
fn machine(d: &mut Drive) {
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Machine", |tx| {
            let main = tx.add_page("1")?;
            let conv = tx.add_page("2")?;
            tx.update::<novo_core::Page>(conv, |p| p.title = "Conveyor".into())?;
            let a = tx.add_device(tag("=A1-K1"), "Main contactor")?;
            let b = tx.add_device(tag("=A1-M1"), "Belt motor")?;
            let pins = || vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 10.0)];
            let role = |d| SymbolRole::Device {
                device: d,
                main: true,
            };
            let s1 = tx.place_symbol(
                main,
                "x",
                Placement::at(Point::new(30.0, 40.0)),
                pins(),
                role(a),
            )?;
            let s2 = tx.place_symbol(
                conv,
                "x",
                Placement::at(Point::new(30.0, 40.0)),
                pins(),
                role(b),
            )?;
            let s3 = tx.place_symbol(
                conv,
                "x",
                Placement::at(Point::new(30.0, 80.0)),
                pins(),
                role(b),
            )?;
            let _ = (s1, tx.connect(Endpoint::pin(s2, 1), Endpoint::pin(s3, 0))?);
            options::define_option(tx, OptionDef::yes_no("conveyor", "Conveyor", false))?;
            options::set_condition(tx, conv, Some("opt.conveyor"))?;
            options::save_variant(tx, "Bare", "")?;
            options::set_option(tx, "conveyor", Value::Bool(true))?;
            options::save_variant(tx, "With conveyor", "")?;
            options::set_option(tx, "conveyor", Value::Bool(false))
        })
        .unwrap();
    d.frame();
}

#[test]
fn the_options_window_opens_and_a_variant_switches_from_the_command_field() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    machine(&mut d);

    assert!(super::workbench::app_runs("project.options"));
    d.app.run_command(&ctx, "project.options", None);
    assert!(d.app.wb.options.is_some());
    d.frame();
    d.frame();
    assert!(d.app.wb.options.is_some(), "the window stays open");

    // `variant With conveyor` makes that variant the current choice, as one undo step.
    d.app
        .run_command(&ctx, "project.variant", Some("with conveyor"));
    d.frame();
    let project = d.app.backend.document().unwrap().project();
    assert_eq!(options::current(project)["conveyor"], Value::Bool(true));
    d.app.backend.document_mut().unwrap().undo().unwrap();
    let project = d.app.backend.document().unwrap().project();
    assert_eq!(options::current(project)["conveyor"], Value::Bool(false));
}

#[test]
fn a_variant_pdf_leaves_out_what_the_options_switch_off() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    machine(&mut d);
    assert!(super::workbench::app_runs("export.variant_pdf"));
    let _ = ctx;

    let bare = dir.path().join("bare.pdf");
    let full = dir.path().join("full.pdf");
    assert!(d.app.export_variant_pdf_to(&bare, Some("Bare")).is_some());
    assert!(
        d.app
            .export_variant_pdf_to(&full, Some("With conveyor"))
            .is_some()
    );
    let pages = |p: &std::path::Path| {
        let bytes = std::fs::read(p).unwrap();
        bytes.windows(11).filter(|w| w == b"/Type /Page").count()
    };
    // The bare machine has one page fewer than the one with the conveyor.
    assert!(
        pages(&bare) < pages(&full),
        "{} vs {}",
        pages(&bare),
        pages(&full)
    );
    // The working project kept the Conveyor page throughout.
    let project = d.app.backend.document().unwrap().project();
    assert_eq!(
        project.pages().count(),
        3,
        "the new project's page plus the two"
    );
    // An unknown variant is a message, not a file.
    let none = dir.path().join("none.pdf");
    assert!(d.app.export_variant_pdf_to(&none, Some("Nope")).is_none());
    assert!(!none.exists());
}

#[test]
fn a_spec_sheet_generates_pages_from_a_shipped_macro_and_exports_back() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();

    let csv = dir.path().join("spec.csv");
    std::fs::write(
        &csv,
        "Macro,Page,Structure,power,voltage\n\
Motor starter DOL,Pumps,=A1,7.5,400\n\
Motor starter DOL,Pumps,=A1,15,480\n\
Nonexistent macro,Fans,=A1,1,\n",
    )
    .unwrap();

    assert!(super::workbench::app_runs("project.generate"));
    d.app.run_command(&ctx, "project.generate", None);
    assert!(d.app.wb.generate.is_some());
    let mut win = d.app.wb.generate.take().unwrap();
    d.app.load_spec(&mut win, csv.clone());
    assert_eq!(win.missing, vec!["Nonexistent macro".to_string()]);
    d.app.wb.generate = Some(win);
    d.frame();
    d.frame();
    // The missing macro is a problem with its line; nothing is generated.
    let project = d.app.backend.document().unwrap().project();
    assert_eq!(project.devices().count(), 0);

    // Fix the sheet: two starters.
    std::fs::write(
        &csv,
        "Macro,Page,Structure,power,voltage\n\
Motor starter DOL,Pumps,=A1,7.5,400\n\
Motor starter DOL,Pumps,=A1,15,480\n",
    )
    .unwrap();
    let mut win = d.app.wb.generate.take().unwrap();
    d.app.load_spec(&mut win, csv);
    d.app.wb.generate = Some(win);
    d.frame();
    d.frame();
    let project = d.app.backend.document().unwrap().project();
    // Preview only: still nothing in the project.
    assert_eq!(project.devices().count(), 0);

    let (result, changes) = {
        let win = d.app.wb.generate.as_ref().unwrap();
        let sheet = win.sheet_for_test().unwrap().clone();
        let macros = win.macros_for_test().clone();
        let lookup = move |n: &str| macros.get(&n.to_lowercase()).cloned();
        let assignment = options::current(d.app.backend.document().unwrap().project());
        d.app
            .backend
            .document_mut()
            .unwrap()
            .transact("Generate", |tx| {
                novo_workflow::generate::generate(
                    tx,
                    &sheet,
                    &lookup,
                    &assignment,
                    novo_workflow::generate::Mode::Add,
                )
            })
            .unwrap()
    };
    d.app.changed_outside_canvas(&changes);
    let g = result.unwrap();
    assert_eq!(g.placements.len(), 2);
    d.frame();

    // The rated current was calculated: 7.5 kW at 400 V is 13.5 A.
    let project = d.app.backend.document().unwrap().project();
    let mut motors: Vec<String> = project
        .devices()
        .map(|d| d.description.clone())
        .filter(|d| d.contains(" kW"))
        .collect();
    motors.sort();
    assert_eq!(motors, vec!["Motor 15 kW, 22.6 A", "Motor 7.5 kW, 13.5 A"]);

    // Export the spec sheet back: Excel can edit it, the pages regenerate.
    let xlsx = dir.path().join("spec.xlsx");
    assert!(d.app.export_spec_to(&xlsx).is_some());
    let rows = novo_interop::sheets::read_grid(&std::fs::read(&xlsx).unwrap()).unwrap();
    assert_eq!(rows[0][..3], ["Macro", "Page", "Structure"]);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[1][0], "Motor starter DOL");
}
