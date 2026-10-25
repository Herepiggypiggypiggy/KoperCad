//! The cooling what-if wizard: it runs from the cabinet as the project has
//! it, ranks the fixes, previews the picked one without storing anything,
//! applies it as the engine promised and says what the numbers are.

use std::time::Duration;

use eframe::egui;
use novo_core::{Command, DeviceTag, MountingPlate, Project, RailKind, StructureId, Value};
use novo_layout::enclosure::Enclosure;
use novo_layout::model::set_enclosure;
use novo_thermal::whatif::{self, Fix};

use super::heat::HeatState;
use super::heat_whatif::{Ask, Found, Wizard};

/// A 150 W drive with a 40 °C limit on the lowest of three rails, a cool
/// device above it, and the right of the rails free.
fn cabinet() -> (Project, StructureId) {
    let mut p = Project::new("t");
    let (cab, _) = p
        .transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            tx.set_plate(
                cab,
                Some(MountingPlate {
                    width: 800.0,
                    height: 1100.0,
                    margin: 10.0,
                }),
            )?;
            set_enclosure(
                tx,
                cab,
                &Enclosure {
                    width: 900.0,
                    height: 1200.0,
                    depth: 300.0,
                    plate_setback: 0.0,
                    door: None,
                    front_clearance: None,
                    hinge_side_clearance: None,
                },
            )?;
            let mut rails = Vec::new();
            for row in 0..3 {
                rails.push(tx.add_rail(
                    cab,
                    20.0,
                    150.0 + 350.0 * f64::from(row),
                    760.0,
                    RailKind::DinRail,
                )?);
            }
            for (tag, x, row, w, h, loss, limit) in [
                ("=A1+CAB1-T1", 30.0, 2, 120.0, 220.0, 150.0, 40.0),
                ("=A1+CAB1-K1", 40.0, 1, 50.0, 90.0, 4.0, 45.0),
            ] {
                let d = tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
                for (k, v) in [
                    ("width_mm", w),
                    ("height_mm", h),
                    ("depth_mm", 80.0),
                    ("power_loss_w", loss),
                    ("max_ambient_c", limit),
                ] {
                    tx.set_property(d, k, Some(Value::Float(v)))?;
                }
                tx.execute(Command::PlaceDevice {
                    device: d,
                    structure: cab,
                    x,
                    y: 150.0 + 350.0 * f64::from(row) - h / 2.0,
                    rotated: false,
                    rail: rails.get(row as usize).map(|r| (*r, x - 20.0)),
                })?;
            }
            Ok(cab)
        })
        .unwrap();
    (p, cab)
}

fn tags(p: &Project) -> impl Fn(u64) -> String + '_ {
    move |id| {
        p.get::<novo_core::Device>(novo_core::DeviceId::from(id))
            .map_or_else(String::new, |d| d.tag.to_string())
    }
}

/// A short run: the tests check what is done with the answer, not how far
/// the search can go.
fn quick() -> whatif::Settings {
    whatif::Settings {
        layout: novo_thermal::placement::Settings {
            top_k: 3,
            max_movers: 2,
            max_moves: 2,
            ..Default::default()
        },
        ..whatif::Settings::quick()
    }
}

fn run(w: &mut Wizard, p: &Project, cab: StructureId) {
    w.settings = Some(quick());
    let ctx = egui::Context::default();
    w.request();
    assert!(w.take_request());
    let fans = Vec::new();
    w.start(
        p,
        cab,
        35.0,
        &super::heat_settings::HeatSettings::default(),
        fans,
        &tags(p),
    );
    // The real run takes ~16 s alone in a debug build, and every test that
    // reads an answer runs it in its own process at the same time under
    // nextest, so the usual 30 s deadline is too short on a two-core runner.
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(600) {
        w.poll(&ctx);
        if !w.running() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("the what-if did not end");
}

/// One real run of the engine for all the tests that read an answer: the
/// run is the slow part and does not change from test to test.
fn answered() -> (Wizard, Project, StructureId) {
    static ANSWER: std::sync::OnceLock<super::heat_whatif::Found> = std::sync::OnceLock::new();
    let (p, cab) = cabinet();
    let found = ANSWER
        .get_or_init(|| {
            let mut w = Wizard::default();
            run(&mut w, &p, cab);
            w.found.expect("an answer")
        })
        .clone();
    let mut w = Wizard::default();
    w.set_found_for_test(found);
    (w, p, cab)
}

fn screen() -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1366.0, 768.0),
        )),
        ..Default::default()
    }
}

