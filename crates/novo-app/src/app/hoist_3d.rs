//! Hoist in 3D (`sim.hoist3d`, Simulate > Hoist in 3D, or the button in Hoist run): the hook of
//! a simulated hoist drawn on a generic overhead crane. The numbers come from the hoist loop of
//! the Hoist run window (hook height, the conditions it reports, the heights of the bound limit
//! switches); the crane around them is a generic picture, see `CRANE_PICTURE_LEAVES_OUT`.

use super::crane_setup::CraneSetup;
use super::hoist_run::conditions;
use super::{Level, NovoApp};
use eframe::egui;
use novo_machine::cl_winch_loop::ClWinchModule;
use novo_machine::hoist_loop::{HoistModule, LimitKind};
use novo_machine::swing::{DEFAULT_DAMPING_RATIO, SWING_MODEL_LEAVES_OUT, Swing};
use novo_view3d::camera::ViewPreset;
use novo_view3d::crane::{
    CRANE_PICTURE_LEAVES_OUT, CraneLayout, CraneState, LimitFlag, LimitMark, TravelMark, build,
    hook_bounds, rope_length_mm,
};
use novo_view3d::palette::View3dTheme;
use novo_view3d::widget::{View3d, ViewOptions};
use novo_view3d::{Look, MeshLibrary, Scene};
use std::sync::Arc;

const VIEW_ID: u64 = 0x4801_5700;

/// What the Hoist in 3D window remembers.
pub(crate) struct Hoist3d {
    pub(super) open: bool,
    view: View3d,
    library: Arc<MeshLibrary>,
    layout: CraneLayout,
    bridge_m: f32,
    trolley_m: f32,
    /// Which hoist of the run the picture follows, by index.
    follow: usize,
    /// The travel axes (by device tag) of a Drive run that move the bridge and the trolley.
    bridge_axis: Option<String>,
    trolley_axis: Option<String>,
    /// The swing of the hook along the runway (x) and across the span (z), and the axis speeds of
    /// the last frame, from which the acceleration is taken.
    swing: [Swing; 2],
    last_speed: [f64; 2],
    /// The setup as the project last had it, read from or written to the project info.
    synced: CraneSetup,
    /// The scene of the last frame and what it was built from, so an unchanged picture is not
    /// uploaded again.
    built: Option<(CraneLayout, CraneState, Arc<Scene>)>,
    revision: u64,
    needs_fit: bool,
    pending_preset: Option<ViewPreset>,
    /// Frame the hook on the next frame.
    frame_hook: bool,
}

impl Default for Hoist3d {
    fn default() -> Self {
        let layout = CraneLayout::default();
        Self {
            open: false,
            view: View3d::new(VIEW_ID),
            library: Arc::new(MeshLibrary::new()),
            // Near one end, so a travel drive that runs forward has room to go.
            bridge_m: layout.runway_m * 0.2,
            trolley_m: layout.span_m * 0.5,
            layout,
            follow: 0,
            bridge_axis: None,
            trolley_axis: None,
            swing: [new_swing(), new_swing()],
            last_speed: [0.0; 2],
            synced: CraneSetup::default(),
            built: None,
            revision: 0,
            needs_fit: true,
            pending_preset: None,
            frame_hook: false,
        }
    }
}

fn new_swing() -> Swing {
    Swing::new(4.0, DEFAULT_DAMPING_RATIO).expect("the default swing numbers are valid")
}

/// What one frame of travel did to the picture: the positions of the bridge and the trolley and
/// the hook's lean, in the order (x, z).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Travel {
    pub bridge_m: f32,
    pub trolley_m: f32,
    pub swing_rad: [f32; 2],
    /// An axis ran past the end of the runway or the span and the picture holds at the end.
    pub bridge_at_end: bool,
    pub trolley_at_end: bool,
}

impl Hoist3d {
    /// What the project remembers of the picture, as the window has it now.
    fn setup(&self) -> CraneSetup {
        CraneSetup {
            layout: self.layout,
            bridge_m: self.bridge_m,
            trolley_m: self.trolley_m,
            bridge_axis: self.bridge_axis.clone(),
            trolley_axis: self.trolley_axis.clone(),
        }
    }

