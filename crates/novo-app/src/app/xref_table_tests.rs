//! The contact table under a coil in the app: the terminal numbers on
//! either side of the symbol, the spare rows and the double-click jumps.

use eframe::egui;
use novo_canvas::Vec2;
use novo_canvas::model::item_id;
use novo_core::{AnyId, DeviceTag, PageId, Pin, Placement, Point, SymbolId, SymbolRole};

use super::gate1_tests::Drive;
use crate::actions::Action;

struct Table {
    d: Drive,
    _dir: tempfile::TempDir,
    pages: [PageId; 2],
    coil: SymbolId,
    /// NO 13-14, NC 21-22, both on page 2.
    contacts: [SymbolId; 2],
}

/// `-K1`: the coil on page 1, a normally open and a normally closed contact on page 2.
fn table() -> Table {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let page1 = d.app.current_page().unwrap();
    let doc = d.app.backend.document_mut().unwrap();
    let ((page2, coil, contacts), _) = doc
        .transact("Draw", |tx| {
            let p2 = tx.add_page("2")?;
            let k1 = tx.add_device(DeviceTag::parse("-K1")?, "")?;
            let coil = tx.place_symbol(
                page1,
                "iec/coils/relay",
                Placement::at(Point::new(60.0, 60.0)),
                vec![Pin::new("A1", 0.0, -8.0), Pin::new("A2", 0.0, 8.0)],
                SymbolRole::Device {
                    device: k1,
                    main: true,
                },
            )?;
            let mut contact = |key: &str, x: f64, pins: [&str; 2]| {
                tx.place_symbol(
                    p2,
                    key,
                    Placement::at(Point::new(x, 60.0)),
                    vec![Pin::new(pins[0], 0.0, -8.0), Pin::new(pins[1], 0.0, 8.0)],
                    SymbolRole::Device {
                        device: k1,
                        main: false,
                    },
                )
            };
            let no = contact("iec/contacts/no", 60.0, ["13", "14"])?;
            let nc = contact("iec/contacts/nc", 120.0, ["21", "22"])?;
            Ok((p2, coil, [no, nc]))
        })
        .unwrap();
    d.frame();
    Table {
        d,
        _dir: dir,
        pages: [page1, page2],
        coil,
        contacts,
    }
}

impl Table {
    fn labels_of(&self, symbol: SymbolId) -> Vec<String> {
        self.d
            .app
            .canvas
            .schematic_ref()
            .and_then(|v| v.scene())
            .and_then(|s| s.scene().get(item_id(AnyId::Symbol(symbol))))
            .map(|i| i.labels.iter().map(|l| l.text.clone()).collect())
            .unwrap_or_default()
    }

    /// The middle of the coil's table row that leads to `to`, in page coordinates.
    fn row_point(&self, to: SymbolId) -> Vec2 {
        let scene = self
            .d
            .app
            .canvas
            .schematic_ref()
            .and_then(|v| v.scene())
            .expect("the page scene");
        let (mut min, mut max) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
        for iy in 0..400 {
            for ix in 0..400 {
                let at = Vec2::new(ix as f32 * 0.5, iy as f32 * 0.5);
                if scene.note_row_at(at) == Some((self.coil, to)) {
                    (min, max) = (min.min(at), max.max(at));
                }
            }
        }
        assert!(min.x <= max.x, "no row of the table leads to {to:?}");
        (min + max) * 0.5
    }

    fn double_click_at(&mut self, world: Vec2) {
        for _ in 0..3 {
            self.d.frame();
        }
        let view = self.d.app.canvas.schematic().unwrap();
        let s = view.canvas.viewport.world_to_screen(world);
        let rect = self.d.app.wb.canvas_area.unwrap().1;
        let pos = rect.min + egui::vec2(s.x, s.y);
        let button = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame_at(&mut self.d, 50.0, vec![egui::Event::PointerMoved(pos)]);
        for (i, t) in [50.1, 50.15, 50.2, 50.25].into_iter().enumerate() {
            frame_at(&mut self.d, t, vec![button(i % 2 == 0)]);
        }
        frame_at(&mut self.d, 50.3, vec![]);
        frame_at(&mut self.d, 50.35, vec![]);
    }
}