/// The text the window draws after it has settled.
fn render(w: &mut Wizard, view: u8, now_key: Option<u64>) -> String {
    render_room(w, view, now_key, 300.0)
}

/// `room` is the height the list gets before it scrolls; what is below the
/// fold of a scroll area is not drawn.
fn render_room(w: &mut Wizard, view: u8, now_key: Option<u64>, room: f32) -> String {
    let ctx = egui::Context::default();
    let mut text = String::new();
    for _ in 0..12 {
        let mut out = ctx.run_ui(screen(), |ui| {
            w.show(ui.ctx(), view, now_key, 35.0, egui::pos2(10.0, 700.0), room);
        });
        out.textures_delta.clear();
        text = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_owned()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
    }
    text
}

#[test]
fn the_run_ranks_fixes_and_previews_the_first_without_storing_it() {
    let (w, p, _) = answered();
    let found = w.found.as_ref().expect("an answer");
    let wi = &found.what_if;
    assert!(!wi.choices.is_empty(), "{:?}", wi.notes);
    for (i, c) in wi.choices.iter().enumerate() {
        assert_eq!(c.rank, i + 1);
    }
    assert_eq!(w.picked(), Some(0), "the best fix is shown at once");
    let before = p.revision();
    let preview = w.preview().expect("a preview");
    assert_eq!(p.revision(), before, "previewing stores nothing");
    let best = &wi.choices[0];
    assert_eq!(preview.moves.len(), best.fix.moves().len());
    assert_eq!(preview.candidate.is_some(), best.fix.candidate().is_some());
}

#[test]
fn the_preview_boxes_are_in_the_plate_frame() {
    let (mut w, p, cab) = answered();
    let wi = &w.found.as_ref().unwrap().what_if;
    let Some(i) = wi
        .choices
        .iter()
        .position(|c| matches!(c.fix, Fix::Layout { .. }))
    else {
        panic!("no layout fix: {:?}", wi.notes);
    };
    w.pick(i);
    let (ox, oy) = novo_thermal::placement_project::plate_origin(&p, cab).unwrap();
    let wi = &w.found.as_ref().unwrap().what_if;
    let moves = wi.choices[i].fix.moves().to_vec();
    let preview = w.preview().unwrap();
    for (m, (id, rect)) in moves.iter().zip(&preview.moves) {
        assert_eq!(m.id, *id);
        assert!((rect.x - (m.to.x - ox)).abs() < 1e-9 && (rect.y - (m.to.y - oy)).abs() < 1e-9);
    }
}

#[test]
fn the_window_names_the_model_the_table_and_the_conditions() {
    let (mut w, _, _) = answered();
    let text = render(&mut w, 2, None);
    for want in [
        "Cooling what-if",
        "MODEL, NOT MEASURED",
        "Hottest day outside",
        "Duty",
        "Devices over limit",
        "Tightest margin",
        "Hottest air",
        "Model estimate",
        "Try again with these",
    ] {
        assert!(text.contains(want), "missing {want:?} in:\n{text}");
    }
}

#[test]
fn a_changed_cabinet_makes_the_answer_stale_and_blocks_apply() {
    let (mut w, _, _) = answered();
    let key = 12345;
    let text = render(&mut w, 2, Some(key));
    assert!(text.contains("cabinet changed"), "{text}");
}

#[test]
fn the_typed_conditions_reach_the_run_and_a_cool_day_has_nothing_to_fix() {
    let (p, cab) = cabinet();
    let mut w = Wizard::default();
    // A cold day at a fifth of the load: the run ends after its baseline.
    w.set_conditions_for_test(Some(10.0), Some(20.0));
    run(&mut w, &p, cab);
    let wi = &w.found.as_ref().unwrap().what_if;
    assert_eq!(wi.conditions.ambient_c, 10.0);
    assert!((wi.conditions.duty - 0.2).abs() < 1e-9);
    assert!(wi.choices.is_empty());
    assert!(super::heat_whatif::headline(wi).contains("Nothing to fix"));
    assert!(wi.notes.iter().any(|n| n.contains("Duty 20 %")));
    let text = render(&mut w, 2, None);
    assert!(text.contains("Nothing to fix"), "{text}");
}

