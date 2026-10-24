//! "Cooling advice": tries where a pair of vents or fans goes and how big it
//! is, ranks the tries by the predicted device temperatures and shows the best
//! few with before and after numbers.
//!
//! Where the user finds it: Cabinet and 3D view, the Heat strip's "Cooling
//! advice" button, and the command palette ("Cooling advice and vent sweep").
//! The sweep runs on a worker thread in the 2D model
//! (`novo_thermal::sweep`); the window says that these are planning values,
//! not a validated result. "Add these openings" puts the pair in the Vents
//! and fans list of the 3D Heat panel, one undo step there, where a 3D run
//! can check it.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Instant;

use eframe::egui::{self, Pos2};
use novo_core::{Project, StructureId};
use novo_thermal::bridge::{BridgeOptions, scenario_from_project};
use novo_thermal::cooling::{Kind as FanKind, Product};
use novo_thermal::model::{Enclosure, Placement};
use novo_thermal::sweep::{
    Candidate, Control, Delivered, Outcome, Pairing, Piece, SIGNIFICANT_K, Settings, Sweep,
    sweep_with,
};

use super::heat_settings::{HeatSettings, Kind, VentSpec, Wall};

/// The window's one statement of what the numbers are. The sweep's own first
/// note ("Model estimate: ...") is hidden as a duplicate, so every caveat it
/// carries must be said here.
pub(super) const FOOTER: &str = "Model estimate: the 2D heat model with planning values, the losses you entered and the wire heat in the ducts. Not validated against measurements, and the 2D slice does not see the depth of the cabinet. Check the winner with Simulate in 3D after adding it.";

/// Width of the window, points.
pub(super) const WINDOW_WIDTH: f32 = 400.0;
const WIDTH: f32 = WINDOW_WIDTH;
/// Tallest the list of options gets before it scrolls, points: the window
/// stays small enough to leave the drawing visible.
const MAX_BODY: f32 = 440.0;

/// What a finished sweep found and what it needs to be applied.
pub(super) struct Found {
    pub sweep: Sweep,
    tags: BTreeMap<u64, String>,
    enclosure: Enclosure,
    /// Openings the cabinet already had when the sweep started.
    existing: usize,
    key: u64,
    /// Fans the ladder was taken from (the parts and the person's list); empty
    /// when the ladder is the generic planning sizes.
    fans: Vec<Product>,
}

struct Running {
    rx: mpsc::Receiver<Result<Sweep, String>>,
    control: Arc<Control>,
    started: Instant,
    tags: BTreeMap<u64, String>,
    enclosure: Enclosure,
    existing: usize,
    key: u64,
    fans: Vec<Product>,
}

/// State of the sweep and its window.
#[derive(Default)]
pub(super) struct CoolingAdvice {
    requested: bool,
    run: Option<Running>,
    pub(super) found: Option<Found>,
    notice: Option<String>,
    open: bool,
    /// What the last "Add" put in the list, shown in the window.
    added: Option<String>,
    /// The view the window opened in (1 Cabinet, 2 3D): it closes when the
    /// person goes to the other one.
    view: Option<u8>,
    /// The option choice leaves devices out (set each frame by the view).
    pub(super) left_out: bool,
}

/// What the window asks the app to do.
pub(super) enum Ask {
    /// Sweep again for the cabinet as it is now.
    Again,
    /// Put this pair in the list of vents and fans.
    Add(Candidate),
}

/// The fans of the project's parts and of the person's list that the sweep can
/// size from: real articles only, never the generic planning sizes.
pub(super) fn real_fans(products: Vec<Product>) -> Vec<Product> {
    products
        .into_iter()
        .filter(|p| p.source.is_real() && matches!(p.kind, FanKind::Fan | FanKind::FilterFan))
        .filter(|p| p.airflow_m3h > 0.0)
        .collect()
}

/// The fans the sweep sizes from for this project: the parts, as the project
/// has them now, and the person's list.
pub(super) fn fans_of(
    cooling: &mut super::heat_cooling::CoolingSettings,
    project: &Project,
) -> Vec<Product> {
    cooling.parts = novo_thermal::cooling::products_from_project(project);
    real_fans(cooling.products())
}

/// Most fan sizes the ladder takes from real products, to bound the cost.
const MAX_REAL_SIZES: usize = 8;

