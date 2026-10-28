//! Hoist run (`sim.hoist`, Simulate > Hoist run): the live circuit drives the two-speed hoist
//! engine of `novo-drive` through the fixed-step loop of `novo-machine` (lane L5a, design
//! `docs/design/electromechanical-loop.md`).
//!
//! A motor whose property sheet has a hook load ("Mechanical load", `mech_*` properties) is a
//! hoist. With Live switched on, press the buttons of the drawing: the contactors settle, the
//! motor is fed on its fine or coarse winding, the brake coil releases, the hook moves, and a
//! limit switch bound in the property sheet opens its drawn contact at its height. Badges on the
//! motor show rpm, winding, brake and height.

use std::collections::{BTreeMap, BTreeSet};

use eframe::egui;
use novo_core::{PageId, Project, SymbolId, Value};
use novo_drive::brake::BrakeState;
use novo_drive::motor::{Direction, WindingSel};
use novo_machine::cosim::StepConfig;
use novo_machine::hoist_loop::{HoistLoop, HoistModule};
use novo_machine::port::{CircuitPort, MotorFeed};
use novo_machine::spec;
use novo_sim::{DeviceStates, MotorRun, is_motor};

use super::live::LiveView;
use super::{Level, NovoApp};

/// Most ticks run in one frame, so a slow frame cannot snowball.
const MAX_TICKS: usize = 40;

/// What the Simulate > Hoist run window remembers.
#[derive(Default)]
pub(crate) struct HoistRun {
    pub(super) open: bool,
    active: Option<Active>,
    /// Why the last start did not run, or what it found.
    notice: Vec<(Level, String)>,
}

struct Active {
    lp: HoistLoop,
    acc: f64,
}

/// The live view as the loop's circuit.
struct LiveCircuit<'a> {
    live: &'a mut LiveView,
    project: &'a Project,
    /// Symbols by device tag.
    by_tag: BTreeMap<String, Vec<SymbolId>>,
    states: DeviceStates,
    coils: BTreeSet<SymbolId>,
}

impl<'a> LiveCircuit<'a> {
    fn new(live: &'a mut LiveView, project: &'a Project) -> Self {
        let mut by_tag: BTreeMap<String, Vec<SymbolId>> = BTreeMap::new();
        for symbol in project.symbols() {
            if let Some(device) = symbol.device().and_then(|d| project.device(d)) {
                by_tag
                    .entry(device.tag.to_string())
                    .or_default()
                    .push(symbol.id);
            }
        }
        Self {
            live,
            project,
            by_tag,
            states: DeviceStates::default(),
            coils: BTreeSet::new(),
        }
    }
}

impl CircuitPort for LiveCircuit<'_> {
    fn settle(&mut self, dt_s: f64) {
        let (states, coils) = self.live.settle_for_run(self.project, dt_s);
        self.states = states;
        self.coils = coils;
    }

    fn motor_feed(&self, motor: &str) -> MotorFeed {
        let run = self
            .by_tag
            .get(motor)
            .into_iter()
            .flatten()
            .find_map(|id| self.states.motors.get(id));
        match run {
            // The first winding set of a two-speed motor symbol is the slow (fine) one.
            Some(MotorRun::Running { direction, speed }) => MotorFeed::Fed {
                winding: if *speed == 0 {
                    WindingSel::Fine
                } else {
                    WindingSel::Coarse
                },
                direction: match direction {
                    novo_sim::Direction::Forward => Direction::Positive,
                    novo_sim::Direction::Reverse => Direction::Negative,
                },
            },
            _ => MotorFeed::Off,
        }
    }

    fn is_energised(&self, device: &str) -> bool {
        self.by_tag.get(device).into_iter().flatten().any(|id| {
            self.coils.contains(id) || self.states.lamps.get(id).copied().unwrap_or(false)
        })
    }

    fn set_contact(&mut self, device: &str, closed: bool) {
        for id in self.by_tag.get(device).into_iter().flatten() {
            self.live.poles.set_all(self.project, *id, closed);
        }
    }

    fn knows_device(&self, device: &str) -> bool {
        self.by_tag.contains_key(device)
    }
}

fn value_text(value: &Value) -> String {
    value.to_string()
}

/// The hoists of a project: every device with a hook load set. A hoist whose numbers the engine
/// refuses is reported and left out.
pub(super) fn hoists_of(project: &Project) -> (Vec<HoistModule>, Vec<(Level, String)>) {
    let mut modules = Vec::new();
    let mut notes = Vec::new();
    for device in project.devices() {
        if !device.properties.contains_key(spec::HOIST_KEY) {
            continue;
        }
        let tag = device.tag.to_string();
        let get = |key: &str| device.properties.get(key).map(value_text);
        match spec::hoist_from_properties(&tag, &tag, &get) {
            Ok(Some(module)) => modules.push(module),
            Ok(None) => {}
            Err(why) => notes.push((Level::Warning, format!("{tag} is left out: {why}"))),
        }
    }
    (modules, notes)
}