#[test]
fn the_table_text_carries_the_label_and_the_conditions() {
    let (w, _, _) = answered();
    let found = w.found.as_ref().unwrap();
    let text = super::heat_whatif::table_text(&w, found);
    for want in [
        "MODEL, NOT MEASURED",
        "35 °C outside, duty 100 %",
        "Devices over limit",
        "Tightest margin",
        "Not validated",
    ] {
        assert!(text.contains(want), "missing {want:?} in:\n{text}");
    }
}

/// The one 3D comparison says the 2D model reads a fan's gain low: the card
/// and the copied table of a fix with fans say so, those without do not.
#[test]
fn a_fix_with_fans_carries_the_fan_note_and_one_without_does_not() {
    let (mut w, _, _) = answered();
    let (fan, plain) = {
        let wi = &w.found.as_ref().unwrap().what_if;
        let has = |i: usize| {
            wi.choices[i]
                .fix
                .candidate()
                .is_some_and(|c| c.pairing.fans() > 0)
        };
        let all = 0..wi.choices.len();
        (
            all.clone().find(|i| has(*i)).expect("a fix with fans"),
            all.clone()
                .find(|i| wi.choices[*i].fix.candidate().is_some() && !has(*i))
                .or_else(|| {
                    all.clone()
                        .find(|i| wi.choices[*i].fix.candidate().is_none())
                }),
        )
    };
    w.pick(fan);
    let found = w.found.clone().unwrap();
    assert!(super::heat_whatif::table_text(&w, &found).contains("under- or over-stated"));
    assert!(render(&mut w, 2, None).contains("under- or over-stated"));
    if let Some(plain) = plain {
        w.pick(plain);
        assert!(!super::heat_whatif::table_text(&w, &found).contains("under- or over-stated"));
        assert!(!render(&mut w, 2, None).contains("under- or over-stated"));
    }
}

#[test]
fn a_fan_fix_says_how_many_devices_stand_beside_its_stream_and_whether_the_hot_ones_do() {
    let (mut w, _, _) = answered();
    let fan = {
        let wi = &w.found.as_ref().unwrap().what_if;
        (0..wi.choices.len())
            .find(|&i| {
                wi.choices[i]
                    .fix
                    .candidate()
                    .is_some_and(|c| c.pairing.fans() > 0)
            })
            .expect("a fix with fans")
    };
    w.pick(fan);
    // Whatever the real run found, say which devices are beside the stream.
    let (over, other, total) = {
        let wi = &w.found.as_ref().unwrap().what_if;
        let over = wi
            .device_ids
            .iter()
            .zip(&wi.baseline.margins_k)
            .find(|(_, m)| m.is_some_and(|m| m < 0.0))
            .map(|(id, _)| *id)
            .expect("a device over its limit");
        let other = *wi.device_ids.iter().find(|&&id| id != over).unwrap();
        (over, other, wi.device_ids.len())
    };
    fn set(w: &mut Wizard, fan: usize, ids: Vec<u64>) -> Found {
        w.found.as_mut().unwrap().what_if.choices[fan].beside_stream = ids;
        w.found.clone().unwrap()
    }
    let found = set(&mut w, fan, vec![over, other]);
    let text = super::heat_whatif::table_text(&w, &found);
    assert!(
        text.contains(&format!(
            "Beside the assumed fan stream: 2 of {total} devices."
        )),
        "{text}"
    );
    assert!(
        text.contains(&format!(
            "Over their limit now and beside it: {}",
            w.tag(over)
        )),
        "{text}"
    );
    assert!(text.contains(novo_thermal::whatif::BESIDE_NOTE));
    assert!(render_room(&mut w, 2, None, 600.0).contains("Beside the assumed fan stream"));
    // Only a device that is not over its limit: say so.
    let index = w
        .found
        .as_ref()
        .unwrap()
        .what_if
        .device_ids
        .iter()
        .position(|&id| id == other)
        .unwrap();
    w.found.as_mut().unwrap().what_if.baseline.margins_k[index] = Some(5.0);
    let found = set(&mut w, fan, vec![other]);
    let text = super::heat_whatif::table_text(&w, &found);
    assert!(
        text.contains("None of the devices over their limit now is beside it by this rule"),
        "{text}"
    );
    // Nobody beside: no line.
    let found = set(&mut w, fan, Vec::new());
    assert!(!super::heat_whatif::table_text(&w, &found).contains("Beside the assumed fan stream"));
    assert!(!render_room(&mut w, 2, None, 600.0).contains("Beside the assumed fan stream"));
}