    /// Puts a setup read from the project into the window.
    fn apply(&mut self, s: &CraneSetup) {
        self.layout = s.layout;
        self.bridge_m = s.bridge_m;
        self.trolley_m = s.trolley_m;
        self.bridge_axis = s.bridge_axis.clone();
        self.trolley_axis = s.trolley_axis.clone();
        self.needs_fit = true;
    }

    /// One frame of travel: the axes that drive the bridge and the trolley give a position and a
    /// speed (`None` for an axis that is not driven or has no run), the swing steps with the
    /// acceleration of each axis over `dt_s`. `rope_m` is the length of the rope.
    pub(super) fn travel(
        &mut self,
        dt_s: f64,
        driven: [Option<(f64, f64)>; 2],
        rope_m: f64,
    ) -> Travel {
        let start = [f64::from(self.bridge_m), f64::from(self.trolley_m)];
        let end = [
            f64::from(self.layout.runway_m),
            f64::from(self.layout.span_m),
        ];
        let mut pos = [0.0; 2];
        let mut at_end = [false; 2];
        for i in 0..2 {
            let (p, v) = driven[i].unwrap_or((0.0, 0.0));
            let want = start[i] + p;
            pos[i] = want.clamp(0.0, end[i]);
            at_end[i] = (want - pos[i]).abs() > 1e-6;
            // An axis held at its end of the picture has no speed to swing the hook with.
            let v = if at_end[i] { 0.0 } else { v };
            if dt_s > 0.0 {
                // A speed that appears from nothing (a run starting) is not a jolt.
                let jump = driven[i].is_some() && self.last_speed[i] == 0.0 && v.abs() > 3.0;
                let accel = if driven[i].is_some() && !jump {
                    ((v - self.last_speed[i]) / dt_s).clamp(-10.0, 10.0)
                } else {
                    0.0
                };
                self.swing[i].set_length_m(rope_m);
                self.swing[i].step(dt_s, accel);
            }
            self.last_speed[i] = v;
        }
        Travel {
            bridge_m: pos[0] as f32,
            trolley_m: pos[1] as f32,
            swing_rad: [
                self.swing[0].angle_rad() as f32,
                self.swing[1].angle_rad() as f32,
            ],
            bridge_at_end: at_end[0],
            trolley_at_end: at_end[1],
        }
    }

    /// The typed start position of the bridge, m.
    #[cfg(test)]
    pub(super) fn bridge_start_m(&self) -> f32 {
        self.bridge_m
    }

    /// The hook is still swinging enough to be worth another frame.
    fn swinging(&self) -> bool {
        self.swing
            .iter()
            .any(|s| s.angle_rad().abs() > 2e-4 || s.rate_rad_s().abs() > 2e-4)
    }
}

/// The picture's numbers for one hoist: its hook, the limit flags and whether it reports a problem.
pub(super) fn state_of(module: Option<&HoistModule>, bridge_m: f32, trolley_m: f32) -> CraneState {
    let Some(m) = module else {
        return CraneState {
            bridge_m,
            trolley_m,
            ..Default::default()
        };
    };
    CraneState {
        bridge_m,
        trolley_m,
        hook_m: m.height_m() as f32,
        swing_rad: [0.0; 2],
        warning: !conditions(m).is_empty(),
        bridge_limits: Vec::new(),
        trolley_limits: Vec::new(),
        limits: m
            .binding
            .limits
            .iter()
            .map(|l| LimitMark {
                height_m: (l.binding.actuate_at_mm / 1000.0) as f32,
                kind: match l.kind {
                    LimitKind::Upper => LimitFlag::Upper,
                    LimitKind::Lower => LimitFlag::Lower,
                },
            })
            .collect(),
    }
}

/// What the picture leaves out when it follows a closed-loop winch of Drive run.
const WINCH_PICTURE_LEAVES_OUT: &[&str] = &[
    "A closed-loop winch is drawn as a hook on a vertical rope at its height; the ground it may rest on, slack rope, the drive state and the load cell reading are in the line above the picture, not drawn",
    "The swing is the same one-pendulum model as for a hoist, from the travel axes picked here; a rope that hangs at an angle from the start is not drawn",
];