/// The sweep's ladder: the free-air flows of the real fans when there are any
/// (what the person can fit), else the generic sizes.
pub(super) fn settings_for(fans: &[Product]) -> Settings {
    let mut flows: Vec<f64> = fans.iter().map(|p| p.airflow_m3h).collect();
    flows.sort_by(|a, b| a.total_cmp(b));
    flows.dedup_by(|a, b| (*a - *b).abs() < 0.5);
    let mut settings = Settings::default();
    if !flows.is_empty() {
        if flows.len() > MAX_REAL_SIZES {
            let n = flows.len();
            flows = (0..MAX_REAL_SIZES)
                .map(|i| flows[i * (n - 1) / (MAX_REAL_SIZES - 1)])
                .collect();
            flows.dedup();
        }
        settings.fan_flows_m3h = flows;
    }
    settings
}

/// The article behind a fan of the sweep, when the ladder came from real ones.
fn products_for<'a>(fans: &'a [Product], piece: &Piece) -> Vec<&'a Product> {
    let Some(flow) = piece.fan_flow_m3h else {
        return Vec::new();
    };
    fans.iter()
        .filter(|p| (p.airflow_m3h - flow).abs() < 0.5)
        .collect()
}

/// Identity of the cabinet the sweep is for: changes with the project and
/// with the openings already set and with the fans the sizes come from.
pub(super) fn key_of(
    project: &Project,
    structure: StructureId,
    ambient: f64,
    settings: &HeatSettings,
    fans: &[Product],
) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (structure, project.revision(), ambient.to_bits()).hash(&mut h);
    format!("{:?}", settings.vents).hash(&mut h);
    for f in fans {
        (&f.name, f.airflow_m3h.to_bits()).hash(&mut h);
    }
    h.finish()
}

#[cfg(test)]
impl Found {
    pub(super) fn for_test(sweep: Sweep, key: u64) -> Found {
        Found {
            sweep,
            tags: BTreeMap::from([(1, "-T1".to_owned())]),
            enclosure: novo_thermal::model::Enclosure {
                width_mm: 800.0,
                height_mm: 1200.0,
                depth_mm: 300.0,
                wall_mm: 2.0,
                material: novo_layout::sizing::Material::Steel,
                installation: novo_layout::sizing::Installation::Wall,
                plate: novo_layout::Rect {
                    x: 50.0,
                    y: 50.0,
                    w: 690.0,
                    h: 1090.0,
                },
            },
            existing: 0,
            key,
            fans: Vec::new(),
        }
    }

    #[cfg(test)]
    pub(super) fn with_fans_for_test(mut self, fans: Vec<Product>) -> Found {
        self.fans = fans;
        self
    }
}

impl CoolingAdvice {
    #[cfg(test)]
    pub(super) fn set_found_for_test(&mut self, f: Found) {
        self.found = Some(f);
    }

    /// `heat.cooling_advice`: sweep on the next frame.
    pub(super) fn request(&mut self) {
        self.requested = true;
        self.open = true;
    }

    pub(super) fn take_request(&mut self) -> bool {
        std::mem::take(&mut self.requested)
    }

    pub(super) fn is_open(&self) -> bool {
        self.open
    }

    pub(super) fn running(&self) -> bool {
        self.run.is_some()
    }

    pub(super) fn close(&mut self) {
        if let Some(r) = &self.run {
            r.control.cancel.store(true, Ordering::Relaxed);
        }
        self.run = None;
        self.open = false;
        self.found = None;
        self.notice = None;
        self.added = None;
        self.view = None;
    }

    /// Says in the window what was added.
    pub(super) fn set_added(&mut self, text: String) {
        self.added = Some(text);
    }