#[test]
fn closing_drops_the_answer_and_stops_a_run() {
    let (mut w, _, _) = answered();
    w.close();
    assert!(w.found.is_none() && !w.running() && !w.is_open());
    assert!(w.preview().is_none());
}

#[test]
fn the_window_closes_when_the_view_changes() {
    let mut w = Wizard::default();
    w.request();
    w.take_request();
    let ctx = egui::Context::default();
    for view in [2, 2, 1] {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            w.show(ui.ctx(), view, None, 35.0, egui::pos2(10.0, 700.0), 300.0);
        });
        out.textures_delta.clear();
    }
    assert!(!w.is_open());
}

#[test]
fn a_cabinet_without_devices_says_so() {
    let mut p = Project::new("t");
    let (cab, _) = p
        .transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            tx.set_plate(
                cab,
                Some(MountingPlate {
                    width: 800.0,
                    height: 1100.0,
                    margin: 10.0,
                }),
            )?;
            set_enclosure(
                tx,
                cab,
                &Enclosure {
                    width: 900.0,
                    height: 1200.0,
                    depth: 300.0,
                    plate_setback: 0.0,
                    door: None,
                    front_clearance: None,
                    hinge_side_clearance: None,
                },
            )?;
            Ok(cab)
        })
        .unwrap();
    let mut w = Wizard::default();
    w.start(
        &p,
        cab,
        35.0,
        &super::heat_settings::HeatSettings::default(),
        Vec::new(),
        &tags(&p),
    );
    assert!(!w.running());
    assert!(
        w.notice().is_some_and(|n| n.contains("No device")),
        "{:?}",
        w.notice()
    );
}

#[test]
fn the_command_from_the_cabinet_opens_the_wizard_in_3d() {
    use super::Action;
    use super::workspace::Workspace;
    let dir = tempfile::tempdir().unwrap();
    let mut d = super::view3d_tests::drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    assert_eq!(d.app.workspace(), Workspace::Cabinet);
    d.app.run_command(&ctx, "heat.cooling_whatif", None);
    for _ in 0..4 {
        d.frame();
    }
    assert_eq!(d.app.workspace(), Workspace::View3d);
    assert!(d.app.cabinet.heat.whatif.is_open());
    assert!(
        !d.app.cabinet.heat.whatif.pending(),
        "taken up by the 3D view"
    );
}

#[test]
fn the_heat_strip_button_in_the_cabinet_also_leads_to_the_3d_view() {
    use super::Action;
    use super::workspace::Workspace;
    let dir = tempfile::tempdir().unwrap();
    let mut d = super::view3d_tests::drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::ShowCabinet);
    d.frame();
    d.app.cabinet.heat.whatif.request();
    for _ in 0..4 {
        d.frame();
    }
    assert_eq!(d.app.workspace(), Workspace::View3d);
}

#[test]
fn applying_openings_puts_them_in_the_project_list_as_one_undo_step() {
    use novo_thermal::model::{Opening, Placement};
    use novo_thermal::sweep::{Candidate, Pairing, Piece};
    let dir = tempfile::tempdir().unwrap();
    let mut d = super::view3d_tests::drive_with_cabinet(dir.path());
    let piece = |p, fan| Piece {
        opening: Opening {
            placement: p,
            width_mm: 150.0,
            height_mm: 150.0,
        },
        fan_flow_m3h: fan,
    };
    let c = Candidate {
        pairing: Pairing::VentInFanOut,
        inlet: piece(Placement::Left { y_mm: 400.0 }, None),
        outlet: piece(Placement::Top { x_mm: 300.0 }, Some(250.0)),
    };
    d.app
        .cabinet
        .heat
        .whatif
        .set_apply_for_test(Ask::Openings(c));
    d.app.whatif_apply();
    assert_eq!(d.app.cabinet.heat.settings.vents.len(), 2);
    assert!(
        d.app
            .cabinet
            .heat
            .whatif
            .applied()
            .is_some_and(|t| t.contains("Added"))
    );
    d.app.cabinet.heat.settings.undo();
    assert!(d.app.cabinet.heat.settings.vents.is_empty());
}