/// The picture's numbers for one closed-loop winch of Drive run: its hook, the limit flags and
/// whether the drive tripped or the rope reports a problem.
pub(super) fn winch_state_of(m: &ClWinchModule, bridge_m: f32, trolley_m: f32) -> CraneState {
    CraneState {
        bridge_m,
        trolley_m,
        hook_m: m.height_m() as f32,
        swing_rad: [0.0; 2],
        warning: m
            .snapshot()
            .is_some_and(|s| s.drive.fault.is_some() || s.slack_m > 0.0 || m.lifting_inhibited()),
        bridge_limits: Vec::new(),
        trolley_limits: Vec::new(),
        limits: m
            .binding
            .limits
            .iter()
            .map(|l| LimitMark {
                height_m: (l.binding.actuate_at_mm / 1000.0) as f32,
                kind: match l.kind {
                    LimitKind::Upper => LimitFlag::Upper,
                    LimitKind::Lower => LimitFlag::Lower,
                },
            })
            .collect(),
    }
}

/// The line under the controls for a closed-loop winch: where the hook is and what it does.
pub(super) fn winch_line(m: &ClWinchModule) -> String {
    let s = m.snapshot();
    let v = s.map_or(0.0, |s| s.hook_speed_m_s);
    let way = if v.abs() < 0.005 {
        "standing"
    } else if v > 0.0 {
        "rising"
    } else {
        "lowering"
    };
    let mut line = format!(
        "{}: hook {:+.2} m from its start, {:.2} m/s, {way}",
        m.id,
        m.height_m(),
        v.abs()
    );
    if let Some(s) = s {
        if s.resting {
            line.push_str(" \u{b7} resting on the ground");
        }
        if s.slack_m > 0.005 {
            line.push_str(&format!(" \u{b7} slack rope {:.0} mm", s.slack_m * 1000.0));
        }
        if let Some(f) = s.drive.fault {
            line.push_str(&format!(" \u{b7} drive tripped ({})", f.code()));
        }
    }
    if let Some(pct) = m.load_pct() {
        line.push_str(&format!(" \u{b7} load {pct:.0} % of rated"));
    }
    line
}

/// The end limits of a travel axis as flag positions in the picture: the start of the axis plus the
/// distance set on its motor. Nothing without an axis or a project.
pub(super) fn travel_marks(
    project: Option<&novo_core::Project>,
    axis: Option<&str>,
    start_m: f32,
) -> Vec<TravelMark> {
    let (Some(project), Some(axis)) = (project, axis) else {
        return Vec::new();
    };
    super::drive_run::travel_limits_of(project, axis)
        .into_iter()
        .map(|(forward, m)| TravelMark {
            position_m: start_m + m as f32,
            kind: if forward {
                LimitFlag::Upper
            } else {
                LimitFlag::Lower
            },
        })
        .collect()
}

/// The line under the controls: where the hook is and what it does.
pub(super) fn hook_line(module: &HoistModule) -> String {
    let h = module.height_m();
    let v = module.snapshot().map_or(0.0, |s| s.hook_speed_m_s);
    let way = if v.abs() < 0.005 {
        "standing"
    } else if v > 0.0 {
        "rising"
    } else {
        "lowering"
    };
    format!(
        "{}: hook {:+.2} m from its start, {:.2} m/s, {way}",
        module.id,
        h,
        v.abs()
    )
}

impl NovoApp {
    /// Keeps the window and the project in step. A setup the project holds that the window does
    /// not (a project just opened, an undo) is taken into the window; a setup the user changed in
    /// the window is saved in the project, one undo step, once the mouse button is up so a drag
    /// is not a hundred steps.
    fn sync_crane_setup(&mut self, ctx: &egui::Context, axes: &[String]) {
        let Some(doc) = self.backend.document() else {
            return;
        };
        let stored = CraneSetup::from_properties(&doc.project().info().properties)
            .without_missing_axes(axes);
        let g = &mut self.wb.hoist.three_d;
        if stored != g.synced {
            g.apply(&stored);
            g.synced = stored;
            return;
        }
        let now = g.setup();
        if now == g.synced || ctx.input(|i| i.pointer.any_down()) {
            return;
        }
        g.synced = now.clone();
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        match doc.transact("Crane picture", |tx| {
            tx.set_info(|i| now.write_properties(&mut i.properties))
        }) {
            Ok(((), changes)) => self.changed_outside_canvas(&changes),
            Err(e) => self.message(Level::Error, format!("Crane picture: {e}")),
        }
    }

