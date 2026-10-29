//! Drive run (`sim.drive_run`, Simulate > Drive run): the live circuit drives the frequency
//! drive and the induction motor of a travel axis through the fixed-step loop of `novo-machine`
//! (lane L5a part 3, design `docs/design/electromechanical-loop.md`).
//!
//! A motor whose property sheet has a moving mass ("Travel drive", `trav_*` properties) is a
//! travel axis. With Live switched on, press the radio buttons of the drawing: the relays pull
//! in, the drive reads its inputs (forward, reverse, step 2), ramps the frequency, the motor
//! follows with its current and power fed back, and a limit switch bound in the property sheet
//! opens its drawn contact at its position. Badges on the motor show frequency, speed, current
//! and position.

use eframe::egui;
use novo_core::{PageId, Project, Value};
use novo_machine::cosim::StepConfig;
use novo_machine::travel_loop::{TravelLoop, TravelModule};
use novo_machine::travel_spec;
use novo_sim::is_motor;

use super::hoist_run::LiveCircuit;
use super::live::LiveView;
use super::{Level, NovoApp};

/// Most ticks run in one frame, so a slow frame cannot snowball.
const MAX_TICKS: usize = 40;

/// What the Simulate > Drive run window remembers.
#[derive(Default)]
pub(crate) struct DriveRun {
    pub(super) open: bool,
    active: Option<Active>,
    /// Why the last start did not run, or what it found.
    notice: Vec<(Level, String)>,
}

struct Active {
    lp: TravelLoop,
    acc: f64,
}

/// The travel axes of a project: every motor with a moving mass set. An axis whose numbers the
/// engine refuses is reported and left out. The notes are what the motor fit assumed.
pub(super) fn axes_of(project: &Project) -> (Vec<TravelModule>, Vec<(Level, String)>) {
    let mut modules = Vec::new();
    let mut notes = Vec::new();
    for device in project.devices() {
        if !device.properties.contains_key(travel_spec::TRAVEL_KEY) {
            continue;
        }
        let tag = device.tag.to_string();
        let get = |key: &str| device.properties.get(key).map(|v: &Value| v.to_string());
        match travel_spec::travel_from_properties(&tag, &get) {
            Ok(Some((module, fit))) => {
                modules.push(module);
                notes.extend(
                    fit.into_iter()
                        .map(|n| (Level::Info, format!("{tag}: {n}"))),
                );
            }
            Ok(None) => {}
            Err(why) => notes.push((Level::Warning, format!("{tag} is left out: {why}"))),
        }
    }
    (modules, notes)
}

/// Steps the loop for one frame. Returns whether a run is active (the frame then reads the
/// circuit without advancing its own clock again).
pub(super) fn advance(
    run: &mut DriveRun,
    live: &mut LiveView,
    project: &Project,
    frame_dt: f64,
) -> bool {
    let Some(active) = run.active.as_mut() else {
        return false;
    };
    if !live.on {
        return false;
    }
    let dt = active.lp.config.dt_s;
    active.acc += frame_dt.clamp(0.0, 0.1);
    let mut circuit = LiveCircuit::new(live, project);
    let mut ticks = 0;
    while active.acc >= dt && ticks < MAX_TICKS {
        active.lp.step(&mut circuit);
        active.acc -= dt;
        ticks += 1;
    }
    if ticks == MAX_TICKS {
        active.acc = 0.0;
    }
    true
}

/// A one-line reading of an axis, for the badge and the window.
pub(super) fn reading(module: &TravelModule) -> String {
    if let Some(why) = module.fault() {
        return why.to_owned();
    }
    let Some(s) = module.snapshot() else {
        return "waiting".to_owned();
    };
    format!(
        "{:.1} Hz \u{b7} {:.0} rpm \u{b7} {:.1} A \u{b7} {:.2} m/s \u{b7} {:.2} m",
        s.drive.frequency_hz.abs(),
        module.rpm().abs(),
        s.motor.stator_current_a,
        s.speed_m_s.abs(),
        s.position_m
    )
}