fn frame_at(d: &mut Drive, time: f64, events: Vec<egui::Event>) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1600.0, 900.0),
        )),
        time: Some(time),
        events,
        ..Default::default()
    };
    let app = &mut d.app;
    let mut output = d.ctx.run_ui(input, |ui| app.show(ui));
    output.textures_delta.clear();
}

#[test]
fn the_table_shows_each_contacts_numbers_on_either_side_of_its_symbol() {
    let t = table();
    // 13 [symbol] 14, 21 [symbol] 22: one number per terminal, not 13-14.
    let labels = t.labels_of(t.coil);
    for want in ["13", "14", "21", "22"] {
        assert!(labels.iter().any(|l| l == want), "{want} in {labels:?}");
    }
    let joined = |l: &String| l.len() > 2 && l.chars().all(|c| c.is_ascii_digit() || c == '-');
    assert!(!labels.iter().any(joined), "{labels:?}");
}

#[test]
fn a_double_click_on_a_table_row_jumps_to_that_contact_and_alt_left_comes_back() {
    let mut t = table();
    let at = t.row_point(t.contacts[1]);
    t.double_click_at(at);
    t.d.frame();
    t.d.frame();
    assert_eq!(
        t.d.app.current_page(),
        Some(t.pages[1]),
        "the NC row's page"
    );
    assert_eq!(t.d.app.selection(), vec![AnyId::Symbol(t.contacts[1])]);
    t.d.key(egui::Key::ArrowLeft, egui::Modifiers::ALT);
    t.d.frame();
    assert_eq!(t.d.app.current_page(), Some(t.pages[0]));
}

#[test]
fn a_double_click_on_a_contacts_reference_jumps_to_its_coil() {
    let mut t = table();
    t.d.app.show_page(t.pages[1]);
    t.d.frame();
    t.d.frame();
    let item =
        t.d.app
            .canvas
            .schematic_ref()
            .and_then(|v| v.scene())
            .and_then(|s| s.scene().get(item_id(AnyId::Symbol(t.contacts[0]))))
            .cloned()
            .unwrap();
    // The reference to the coil is the last text of the contact.
    let note = item.labels.last().unwrap();
    assert!(
        note.text.starts_with("1."),
        "page 1, then the column: {}",
        note.text
    );
    t.double_click_at(note.bounds().center());
    assert_eq!(t.d.app.current_page(), Some(t.pages[0]));
    assert_eq!(t.d.app.selection(), vec![AnyId::Symbol(t.coil)]);
}

#[test]
fn spare_contacts_follow_the_device_and_the_setting() {
    let mut t = table();
    let k1 = {
        let doc = t.d.app.backend.document().unwrap();
        doc.project().devices().next().unwrap().id
    };
    t.d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Limits", |tx| {
            tx.set_property(k1, "contacts.no", Some(3i64.into()))
        })
        .unwrap();
    t.d.frame();
    t.d.frame();
    let labels = t.labels_of(t.coil);
    // 13-14 and 21-22 are drawn, so the third contact is 33-34 or the next free ordinal.
    assert!(labels.iter().any(|l| l == "spare"), "{labels:?}");
    assert!(labels.iter().any(|l| l == "33"), "{labels:?}");
    // Hidden in Preferences > Cross references.
    t.d.app.settings.xref_hide_spare = true;
    t.d.frame();
    t.d.frame();
    let labels = t.labels_of(t.coil);
    assert!(!labels.iter().any(|l| l == "spare"), "{labels:?}");
}

#[test]
fn a_mistyped_contact_count_stays_a_short_table() {
    let mut t = table();
    let k1 = {
        let doc = t.d.app.backend.document().unwrap();
        doc.project().devices().next().unwrap().id
    };
    t.d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Limits", |tx| {
            tx.set_property(k1, "contacts.no", Some(3_000_000i64.into()))
        })
        .unwrap();
    t.d.frame();
    t.d.frame();
    let labels = t.labels_of(t.coil);
    let spare = labels.iter().filter(|l| *l == "spare").count();
    assert_eq!(spare, novo_xref::MAX_SPARE_ROWS, "{spare} spare rows");
    assert!(
        labels
            .iter()
            .any(|l| l.starts_with("and ") && l.ends_with(" more spare")),
        "{labels:?}"
    );
}