    /// Starts the sweep from the cabinet as the project has it now, with the
    /// openings already in the Vents and fans list. Says why when it cannot.
    pub(super) fn start(
        &mut self,
        project: &Project,
        structure: StructureId,
        ambient: f64,
        settings: &HeatSettings,
        fans: Vec<Product>,
        tag_of: &dyn Fn(u64) -> String,
    ) {
        if self.run.is_some() {
            return;
        }
        self.found = None;
        self.notice = None;
        self.added = None;
        let wires = super::heat_wires::wire_heat(project, structure);
        let options = BridgeOptions {
            ambient_c: ambient,
            duct_loss_w: wires.by_duct,
            ..Default::default()
        };
        let mut scenario = match scenario_from_project(project, structure, &options) {
            Ok(b) => b.scenario,
            Err(e) => {
                self.notice = Some(format!(
                    "Cannot search: {e}. Set it in the Cabinet: Size enclosure or Plate setup"
                ));
                return;
            }
        };
        settings.apply(&mut scenario);
        if scenario.devices.is_empty() {
            self.notice =
                Some("No device stands on the plate, so there is no heat to cool.".into());
            return;
        }
        let key = key_of(project, structure, ambient, settings, &fans);
        let tags = scenario
            .devices
            .iter()
            .map(|d| (d.id, tag_of(d.id)))
            .collect();
        let enclosure = scenario.enclosure.clone();
        let existing = scenario.vents.len() + scenario.fans.len();
        let control = Arc::new(Control::default());
        let (tx, rx) = mpsc::channel();
        let c = control.clone();
        let sweep_settings = settings_for(&fans);
        std::thread::spawn(move || {
            let result = sweep_with(&scenario, &sweep_settings, &c).map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
        self.run = Some(Running {
            rx,
            control,
            started: Instant::now(),
            tags,
            enclosure,
            existing,
            key,
            fans,
        });
    }

    /// Takes in a finished sweep. Called every frame.
    pub(super) fn poll(&mut self, ctx: &egui::Context) {
        let Some(run) = &self.run else {
            return;
        };
        match run.rx.try_recv() {
            Ok(result) => {
                let run = self.run.take().expect("checked above");
                match result {
                    Ok(sweep) => {
                        self.found = Some(Found {
                            sweep,
                            tags: run.tags,
                            enclosure: run.enclosure,
                            existing: run.existing,
                            key: run.key,
                            fans: run.fans,
                        });
                    }
                    Err(e) => self.notice = Some(format!("The sweep failed: {e}")),
                }
            }
            Err(mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(200));
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.run = None;
                self.notice = Some("The sweep stopped unexpectedly".into());
            }
        }
    }

    /// The window. `now_key` is [`key_of`] for the cabinet as it is now.
    pub(super) fn show(
        &mut self,
        ctx: &egui::Context,
        view: u8,
        now_key: u64,
        at: Pos2,
    ) -> Option<Ask> {
        if !self.open {
            return None;
        }
        match self.view {
            None => self.view = Some(view),
            Some(v) if v != view => {
                self.close();
                return None;
            }
            Some(_) => {}
        }
        let mut ask = None;
        let mut close = false;
        let mut open = true;
        let pal = crate::theme::ui_palette();
        // Start high enough that the list has its full height: in a short
        // window the list used to get a sliver and cut a sentence off.
        let at = Pos2::new(
            at.x,
            at.y.min(ctx.content_rect().bottom() - MAX_BODY - 140.0)
                .max(60.0),
        );
        egui::Window::new("Cooling advice")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_pos(at)
            .default_width(WIDTH)
            .show(ctx, |ui| {
                ui.set_max_width(WIDTH);
                ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
                super::heat_cooling::left_out_line(ui, self.left_out);
                if let Some(run) = &self.run {
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new());
                        ui.label(format!(
                            "Trying vent and fan places and sizes in the model… {} runs so far, {:.0} s",
                            run.control.evaluated.load(Ordering::Relaxed),
                            run.started.elapsed().as_secs_f64()
                        ));
                    });
                    if ui.button("Stop and keep the best so far").clicked() {
                        run.control.cancel.store(true, Ordering::Relaxed);
                    }
                    return;
                }
                if let Some(n) = &self.notice {
                    ui.colored_label(pal.warning, n);
                }
                if let Some(a) = &self.added {
                    ui.colored_label(pal.success, a);
                }
                let Some(found) = &self.found else {
                    return;
                };
                let stale = found.key != now_key;
                if stale {
                    let text = if self.added.is_some() {
                        "Adding changed the cabinet, so this list is out of date: sweep again for more."
                    } else {
                        "The cabinet changed after this sweep, so this list is out of date. Sweep again before adding."
                    };
                    ui.label(egui::RichText::new(text).small().color(pal.text_secondary));
                    if ui.button("Sweep again").clicked() {
                        ask = Some(Ask::Again);
                    }
                }
                let room = (ctx.content_rect().bottom() - at.y - 130.0).clamp(160.0, MAX_BODY);
                egui::ScrollArea::vertical()
                    .max_height(room)
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                    .auto_shrink([false, true])
                    .show(ui, |ui| body(ui, found, stale, &mut ask));
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(FOOTER)
                    .small()
                    .color(pal.text_secondary),
                );
                ui.add_space(4.0);
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        if !open || close {
            self.close();
        }
        ask
    }
}