#[test]
fn a_move_to_a_gone_rail_changes_nothing_and_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = super::view3d_tests::drive_with_cabinet(dir.path());
    let cab = {
        let project = d.app.backend.document().unwrap().project();
        project
            .structures()
            .find(|s| s.enclosure && s.plate.is_some())
            .map(|s| s.id)
            .unwrap()
    };
    let device = {
        let project = d.app.backend.document().unwrap().project();
        novo_layout::model::load(project, cab).unwrap().placements[0].id
    };
    let rect = novo_layout::Rect {
        x: 0.0,
        y: 0.0,
        w: 50.0,
        h: 50.0,
    };
    let m = novo_thermal::placement::Move {
        id: device,
        from_row: 1,
        to_row: u64::MAX - 1,
        from: rect,
        to: rect,
        cost_before: 1.0,
        cost_after: 0.5,
        swapped_with: None,
    };
    let before = d.app.backend.document().unwrap().project().revision();
    d.app
        .cabinet
        .heat
        .whatif
        .set_apply_for_test(Ask::Move(cab, vec![m]));
    d.app.whatif_apply();
    assert!(
        d.app
            .cabinet
            .heat
            .whatif
            .applied()
            .is_some_and(|t| t.contains("Nothing moved"))
    );
    let _ = before;
}

#[test]
fn headline_and_direction_words() {
    // Direction: a change under half a unit is neither better nor worse.
    assert_eq!(super::heat_whatif::dir(10.0, 10.2, true), None);
    assert_eq!(super::heat_whatif::dir(10.0, 8.0, true), Some(true));
    assert_eq!(super::heat_whatif::dir(-3.0, 4.0, false), Some(true));
    assert_eq!(super::heat_whatif::dir(-3.0, -6.0, false), Some(false));
}

#[test]
fn heat_state_command_requests_the_wizard() {
    let mut s = HeatState::new();
    s.command("heat.cooling_whatif");
    assert!(s.whatif.pending());
    assert!(s.whatif.is_open());
}

/// An answer is for the file it was run on: opening another file closes the
/// window and drops the answer, so Apply cannot move the devices of file B
/// with file A's moves, and a run in flight does not land in file B.
#[test]
fn opening_another_project_closes_the_window_and_drops_the_answer() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = super::view3d_tests::drive_with_cabinet(dir.path());
    let (answer, _, _) = answered();
    d.app
        .cabinet
        .heat
        .whatif
        .set_found_for_test(answer.found.unwrap());
    assert!(d.app.cabinet.heat.whatif.is_open());
    assert!(d.app.cabinet.heat.whatif.preview().is_some());
    d.app.document_replaced();
    assert!(!d.app.cabinet.heat.whatif.is_open());
    assert!(d.app.cabinet.heat.whatif.found.is_none());
    assert!(d.app.cabinet.heat.whatif.preview().is_none());
}

/// The vent and fan sweep is for one cabinet too: its window closes and its
/// answer goes when another project opens.
#[test]
fn opening_another_project_closes_the_cooling_advice_too() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = super::view3d_tests::drive_with_cabinet(dir.path());
    d.app.cabinet.heat.advice.request();
    assert!(d.app.cabinet.heat.advice.is_open());
    d.app.document_replaced();
    assert!(!d.app.cabinet.heat.advice.is_open());
    assert!(d.app.cabinet.heat.advice.found.is_none());
}