/// Badges on the motors of the axes, on the page shown.
pub(super) fn paint_badges(
    ctx: &egui::Context,
    canvas: egui::Rect,
    to_screen: &dyn Fn(egui::Vec2) -> egui::Pos2,
    project: &Project,
    library: Option<&novo_symbols::Library>,
    page: PageId,
    run: &DriveRun,
) {
    let Some(active) = run.active.as_ref() else {
        return;
    };
    let painter = ctx
        .layer_painter(egui::LayerId::new(
            egui::Order::Middle,
            egui::Id::new("drive_badges"),
        ))
        .with_clip_rect(canvas);
    let palette = crate::theme::ui_palette();
    for module in &active.lp.modules {
        for symbol in project.symbols().filter(|s| s.page == page && is_motor(s)) {
            let tag = symbol
                .device()
                .and_then(|d| project.device(d))
                .map(|d| d.tag.to_string());
            if tag.as_deref() != Some(module.id.as_str()) {
                continue;
            }
            let Some((at, r)) = super::live_anim::body(library, symbol) else {
                continue;
            };
            let centre = to_screen(at);
            let galley = painter.layout_no_wrap(
                reading(module),
                egui::FontId::proportional(12.0),
                palette.text_strong,
            );
            let pos = centre + egui::vec2(-galley.size().x / 2.0, r * 3.0 + 6.0);
            let rect = egui::Rect::from_min_size(pos, galley.size()).expand2(egui::vec2(6.0, 3.0));
            painter.rect_filled(rect, 4.0, palette.raised);
            painter.rect_stroke(
                rect,
                4.0,
                egui::Stroke::new(1.0, palette.border_control),
                egui::StrokeKind::Inside,
            );
            painter.galley(pos, galley, palette.text_strong);
        }
    }
    if active.lp.modules.iter().any(|m| m.snapshot().is_some()) {
        ctx.request_repaint();
    }
}

impl NovoApp {
    /// `sim.drive_run`: opens or closes the window.
    pub(super) fn toggle_drive_run(&mut self) {
        self.wb.drive_run.open ^= true;
    }

    pub(super) fn drive_run_window(&mut self, ctx: &egui::Context) {
        if !self.wb.drive_run.open {
            return;
        }
        let Some(doc) = self.backend.document() else {
            return;
        };
        let (axes, _) = axes_of(doc.project());
        let live_on = self.wb.live.on;
        let mut open = true;
        let (mut start, mut stop, mut live_toggle) = (false, false, false);
        let run = &self.wb.drive_run;
        let rows: Vec<(String, String)> = run.active.as_ref().map_or_else(Vec::new, |a| {
            a.lp.modules
                .iter()
                .map(|m| (m.id.clone(), reading(m)))
                .collect()
        });
        let running = run.active.is_some();
        let notice = run.notice.clone();
        let names: Vec<String> = axes.iter().map(|a| a.id.clone()).collect();
        // Foreground: the marks of the live view (contacts, coil rings) are painted on the Middle
        // layer and must not run through the notes.
        super::panel::dialog(ctx, "Drive run", [560.0, 560.0])
            .default_pos([ctx.content_rect().max.x, 60.0])
            .order(egui::Order::Foreground)
            .open(super::panel::esc(ctx, &mut open))
            .show(ctx, |ui| {
                ui.label(
                    "Press the radio buttons of the drawing in the live view: the relays pull in, \
                     the drive ramps its frequency, the motor follows with its current fed back, \
                     and the end limits you bound open their contacts at their position.",
                );
                ui.add_space(4.0);
                if names.is_empty() {
                    ui.weak(
                        "No travel axis yet. Select a motor and enter its moving mass under \
                         Travel drive in the property sheet.",
                    );
                } else {
                    ui.label(format!("Travel axes in this project: {}", names.join(", ")));
                }
                ui.horizontal(|ui| {
                    if !live_on && ui.button("Switch on Live").clicked() {
                        live_toggle = true;
                    }
                    let can = live_on && !names.is_empty();
                    if running {
                        if ui.button("Stop").clicked() {
                            stop = true;
                        }
                    } else if ui.add_enabled(can, egui::Button::new("Start")).clicked() {
                        start = true;
                    }
                    if !live_on {
                        ui.weak("The run reads the live view (Shift+L).");
                    }
                });
                // Solid behind the notes: marks of the drawing must not run through them.
                let palette = crate::theme::ui_palette();
                let fit: Vec<&str> = notice
                    .iter()
                    .filter(|(l, _)| matches!(l, Level::Info))
                    .map(|(_, t)| t.as_str())
                    .collect();
                egui::Frame::new().fill(palette.raised).show(ui, |ui| {
                    for (level, text) in notice.iter().filter(|(l, _)| !matches!(l, Level::Info)) {
                        let w = match level {
                            Level::Error => novo_theme::widgets::Level::Error,
                            _ => novo_theme::widgets::Level::Warning,
                        };
                        novo_theme::widgets::banner(ui, w, text, |_| {});
                    }
                    if !fit.is_empty() {
                        ui.collapsing(
                            format!("What the motor fit assumed ({})", fit.len()),
                            |ui| {
                                for line in &fit {
                                    ui.weak(format!("\u{2022} {line}"));
                                }
                            },
                        );
                    }
                });
                if !rows.is_empty() {
                    ui.separator();
                    egui::Grid::new("drive_run_rows")
                        .striped(true)
                        .show(ui, |ui| {
                            for (tag, text) in &rows {
                                ui.strong(tag);
                                ui.label(text);
                                ui.end_row();
                            }
                        });
                }
                ui.separator();
                ui.label(egui::RichText::new("Model, not measurement").strong());
                ui.weak(
                    "\u{2022} The drive is a generic V/f drive (ramps, current limit, trip \
                     levels); its parameters are the ones you enter, not a vendor's.",
                );
                ui.weak(
                    "\u{2022} The motor is fitted from the plate (equivalent circuit, constant \
                     rotor resistance): it can underestimate the standstill torque of large \
                     deep-bar motors.",
                );
                ui.weak(
                    "\u{2022} Rolling resistance is one constant; no stiction, no travel brake, \
                     no skew or sway yet.",
                );
                ui.weak(
                    "\u{2022} The drive replaces the contactors: its output terminals are not \
                     traced through the drawing, its inputs and mains are.",
                );
                ui.weak("\u{2022} Not a safety or compliance statement.");
            });
        if !open {
            self.wb.drive_run.open = false;
        }
        if live_toggle {
            self.toggle_live();
        }
        if stop {
            self.wb.drive_run.active = None;
        }
        if start {
            self.start_drive_run();
        }
    }