fn body(ui: &mut egui::Ui, found: &Found, stale: bool, ask: &mut Option<Ask>) {
    let pal = crate::theme::ui_palette();
    let s = &found.sweep;
    ui.label(egui::RichText::new(headline(s)).strong());
    ui.label(
        egui::RichText::new(
            "A try is one run of the 2D model with a pair of openings added to the cabinet.",
        )
        .small()
        .color(pal.text_secondary),
    );
    ui.add_space(2.0);
    ui.label(format!(
        "Now: {} · {} · hottest air {:.0} °C",
        margin_text(s.before.worst_margin_k),
        over_text(s.before.over),
        s.before.max_air_c
    ));
    if found.existing > 0 {
        ui.label(
            egui::RichText::new(format!(
                "On top of the {} opening(s) already in the Vents and fans list.",
                found.existing
            ))
            .small()
            .color(pal.text_secondary),
        );
    }
    ui.label(
        egui::RichText::new(format!(
            "The sizing rule asks for about {:.0} m³/h of fan flow to carry all the loss.",
            s.sizing_flow_m3h
        ))
        .small()
        .color(pal.text_secondary),
    );
    for o in &s.best {
        ui.add_space(8.0);
        ui.separator();
        option(ui, found, o, stale, ask);
    }
    // The window's footer already says the numbers are a model estimate.
    if !found.fans.is_empty() {
        ui.label(
            egui::RichText::new(format!(
                "Fan sizes come from your parts and cooling list ({} sizes), not the generic ladder.",
                settings_for(&found.fans).fan_flows_m3h.len()
            ))
            .small()
            .color(pal.text_secondary),
        );
    }
    for n in s.notes.iter().filter(|n| !n.starts_with("Model estimate")) {
        ui.add_space(4.0);
        ui.label(egui::RichText::new(n).small().color(pal.text_secondary));
    }
    if s.skipped > 0 {
        ui.label(
            egui::RichText::new(format!(
                "{} try(s) did not fit the cabinet and were left out.",
                s.skipped
            ))
            .small()
            .color(pal.text_secondary),
        );
    }
}

fn option(ui: &mut egui::Ui, found: &Found, o: &Outcome, stale: bool, ask: &mut Option<Ask>) {
    let pal = crate::theme::ui_palette();
    let before = &found.sweep.before;
    // An option with a fan gets the small slow fan, as in the Cooling selection.
    let fan = [&o.candidate.inlet, &o.candidate.outlet]
        .iter()
        .any(|p| p.fan_flow_m3h.is_some());
    ui.horizontal_wrapped(|ui| {
        if fan {
            super::motion::fan_icon(ui, 16.0, pal.info).on_hover_text(
                "This option has a fan. The turning is only a picture; its pace is not the flow.",
            );
        }
        ui.label(
            egui::RichText::new(format!("{}. {}", o.rank, o.candidate.pairing.label())).strong(),
        );
        if o.clear {
            ui.colored_label(
                pal.success,
                format!("gains {} K", gain_text(before, &o.reading, o.gain_k)),
            );
        } else {
            ui.colored_label(
                pal.warning,
                format!("{:+.1} K: within the model's noise", o.gain_k),
            );
        }
    });
    ui.label(o.candidate.describe(&found.enclosure));
    ui.label(format!(
        "Tightest margin {} → {}",
        margin_text(before.worst_margin_k),
        margin_text(o.reading.worst_margin_k)
    ));
    ui.label(format!(
        "{} → {} · hottest air {:.0} → {:.0} °C",
        over_text(before.over),
        over_text(o.reading.over),
        before.max_air_c,
        o.reading.max_air_c
    ));
    for piece in [&o.candidate.inlet, &o.candidate.outlet] {
        let products = products_for(&found.fans, piece);
        if let (false, Some(flow)) = (products.is_empty(), piece.fan_flow_m3h) {
            let names: Vec<&str> = products.iter().take(3).map(|p| p.name.as_str()).collect();
            let more = if products.len() > 3 { " or others" } else { "" };
            ui.label(
                egui::RichText::new(format!(
                    "Fan: {}{more} ({:.0} m³/h free air{})",
                    names.join(" or "),
                    flow,
                    if products.len() > 1 {
                        ", same flow"
                    } else {
                        ""
                    }
                ))
                .small()
                .color(pal.text_secondary),
            );
        }
    }
    if let (Some(i), Some(out)) = (&o.inlet, &o.outlet) {
        ui.label(
            egui::RichText::new(format!(
                "The model passes {} in and {} out.",
                flow_text(i),
                flow_text(out)
            ))
            .small()
            .color(pal.text_secondary),
        );
    }
    let changed: Vec<String> = o
        .devices
        .iter()
        .filter(|d| (d.surface_before_c - d.surface_after_c).abs() >= 0.5)
        .map(|d| {
            let tag = found
                .tags
                .get(&d.id)
                .filter(|t| !t.is_empty())
                .cloned()
                .unwrap_or_else(|| format!("device {}", d.id));
            format!(
                "{tag} {:.0} → {:.0} °C",
                d.surface_before_c, d.surface_after_c
            )
        })
        .collect();
    if !changed.is_empty() {
        ui.label(
            egui::RichText::new(format!("Housing: {}", changed.join(", ")))
                .small()
                .color(pal.text_secondary),
        );
    }
    if o.reading.unsettled {
        ui.label(
            egui::RichText::new(
                "This run did not settle fully; its numbers may move by about 1 K.",
            )
            .small()
            .color(pal.warning),
        );
    }
    if ui
        .add_enabled(!stale, egui::Button::new("Add these two to Vents and fans"))
        .on_hover_text(
            "Puts the pair in the list of the 3D Heat panel as one undo step there. Simulate in 3D then checks it",
        )
        .clicked()
    {
        *ask = Some(Ask::Add(o.candidate.clone()));
    }
}