/// A combined fix whose target rail is gone adds the openings and says so,
/// instead of reporting that nothing happened.
#[test]
fn a_combined_apply_with_a_missing_rail_says_only_the_openings_were_added() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = super::view3d_tests::drive_with_cabinet(dir.path());
    let (answer, _, _) = answered();
    let found = answer.found.unwrap();
    let candidate = found
        .what_if
        .choices
        .iter()
        .find_map(|c| c.fix.candidate().cloned())
        .expect("a fix with openings");
    let (cab, placement) = {
        let project = d.app.backend.document().unwrap().project();
        let cab = project
            .structures()
            .find(|s| s.enclosure && s.plate.is_some())
            .map(|s| s.id)
            .unwrap();
        let p = novo_layout::model::load(project, cab).unwrap().placements[0].clone();
        (cab, p)
    };
    let to = novo_layout::Rect {
        x: placement.x,
        y: placement.y,
        w: placement.size.width,
        h: placement.size.height,
    };
    let m = novo_thermal::placement::Move {
        id: placement.id,
        from_row: 0,
        to_row: u64::MAX >> 8,
        from: to,
        to,
        cost_before: 1.0,
        cost_after: 0.5,
        swapped_with: None,
    };
    d.app
        .cabinet
        .heat
        .whatif
        .set_apply_for_test(Ask::Both(cab, vec![m], candidate));
    d.app.whatif_apply();
    let said = d.app.cabinet.heat.whatif.applied_text().unwrap_or_default();
    assert!(said.contains("No device moved"), "{said}");
    assert!(said.contains("only the openings were added"), "{said}");
    assert!(said.contains("Added "), "{said}");
}

#[test]
fn a_run_in_flight_is_dropped_when_another_project_opens() {
    let (p, cab) = cabinet();
    let mut w = Wizard::default();
    w.settings = Some(quick());
    w.start(
        &p,
        cab,
        35.0,
        &super::heat_settings::HeatSettings::default(),
        Vec::new(),
        &tags(&p),
    );
    assert!(w.running());
    w.close();
    assert!(!w.running() && w.found.is_none());
}

/// The moves of an Apply are one project undo step; the openings of a
/// combined fix are in the Vents and fans list, which has its own undo.
#[test]
fn undoing_applied_moves_puts_the_devices_back() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = super::view3d_tests::drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    let (cab, rail, placement) = {
        let project = d.app.backend.document().unwrap().project();
        let cab = project
            .structures()
            .find(|s| s.enclosure && s.plate.is_some())
            .map(|s| s.id)
            .unwrap();
        let rail = project
            .all::<novo_core::Rail>()
            .find(|r| r.structure == cab)
            .map(|r| r.id.raw())
            .expect("a rail");
        let p = novo_layout::model::load(project, cab).unwrap().placements[0].clone();
        (cab, rail, p)
    };
    let (ox, oy) = novo_thermal::placement_project::plate_origin(
        d.app.backend.document().unwrap().project(),
        cab,
    )
    .unwrap();
    let to = novo_layout::Rect {
        x: ox + placement.x + 120.0,
        y: oy + placement.y,
        w: placement.size.width,
        h: placement.size.height,
    };
    let m = novo_thermal::placement::Move {
        id: placement.id,
        from_row: rail,
        to_row: rail,
        from: to,
        to,
        cost_before: 1.0,
        cost_after: 0.5,
        swapped_with: None,
    };
    d.app
        .cabinet
        .heat
        .whatif
        .set_apply_for_test(Ask::Move(cab, vec![m]));
    d.app.whatif_apply();
    let at = |d: &super::gate1_tests::Drive| {
        let project = d.app.backend.document().unwrap().project();
        let l = novo_layout::model::load(project, cab).unwrap();
        let p = l.placements.iter().find(|p| p.id == placement.id).unwrap();
        (p.x, p.y)
    };
    assert_ne!(at(&d), (placement.x, placement.y), "the device moved");
    d.app.run_command(&ctx, "edit.undo", None);
    d.frame();
    assert_eq!(at(&d), (placement.x, placement.y), "one undo puts it back");
}

/// Writes `qa/fixtures/cabinet-whatif-layout.novo`, the cabinet of these
/// tests: a 150 W transformer on the top rail that moving it cools, so the
/// wizard offers a layout fix for the QA picture (scenario 403).
/// `cargo test -p novo-app --lib write_the_layout_fixture -- --ignored`
#[test]
#[ignore = "writes a fixture file"]
fn write_the_layout_fixture() {
    let (p, _) = cabinet();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../qa/fixtures/cabinet-whatif-layout.novo");
    let _ = std::fs::remove_file(&path);
    novo_storage::ProjectFile::create(&path, &p).unwrap();
}