    /// Builds the loop from the travel axes of the project and checks their bindings.
    fn start_drive_run(&mut self) {
        let Some(doc) = self.backend.document() else {
            return;
        };
        let project = doc.project();
        let (modules, mut notes) = axes_of(project);
        if modules.is_empty() {
            notes.push((
                Level::Warning,
                "No axis can run: see the notes above.".into(),
            ));
            self.wb.drive_run.notice = notes;
            return;
        }
        let mut lp = match TravelLoop::new(StepConfig::default()) {
            Ok(lp) => lp,
            Err(why) => {
                self.wb.drive_run.notice = vec![(Level::Error, why.to_string())];
                return;
            }
        };
        lp.modules = modules;
        let findings = {
            let circuit = LiveCircuit::new(&mut self.wb.live, project);
            lp.check_bindings(&circuit)
        };
        notes.extend(
            findings
                .into_iter()
                .map(|f| (Level::Warning, format!("{}: {}", f.module, f.message))),
        );
        self.wb.drive_run.notice = notes;
        self.wb.drive_run.active = Some(Active { lp, acc: 0.0 });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_core::{
        DeviceTag, Document, Endpoint, Pin, Placement, Point, PotentialKind, SymbolId,
        SymbolInstance, SymbolRole,
    };

    fn put(
        tx: &mut novo_core::Tx<'_>,
        page: PageId,
        key: &str,
        y: f64,
        role: SymbolRole,
    ) -> SymbolId {
        let id = tx
            .place_symbol(
                page,
                key,
                Placement::at(Point::new(0.0, y)),
                vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 8.0)],
                role,
            )
            .unwrap();
        if key.contains("contact") {
            tx.update::<SymbolInstance>(id, |s| s.pass_through = vec![vec![0, 1]])
                .unwrap();
        }
        id
    }

    /// L1 -> button S1 -> limit S9 (NC) -> coil KF -> N; the drive's forward input is KF.
    fn axis(limit_m: Option<f64>) -> (Document, SymbolId) {
        let mut doc = Document::default();
        let (button, _) = doc
            .transact("Draw", |tx| {
                let page = tx.add_page("1")?;
                let dev = |d, main| SymbolRole::Device { device: d, main };
                let l1 = tx.add_potential("L1", PotentialKind::Line)?;
                let n = tx.add_potential("N", PotentialKind::Neutral)?;
                let kf = tx.add_device(DeviceTag::product("KF"), "")?;
                let s1 = tx.add_device(DeviceTag::product("S1"), "")?;
                let s9 = tx.add_device(DeviceTag::product("S9"), "")?;
                let m1 = tx.add_device(DeviceTag::product("M1"), "")?;
                let feed = put(
                    tx,
                    page,
                    "t/feed",
                    0.0,
                    SymbolRole::PotentialFeed { potential: l1 },
                );
                let feed_n = put(
                    tx,
                    page,
                    "t/feed",
                    30.0,
                    SymbolRole::PotentialFeed { potential: n },
                );
                let button = put(tx, page, "t/contact/no", 80.0, dev(s1, true));
                let limit = put(tx, page, "t/contact/nc", 90.0, dev(s9, true));
                let coil = put(tx, page, "t/coil", 100.0, dev(kf, true));
                tx.connect(Endpoint::pin(feed, 1), Endpoint::pin(button, 0))?;
                tx.connect(Endpoint::pin(button, 1), Endpoint::pin(limit, 0))?;
                tx.connect(Endpoint::pin(limit, 1), Endpoint::pin(coil, 0))?;
                tx.connect(Endpoint::pin(coil, 1), Endpoint::pin(feed_n, 1))?;
                let m = tx.project().device(m1).map(|d| d.id).unwrap();
                tx.set_property(m, "trav_mass_kg", Some(5000.0.into()))?;
                tx.set_property(
                    m,
                    "trav_in_fwd",
                    Some(DeviceTag::product("KF").to_string().into()),
                )?;
                if let Some(at) = limit_m {
                    tx.set_property(
                        m,
                        "trav_limit_fwd",
                        Some(DeviceTag::product("S9").to_string().into()),
                    )?;
                    tx.set_property(m, "trav_limit_fwd_m", Some(at.into()))?;
                }
                Ok(button)
            })
            .unwrap();
        (doc, button)
    }

    fn run_held(doc: &Document, button: SymbolId, seconds: f64) -> DriveRun {
        let project = doc.project();
        let (modules, notes) = axes_of(project);
        assert!(
            notes.iter().all(|(l, _)| matches!(l, Level::Info)),
            "{notes:?}"
        );
        let mut lp = TravelLoop::new(StepConfig::default()).unwrap();
        lp.modules = modules;
        let mut run = DriveRun {
            active: Some(Active { lp, acc: 0.0 }),
            ..Default::default()
        };
        let mut live = LiveView::default();
        live.on = true;
        live.reset_poles(project);
        live.poles.set_all(project, button, true);
        for _ in 0..(seconds / 0.016) as usize {
            advance(&mut run, &mut live, project, 0.016);
        }
        run
    }

    fn position(run: &DriveRun) -> f64 {
        run.active.as_ref().unwrap().lp.modules[0].position_m()
    }

    #[test]
    fn the_button_in_the_drawing_drives_the_axis() {
        let (doc, button) = axis(None);
        let run = run_held(&doc, button, 20.0);
        // Step 1 is 10 Hz: about 0.15 m/s after the 4 s ramp (5 s at 50 Hz scale).
        assert!(position(&run) > 1.5, "{}", position(&run));
        assert!(reading(&run.active.as_ref().unwrap().lp.modules[0]).contains("Hz"));
    }

    #[test]
    fn the_drawn_end_limit_stops_the_axis() {
        let (doc, button) = axis(Some(1.0));
        let run = run_held(&doc, button, 30.0);
        let p = position(&run);
        assert!((1.0..1.15).contains(&p), "{p}");
    }

    #[test]
    fn a_released_button_leaves_the_axis_at_rest() {
        let (doc, _) = axis(None);
        let (modules, _) = axes_of(doc.project());
        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].position_m(), 0.0);
    }
}