impl Table {
    /// The opacity (0 to 255) of the first label of `symbol`'s item that reads `text`.
    fn alpha_of(&self, symbol: SymbolId, text: &str) -> Option<u8> {
        self.d
            .app
            .canvas
            .schematic_ref()
            .and_then(|v| v.scene())
            .and_then(|s| s.scene().get(item_id(AnyId::Symbol(symbol))))
            .and_then(|i| {
                i.labels
                    .iter()
                    .find(|l| l.text == text)
                    .map(|l| l.color.0[3])
            })
    }

    fn cycle_references(&mut self) {
        let ctx = self.d.ctx.clone();
        self.d.app.run_command(&ctx, "view.toggle_references", None);
        self.d.frame();
        self.d.frame();
    }

    /// Selects `symbol` the way a click does.
    fn select(&mut self, symbols: &[SymbolId]) {
        let view = self.d.app.canvas.schematic().expect("editor");
        view.canvas.selection.clear();
        view.canvas.selection.apply(
            symbols
                .iter()
                .map(|&s| item_id(AnyId::Symbol(s)))
                .collect::<Vec<_>>(),
            novo_canvas::SelectOp::Replace,
        );
        self.d.frame();
        self.d.frame();
    }
}

#[test]
fn references_cycle_full_faded_hidden_and_the_choice_is_kept() {
    let mut t = table();
    assert_eq!(t.alpha_of(t.coil, "13"), Some(255), "full to begin with");
    t.cycle_references();
    assert_eq!(t.d.app.settings.xref_view, "faded");
    let faded = t.alpha_of(t.coil, "13").expect("still drawn");
    assert!(faded < 128, "faded: {faded}");
    // Same place, same rows: only the opacity changed.
    assert!(t.labels_of(t.coil).iter().any(|l| l == "14"));
    t.cycle_references();
    assert_eq!(t.d.app.settings.xref_view, "hidden");
    assert!(
        t.labels_of(t.coil).iter().all(|l| l != "13" && l != "14"),
        "{:?}",
        t.labels_of(t.coil)
    );
    // A contact's own reference (the coil location) goes too.
    assert!(
        !t.labels_of(t.contacts[0])
            .iter()
            .any(|l| l.starts_with("1."))
    );
    t.cycle_references();
    assert_eq!(t.d.app.settings.xref_view, "full");
    assert_eq!(t.alpha_of(t.coil, "13"), Some(255));
}

#[test]
fn faded_references_come_back_for_the_selected_coil() {
    let mut t = table();
    t.cycle_references();
    assert!(t.alpha_of(t.coil, "13").unwrap() < 128);
    t.select(&[t.coil]);
    assert_eq!(t.alpha_of(t.coil, "13"), Some(255), "selected: clear");
    t.select(&[]);
    assert!(
        t.alpha_of(t.coil, "13").unwrap() < 128,
        "deselected: faded again"
    );
}

#[test]
fn the_pdf_prints_references_in_full_whatever_the_screen_shows() {
    let mut t = table();
    t.cycle_references();
    t.cycle_references();
    assert_eq!(t.d.app.settings.xref_view, "hidden");
    let pages = t.d.app.xref_notes_by_page();
    let coil_note = pages[&t.pages[0]]
        .iter()
        .find(|n| n.symbol == t.coil)
        .expect("the coil's table is printed");
    assert!(!coil_note.rows.is_empty());
    assert!(pages.values().flatten().all(|n| !n.faded));
}

#[test]
fn shift_x_is_the_shortcut_and_the_status_bar_offers_the_way_back() {
    let mut t = table();
    assert!(
        t.d.app
            .mode_chips()
            .iter()
            .all(|c| c.command != "view.toggle_references")
    );
    t.d.key(egui::Key::X, egui::Modifiers::SHIFT);
    t.d.frame();
    assert_eq!(t.d.app.settings.xref_view, "faded");
    let chip =
        t.d.app
            .mode_chips()
            .into_iter()
            .find(|c| c.command == "view.toggle_references")
            .expect("a chip while references are not shown in full");
    assert!(chip.label.contains("faded"), "{}", chip.label);
    assert!(novo_workflow::commands::is_known(chip.command));
}