/// Steps the loop for one frame. Returns whether a run is active (the frame then reads the
/// circuit without advancing its own clock again).
pub(super) fn advance(
    run: &mut HoistRun,
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

/// A one-line reading of a hoist, for the badge and the window.
pub(super) fn reading(module: &HoistModule) -> String {
    let Some(s) = module.snapshot() else {
        return "waiting".to_owned();
    };
    let winding = match s.energised.map(|e| e.winding) {
        Some(WindingSel::Fine) => "fine",
        Some(WindingSel::Coarse) => "coarse",
        None => "off",
    };
    let brake = match s.brake_state {
        BrakeState::Applied => "brake closed",
        BrakeState::Released => "brake open",
    };
    format!(
        "{:.0} rpm \u{b7} {winding} \u{b7} {brake} \u{b7} {:.2} m",
        s.shaft_rpm.abs(),
        module.height_m()
    )
}

/// Badges on the motors of the hoists, on the page shown.
#[allow(clippy::too_many_arguments)]
pub(super) fn paint_badges(
    ctx: &egui::Context,
    canvas: egui::Rect,
    to_screen: &dyn Fn(egui::Vec2) -> egui::Pos2,
    project: &Project,
    library: Option<&novo_symbols::Library>,
    page: PageId,
    run: &HoistRun,
) {
    let Some(active) = run.active.as_ref() else {
        return;
    };
    let painter = ctx
        .layer_painter(egui::LayerId::new(
            egui::Order::Middle,
            egui::Id::new("hoist_badges"),
        ))
        .with_clip_rect(canvas);
    let palette = crate::theme::ui_palette();
    for module in &active.lp.hoists {
        for symbol in project.symbols().filter(|s| s.page == page && is_motor(s)) {
            let tag = symbol
                .device()
                .and_then(|d| project.device(d))
                .map(|d| d.tag.to_string());
            if tag.as_deref() != Some(module.binding.motor.as_str()) {
                continue;
            }
            let Some((at, r)) = super::live_anim::body(library, symbol) else {
                continue;
            };
            let centre = to_screen(at);
            let text = reading(module);
            let galley =
                painter.layout_no_wrap(text, egui::FontId::proportional(12.0), palette.text_strong);
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
    if active.lp.hoists.iter().any(|m| m.snapshot().is_some()) {
        ctx.request_repaint();
    }
}

impl NovoApp {
    /// `sim.hoist`: opens or closes the window.
    pub(super) fn toggle_hoist_run(&mut self) {
        self.wb.hoist.open ^= true;
    }

    pub(super) fn hoist_window(&mut self, ctx: &egui::Context) {
        if !self.wb.hoist.open {
            return;
        }
        let Some(doc) = self.backend.document() else {
            return;
        };
        let project = doc.project();
        let (hoists, _) = hoists_of(project);
        let live_on = self.wb.live.on;
        let mut open = true;
        let mut start = false;
        let mut stop = false;
        let mut live_toggle = false;
        let hoist = &self.wb.hoist;
        let rows: Vec<(String, String)> = hoist.active.as_ref().map_or_else(Vec::new, |a| {
            a.lp.hoists
                .iter()
                .map(|m| (m.id.clone(), reading(m)))
                .collect()
        });
        let running = hoist.active.is_some();
        let notice = hoist.notice.clone();
        let names: Vec<String> = hoists.iter().map(|h| h.id.clone()).collect();
        // The Live card sits at the left edge and takes the buttons of the drawing: the window
        // opens at the right edge (the constraint pulls it inside) so neither covers the other.
        super::panel::dialog(ctx, "Hoist run", [560.0, 440.0])
            .default_pos([ctx.content_rect().max.x, 60.0])
            .open(super::panel::esc(ctx, &mut open))
            .show(ctx, |ui| {
                ui.label(
                    "Press the buttons of the drawing in the live view: the contactors settle, \
                     the two-speed motor and its brake move the hook, and the limit switches you \
                     bound open their contacts at their height.",
                );
                ui.add_space(4.0);
                if names.is_empty() {
                    ui.weak(
                        "No hoist yet. Select a motor and enter its hook load under \
                         Mechanical load in the property sheet.",
                    );
                } else {
                    ui.label(format!("Hoists in this project: {}", names.join(", ")));
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
                for (level, text) in &notice {
                    let w = match level {
                        Level::Error => novo_theme::widgets::Level::Error,
                        _ => novo_theme::widgets::Level::Warning,
                    };
                    novo_theme::widgets::banner(ui, w, text, |_| {});
                }
                if !rows.is_empty() {
                    ui.separator();
                    egui::Grid::new("hoist_rows").striped(true).show(ui, |ui| {
                        for (tag, text) in &rows {
                            ui.strong(tag);
                            ui.label(text);
                            ui.end_row();
                        }
                    });
                }
                ui.separator();
                ui.label(egui::RichText::new("Model, not measurement").strong());
                for line in novo_drive::motor::MOTOR_MODEL_LEAVES_OUT {
                    ui.weak(format!("\u{2022} {line}"));
                }
                ui.weak(
                    "\u{2022} Brake delays and torque are the numbers you enter; \
                     speeds follow the nameplate fit, not a measured curve.",
                );
                ui.weak(
                    "\u{2022} Assumed from the drawing: motor speed 0 is the fine winding, any \
                     other is coarse, and forward is up.",
                );
                ui.weak("\u{2022} Limit heights are measured from where the hook starts.");
                ui.weak("\u{2022} Not a safety or compliance statement.");
            });
        if !open {
            self.wb.hoist.open = false;
        }
        if live_toggle {
            self.toggle_live();
        }
        if stop {
            self.wb.hoist.active = None;
        }
        if start {
            self.start_hoist_run();
        }
    }

    /// Builds the loop from the hoists of the project and checks their bindings.
    fn start_hoist_run(&mut self) {
        let Some(doc) = self.backend.document() else {
            return;
        };
        let project = doc.project();
        let (modules, mut notes) = hoists_of(project);
        if modules.is_empty() {
            notes.push((
                Level::Warning,
                "No hoist can run: see the notes above.".into(),
            ));
            self.wb.hoist.notice = notes;
            return;
        }
        let mut lp = match HoistLoop::new(StepConfig::default()) {
            Ok(lp) => lp,
            Err(why) => {
                self.wb.hoist.notice = vec![(Level::Error, why.to_string())];
                return;
            }
        };
        lp.hoists = modules;
        let findings = {
            let circuit = LiveCircuit::new(&mut self.wb.live, project);
            lp.check_bindings(&circuit)
        };
        notes.extend(
            findings
                .into_iter()
                .map(|f| (Level::Warning, format!("{}: {}", f.module, f.message))),
        );
        self.wb.hoist.notice = notes;
        self.wb.hoist.active = Some(Active { lp, acc: 0.0 });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_core::{
        DeviceTag, Document, Endpoint, Pin, Placement, Point, PotentialKind, SymbolInstance,
        SymbolRole, Tx,
    };

    fn put(tx: &mut Tx<'_>, page: PageId, key: &str, y: f64, role: SymbolRole) -> SymbolId {
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

    /// L1 L2 L3 -> KF main contacts -> two-speed motor M1 (fine winding); the coil of KF is fed
    /// from L1 through the up button S1 and the upper limit contact S9 (NC) to N.
    struct Hoist {
        doc: Document,
        button: SymbolId,
    }

    fn hoist(limit_wired: bool, limit_m: f64) -> Hoist {
        let mut doc = Document::default();
        let (button, _) = doc
            .transact("Draw", |tx| {
                let page = tx.add_page("1")?;
                let dev = |d, main| SymbolRole::Device { device: d, main };
                let lines: Vec<_> = ["L1", "L2", "L3"]
                    .iter()
                    .map(|n| tx.add_potential(*n, PotentialKind::Line))
                    .collect::<Result<_, _>>()?;
                let n = tx.add_potential("N", PotentialKind::Neutral)?;
                let kf = tx.add_device(DeviceTag::product("KF"), "")?;
                let s1 = tx.add_device(DeviceTag::product("S1"), "")?;
                let s9 = tx.add_device(DeviceTag::product("S9"), "")?;
                let m1 = tx.add_device(DeviceTag::product("M1"), "")?;
                let feeds: Vec<_> = lines
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        put(
                            tx,
                            page,
                            "t/feed",
                            i as f64 * 10.0,
                            SymbolRole::PotentialFeed { potential: *p },
                        )
                    })
                    .collect();
                let feed_n = put(
                    tx,
                    page,
                    "t/feed",
                    30.0,
                    SymbolRole::PotentialFeed { potential: n },
                );
                let motor = tx.place_symbol(
                    page,
                    "iec/motors/motor_3ph_dahlander",
                    Placement::at(Point::new(100.0, 0.0)),
                    ["1U", "1V", "1W", "2U", "2V", "2W", "PE"]
                        .iter()
                        .enumerate()
                        .map(|(i, p)| Pin::new(*p, i as f64 * 8.0, 0.0))
                        .collect(),
                    dev(m1, true),
                )?;
                for (i, feed) in feeds.iter().enumerate() {
                    let c = put(
                        tx,
                        page,
                        "t/contact/no",
                        40.0 + i as f64 * 10.0,
                        dev(kf, false),
                    );
                    tx.connect(Endpoint::pin(*feed, 1), Endpoint::pin(c, 0))?;
                    tx.connect(Endpoint::pin(c, 1), Endpoint::pin(motor, i as u16))?;
                }
                let button = put(tx, page, "t/contact/no", 80.0, dev(s1, true));
                let limit = put(tx, page, "t/contact/nc", 90.0, dev(s9, true));
                let coil = put(tx, page, "t/coil", 100.0, dev(kf, true));
                tx.connect(Endpoint::pin(feeds[0], 1), Endpoint::pin(button, 0))?;
                if limit_wired {
                    tx.connect(Endpoint::pin(button, 1), Endpoint::pin(limit, 0))?;
                    tx.connect(Endpoint::pin(limit, 1), Endpoint::pin(coil, 0))?;
                } else {
                    tx.connect(Endpoint::pin(button, 1), Endpoint::pin(coil, 0))?;
                }
                tx.connect(Endpoint::pin(coil, 1), Endpoint::pin(feed_n, 1))?;
                let m = tx.project().device(m1).map(|d| d.id).unwrap();
                tx.set_property(m, "mech_mass_kg", Some(100.0.into()))?;
                tx.set_property(
                    m,
                    "mech_upper_limit",
                    Some(DeviceTag::product("S9").to_string().into()),
                )?;
                tx.set_property(m, "mech_upper_limit_m", Some(limit_m.into()))?;
                Ok(button)
            })
            .unwrap();
        Hoist { doc, button }
    }

    /// Runs `seconds` of the live view with the button held; returns the loop.
    fn run_held(h: &Hoist, seconds: f64) -> (HoistRun, LiveView) {
        let project = h.doc.project();
        let (modules, notes) = hoists_of(project);
        assert!(notes.is_empty(), "{notes:?}");
        let mut lp = HoistLoop::new(StepConfig::default()).unwrap();
        lp.hoists = modules;
        let mut run = HoistRun {
            active: Some(Active { lp, acc: 0.0 }),
            ..Default::default()
        };
        let mut live = LiveView::default();
        live.on = true;
        live.reset_poles(project);
        live.poles.set_all(project, h.button, true);
        for _ in 0..(seconds / 0.016) as usize {
            advance(&mut run, &mut live, project, 0.016);
        }
        (run, live)
    }

    fn height(run: &HoistRun) -> f64 {
        run.active.as_ref().unwrap().lp.hoists[0].height_m()
    }

    #[test]
    fn the_button_in_the_drawing_lifts_the_hook_and_the_drawn_limit_stops_it() {
        let h = hoist(true, 0.5);
        let (run, live) = run_held(&h, 8.0);
        let reached = height(&run);
        // The drawn NC limit contact opened at 0.5 m; coast to a stop before the brake bites
        // is a few centimetres at fine speed (about 0.2 m/s, 0.2 s apply delay).
        assert!((0.5..0.62).contains(&reached), "{reached}");
        let s9 = h
            .doc
            .project()
            .symbols()
            .find(|s| s.symbol.0.ends_with("contact/nc") && !live.poles.is_closed(s.id, 0))
            .is_some();
        assert!(s9, "the limit contact is open in the live state");
        // And it stays there: the brake holds the load.
        let (run2, _) = run_held(&h, 12.0);
        assert!((height(&run2) - reached).abs() < 0.02);
    }

    #[test]
    fn a_limit_that_is_not_in_the_drawing_does_not_stop_the_hook() {
        let h = hoist(false, 0.5);
        let (run, _) = run_held(&h, 8.0);
        assert!(height(&run) > 1.0, "{}", height(&run));
    }

    #[test]
    fn unknown_bindings_are_reported_before_the_run() {
        let h = hoist(true, 0.5);
        let project = h.doc.project();
        let (mut modules, _) = hoists_of(project);
        modules[0].binding.limits[0].binding.device_tag = "S99".into();
        let mut live = LiveView::default();
        let circuit = LiveCircuit::new(&mut live, project);
        let mut lp = HoistLoop::new(StepConfig::default()).unwrap();
        lp.hoists = modules;
        let found = lp.check_bindings(&circuit);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].message.contains("limit switch S99"));
    }
}