fn headline(s: &Sweep) -> String {
    if s.best.is_empty() {
        return "Nothing to fix: every device has room to spare.".into();
    }
    match s.winner() {
        Some(w) => format!(
            "Best of {} tries: {}. It gains {} K of margin in the model.",
            s.tried,
            w.candidate.pairing.label().to_lowercase(),
            gain_text(&s.before, &w.reading, w.gain_k)
        ),
        None => format!(
            "No position or size wins {SIGNIFICANT_K:.0} K or more in the model ({} tries).",
            s.tried
        ),
    }
}

/// The gain as the difference of the margins shown, so the three numbers on
/// the card agree to the last digit.
fn gain_text(
    before: &novo_thermal::sweep::Reading,
    after: &novo_thermal::sweep::Reading,
    gain: f64,
) -> String {
    match (before.worst_margin_k, after.worst_margin_k) {
        (Some(b), Some(a)) => {
            let r = |v: f64| (v * 10.0).round() / 10.0;
            format!("{:.1}", r(a) - r(b))
        }
        _ => format!("{gain:.1}"),
    }
}

fn margin_text(m: Option<f64>) -> String {
    match m {
        Some(m) => format!("{m:+.1} K"),
        None => "no limits set".into(),
    }
}

fn over_text(n: usize) -> String {
    match n {
        0 => "no device over its limit".into(),
        1 => "1 device over".into(),
        n => format!("{n} devices over"),
    }
}

fn flow_text(d: &Delivered) -> String {
    format!("{:.0} m³/h", d.flow_m3h.abs())
}

/// The Vents and fans entry for one piece of a candidate.
pub(super) fn spec_of(piece: &Piece, inlet: bool) -> VentSpec {
    let o = &piece.opening;
    let (wall, u, v) = match o.placement {
        Placement::Top { x_mm } => (Wall::Top, x_mm, 0.0),
        Placement::Bottom { x_mm } => (Wall::Bottom, x_mm, 0.0),
        Placement::Left { y_mm } => (Wall::Left, y_mm, 0.0),
        Placement::Right { y_mm } => (Wall::Right, y_mm, 0.0),
        Placement::Front { x_mm, y_mm } => (Wall::Door, x_mm, y_mm),
    };
    let mut spec = VentSpec::new(wall);
    spec.u_mm = u;
    spec.v_mm = v;
    spec.width_mm = o.width_mm;
    spec.height_mm = o.height_mm;
    match piece.fan_flow_m3h {
        Some(flow) => {
            spec.kind = if inlet { Kind::FilterFan } else { Kind::Fan };
            spec.exhaust = !inlet;
            spec.flow_m3h = flow;
        }
        None => spec.kind = Kind::Grille,
    }
    spec
}

/// The two entries of a candidate: inlet first.
pub(super) fn specs_of(c: &Candidate) -> [VentSpec; 2] {
    [spec_of(&c.inlet, true), spec_of(&c.outlet, false)]
}

/// Adds the pair to the settings as one undo step.
pub(super) fn add_pair(settings: &mut HeatSettings, c: &Candidate) {
    settings.checkpoint();
    settings.vents.extend(specs_of(c));
    settings.selected = Some(settings.vents.len() - 1);
}

/// The sentence for the status line after adding.
pub(super) fn added_text(c: &Candidate) -> String {
    let kind = match c.pairing {
        Pairing::VentPair => "two vents",
        Pairing::FanPair => "a filter fan and an exhaust fan",
        Pairing::VentInFanOut => "a vent and an exhaust fan",
        Pairing::FanInVentOut => "a filter fan and a vent",
    };
    format!(
        "Added {kind} to Vents and fans in the 3D Heat panel. Simulate in 3D to check them; undo there takes them out."
    )
}