    pub(super) fn toggle_hoist_3d(&mut self) {
        self.wb.hoist.three_d.open ^= true;
    }

    pub(super) fn hoist_3d_window(&mut self, ctx: &egui::Context) {
        if !self.wb.hoist.three_d.open {
            return;
        }
        let dark = ctx.global_style().visuals.dark_mode;
        let reduced = self.settings.reduce_motion;
        let mut open = true;
        let mut start = false;
        let mut open_drive = false;
        let running = self.wb.hoist.is_running();
        // The hoists of Hoist run, then the closed-loop winches of Drive run.
        let hoists = self.wb.hoist.hoist_ids();
        let hoist_count = hoists.len();
        let names: Vec<String> = hoists
            .into_iter()
            .chain(self.wb.drive_run.winch_ids())
            .collect();
        if self.wb.hoist.three_d.follow >= names.len().max(1) {
            self.wb.hoist.three_d.follow = 0;
        }
        let follow = self.wb.hoist.three_d.follow;
        // What the picture needs of the hoist, taken before the window borrows the state mutably.
        let line = match follow.checked_sub(hoist_count) {
            None => self.wb.hoist.module_at(follow).map(hook_line),
            Some(i) => self.wb.drive_run.winch_at(i).map(winch_line),
        };
        // The travel axes of the project, and what the axes picked for the bridge and the trolley
        // did this frame.
        let axes: Vec<String> = self
            .backend
            .document()
            .map(|d| super::drive_run::travel_ids(d.project()))
            .unwrap_or_default();
        self.sync_crane_setup(ctx, &axes);
        let dt = f64::from(ctx.input(|i| i.stable_dt).clamp(0.0, 0.1));
        let hook_m = match follow.checked_sub(hoist_count) {
            None => self
                .wb
                .hoist
                .module_at(follow)
                .map_or(0.0, |m| m.height_m() as f32),
            Some(i) => self
                .wb
                .drive_run
                .winch_at(i)
                .map_or(0.0, |m| m.height_m() as f32),
        };
        let driven = {
            let g = &self.wb.hoist.three_d;
            let drive = &self.wb.drive_run;
            [
                g.bridge_axis.as_deref().and_then(|id| drive.travel_of(id)),
                g.trolley_axis.as_deref().and_then(|id| drive.travel_of(id)),
            ]
        };
        let rope_m = f64::from(rope_length_mm(&self.wb.hoist.three_d.layout, hook_m)) / 1000.0;
        let travel = self.wb.hoist.three_d.travel(dt, driven, rope_m);
        let hoist_state = match follow.checked_sub(hoist_count) {
            None => self
                .wb
                .hoist
                .module_at(follow)
                .map(|m| state_of(Some(m), travel.bridge_m, travel.trolley_m)),
            Some(i) => self
                .wb
                .drive_run
                .winch_at(i)
                .map(|m| winch_state_of(m, travel.bridge_m, travel.trolley_m)),
        };
        // The end limits of the picked axes, as flags: where the axis starts plus the distance
        // set on the motor.
        let (bridge_marks, trolley_marks) = {
            let g = &self.wb.hoist.three_d;
            let project = self.backend.document().map(|d| d.project());
            (
                travel_marks(project, g.bridge_axis.as_deref(), g.bridge_m),
                travel_marks(project, g.trolley_axis.as_deref(), g.trolley_m),
            )
        };
        let outside = [
            (&bridge_marks, self.wb.hoist.three_d.layout.runway_m),
            (&trolley_marks, self.wb.hoist.three_d.layout.span_m),
        ]
        .iter()
        .map(|(marks, end)| {
            marks
                .iter()
                .filter(|m| !(0.0..=*end).contains(&m.position_m))
                .count()
        })
        .sum::<usize>();
        let g = &mut self.wb.hoist.three_d;
        let swinging = g.swinging();
        let drive_running = self.wb.drive_run.is_running();
        // The window opens at the right edge: the Live card at the left keeps its buttons.
        super::panel::dialog(ctx, "Hoist in 3D", [760.0, 540.0])
            .default_pos([(ctx.content_rect().max.x - 770.0).max(310.0), 80.0])
            .open(super::panel::esc(ctx, &mut open))
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if names.len() > 1 {
                        egui::ComboBox::from_id_salt("hoist3d_follow")
                            .selected_text(names.get(g.follow).cloned().unwrap_or_default())
                            .show_ui(ui, |ui| {
                                for (i, n) in names.iter().enumerate() {
                                    ui.selectable_value(&mut g.follow, i, n);
                                }
                            });
                    }
                    let mut refit = false;
                    ui.label(if g.bridge_axis.is_some() {
                        "Bridge start"
                    } else {
                        "Bridge"
                    });
                    ui.add(
                        egui::DragValue::new(&mut g.bridge_m)
                            .range(0.0..=g.layout.runway_m)
                            .speed(0.05)
                            .suffix(" m"),
                    );
                    ui.label(if g.trolley_axis.is_some() {
                        "Trolley start"
                    } else {
                        "Trolley"
                    });
                    ui.add(
                        egui::DragValue::new(&mut g.trolley_m)
                            .range(0.0..=g.layout.span_m)
                            .speed(0.05)
                            .suffix(" m"),
                    );
                    ui.label("Runway");
                    refit |= ui
                        .add(
                            egui::DragValue::new(&mut g.layout.runway_m)
                                .range(4.0..=80.0)
                                .suffix(" m"),
                        )
                        .changed();
                    ui.label("Span");
                    refit |= ui
                        .add(
                            egui::DragValue::new(&mut g.layout.span_m)
                                .range(2.0..=40.0)
                                .suffix(" m"),
                        )
                        .changed();
                    ui.label("Rail height");
                    refit |= ui
                        .add(
                            egui::DragValue::new(&mut g.layout.rail_m)
                                .range(3.0..=30.0)
                                .suffix(" m"),
                        )
                        .changed();
                    ui.label("Hook start");
                    ui.add(
                        egui::DragValue::new(&mut g.layout.hook_start_m)
                            .range(0.5..=25.0)
                            .suffix(" m"),
                    )
                    .on_hover_text(
                        "Height of the top of the hook block above the floor at hook height 0",
                    );
                    let mut with_load = g.layout.load_m.is_some();
                    if ui.checkbox(&mut with_load, "Load").changed() {
                        g.layout.load_m = with_load.then_some([1.2, 0.8, 0.8]);
                    }
                    if ui.button("Fit").clicked() || refit {
                        g.needs_fit = true;
                    }
                    if ui
                        .button("Hook")
                        .on_hover_text("Frame the hook, the rope and the load")
                        .clicked()
                    {
                        g.frame_hook = true;
                    }
                    for (label, preset) in [
                        ("Front", ViewPreset::Front),
                        ("Side", ViewPreset::Right),
                        ("Iso", ViewPreset::Iso),
                    ] {
                        if ui.button(label).clicked() {
                            g.pending_preset = Some(preset);
                        }
                    }
                });
                g.bridge_m = g.bridge_m.clamp(0.0, g.layout.runway_m);
                g.trolley_m = g.trolley_m.clamp(0.0, g.layout.span_m);
                ui.horizontal_wrapped(|ui| {
                    ui.label("Bridge driven by");
                    axis_picker(ui, "hoist3d_bridge_axis", &axes, &mut g.bridge_axis);
                    ui.label("Trolley driven by");
                    axis_picker(ui, "hoist3d_trolley_axis", &axes, &mut g.trolley_axis);
                    if g.bridge_axis.is_some() || g.trolley_axis.is_some() {
                        let deg = |r: f32| r.to_degrees();
                        ui.weak(format!(
                            "Swing {:+.1}\u{b0} along the runway, {:+.1}\u{b0} across",
                            deg(travel.swing_rad[0]),
                            deg(travel.swing_rad[1])
                        ));
                    }
                });
                if axes.is_empty() {
                    ui.weak(
                        "No travel axis in this project: enter a moving mass under Travel drive \
                         in a motor's property sheet, run it in Simulate > Drive run, then pick \
                         it here.",
                    );
                } else if (g.bridge_axis.is_some() || g.trolley_axis.is_some()) && !drive_running {
                    ui.horizontal(|ui| {
                        ui.weak("The axes move while Drive run runs.");
                        if ui.button("Drive run").clicked() {
                            open_drive = true;
                        }
                    });
                }
                for (at_end, what) in [
                    (travel.bridge_at_end, "bridge"),
                    (travel.trolley_at_end, "trolley"),
                ] {
                    if at_end {
                        ui.colored_label(
                            crate::theme::ui_palette().warning,
                            format!(
                                "\u{2022} The {what} went past the end of the picture: it holds \
                                 at the end (the drive does not know)"
                            ),
                        );
                    }
                }
                if outside > 0 {
                    ui.weak(format!(
                        "\u{2022} {outside} end limit(s) lie outside the picture and are not drawn"
                    ));
                }
                match &line {
                    Some(text) => {
                        ui.label(text);
                    }
                    None if running => {
                        ui.weak("This run has no hoist.");
                    }
                    None => {
                        ui.horizontal(|ui| {
                            ui.weak("No run: the hook stands at its start. Press Start in");
                            if ui.button("Hoist run").clicked() {
                                start = true;
                            }
                        });
                    }
                }
                let state = CraneState {
                    swing_rad: travel.swing_rad,
                    bridge_limits: bridge_marks,
                    trolley_limits: trolley_marks,
                    ..hoist_state
                        .clone()
                        .unwrap_or_else(|| state_of(None, travel.bridge_m, travel.trolley_m))
                };
                let scene = g.scene_for(&state);
                let theme = if dark {
                    View3dTheme::Dark
                } else {
                    View3dTheme::Light
                };
                let now = ui.input(|i| i.time);
                if g.needs_fit {
                    g.needs_fit = false;
                    g.view.fit(&scene, now, reduced);
                    g.pending_preset = g.pending_preset.or(Some(ViewPreset::Iso));
                }
                if let Some(p) = g.pending_preset.take() {
                    g.view.go_to(p, &scene, now, reduced);
                }
                if std::mem::take(&mut g.frame_hook) {
                    let b = hook_bounds(&g.layout, &state);
                    g.view.frame_bounds(&b, now, reduced);
                }
                let height = (ui.available_height() - 190.0).max(160.0);
                ui.allocate_ui(egui::vec2(ui.available_width(), height), |ui| {
                    g.view.show(
                        ui,
                        &scene,
                        &g.library,
                        &Look::for_theme(theme),
                        ViewOptions {
                            reduced_motion: reduced,
                        },
                    );
                });
                ui.label(egui::RichText::new("Picture, not measurement").strong());
                egui::ScrollArea::vertical()
                    .id_salt("hoist3d_notes")
                    .max_height(150.0)
                    .show(ui, |ui| {
                        let winch_notes: &[&str] = if g.follow >= hoist_count {
                            WINCH_PICTURE_LEAVES_OUT
                        } else {
                            &[]
                        };
                        for line in CRANE_PICTURE_LEAVES_OUT
                            .iter()
                            .chain(SWING_MODEL_LEAVES_OUT)
                            .chain(winch_notes)
                        {
                            ui.weak(format!("\u{2022} {line}"));
                        }
                    });
            });
        if !open {
            self.wb.hoist.three_d.open = false;
        }
        if start {
            self.wb.hoist.open = true;
        }
        if open_drive {
            self.wb.drive_run.open = true;
        }
        if self.wb.hoist.three_d.open && (running || drive_running || swinging) {
            ctx.request_repaint();
        }
    }
}

/// A combo box over the travel axes with "none" first.
fn axis_picker(ui: &mut egui::Ui, id: &str, axes: &[String], choice: &mut Option<String>) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(choice.clone().unwrap_or_else(|| "none".to_owned()))
        .show_ui(ui, |ui| {
            ui.selectable_value(choice, None, "none");
            for a in axes {
                ui.selectable_value(choice, Some(a.clone()), a);
            }
        });
}

impl Hoist3d {
    /// The scene for `state`, rebuilt only when the layout or the numbers changed.
    fn scene_for(&mut self, state: &CraneState) -> Arc<Scene> {
        if let Some((l, s, scene)) = &self.built
            && *l == self.layout
            && s == state
        {
            return scene.clone();
        }
        self.revision += 1;
        let mut scene = build(&self.layout, state);
        scene.revision = self.revision;
        let scene = Arc::new(scene);
        self.built = Some((self.layout, state.clone(), scene.clone()));
        scene
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_a_driven_axis_the_picture_stands_where_it_was_typed_and_does_not_swing() {
        let mut g = Hoist3d::default();
        let (b, t) = (g.bridge_m, g.trolley_m);
        let travel = g.travel(0.016, [None, None], 4.0);
        assert_eq!((travel.bridge_m, travel.trolley_m), (b, t));
        assert_eq!(travel.swing_rad, [0.0, 0.0]);
        assert!(!g.swinging());
    }

    #[test]
    fn a_driven_axis_adds_its_position_to_the_typed_start() {
        let mut g = Hoist3d::default();
        let travel = g.travel(0.016, [Some((1.25, 0.0)), Some((-0.5, 0.0))], 4.0);
        assert!((travel.bridge_m - (g.bridge_m + 1.25)).abs() < 1e-5);
        assert!((travel.trolley_m - (g.trolley_m - 0.5)).abs() < 1e-5);
    }

    #[test]
    fn an_axis_past_the_end_is_held_there_and_reported() {
        let mut g = Hoist3d::default();
        let travel = g.travel(0.016, [Some((100.0, 0.5)), Some((100.0, 0.5))], 4.0);
        assert_eq!(travel.bridge_m, g.layout.runway_m);
        assert_eq!(travel.trolley_m, g.layout.span_m);
        assert!(travel.bridge_at_end && travel.trolley_at_end);
        // Held axes carry no speed, so they do not swing the hook.
        assert_eq!(g.last_speed, [0.0, 0.0]);
        let travel = g.travel(0.016, [Some((-100.0, 0.0)), None], 4.0);
        assert_eq!(travel.bridge_m, 0.0);
        assert!(travel.bridge_at_end);
    }

    #[test]
    fn a_speed_that_appears_out_of_nothing_is_not_a_jolt() {
        let mut g = Hoist3d::default();
        // A run that starts with the axis already at speed (a window opened late) does not
        // throw the hook about.
        let _ = g.travel(0.016, [Some((3.0, 4.0)), None], 4.0);
        assert!(g.swing[0].angle_rad().abs() < 1e-9);
    }

    #[test]
    fn speeding_up_leans_the_hook_back_and_it_keeps_swinging_after_the_axis_stops() {
        let mut g = Hoist3d::default();
        // 1 m/s^2 for 2 s, then the axis stops dead.
        let mut v = 0.0;
        let mut pos = 0.0;
        let mut lean = 0.0_f32;
        for _ in 0..125 {
            v += 1.0 * 0.016;
            pos += v * 0.016;
            let t = g.travel(0.016, [Some((pos, v)), None], 4.0);
            lean = lean.min(t.swing_rad[0]);
        }
        assert!(lean < -0.05, "{lean}");
        let t = g.travel(0.016, [Some((pos, 0.0)), None], 4.0);
        assert!(t.swing_rad[0] != 0.0);
        assert!(g.swinging());
    }

    #[test]
    fn the_scene_changes_when_the_hook_swings_and_not_when_nothing_moved() {
        let mut g = Hoist3d::default();
        let still = CraneState::default();
        let a = g.scene_for(&still);
        let rev = g.revision;
        let b = g.scene_for(&still);
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(g.revision, rev);
        let swung = CraneState {
            swing_rad: [0.1, 0.0],
            ..Default::default()
        };
        let c = g.scene_for(&swung);
        assert!(!Arc::ptr_eq(&a, &c));
        assert!(g.revision > rev);
    }
}
