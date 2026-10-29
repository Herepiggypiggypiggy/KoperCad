//! Hoist in 3D (`sim.hoist3d`, Simulate > Hoist in 3D, or the button in Hoist run): the hook of
//! a simulated hoist drawn on a generic overhead crane. The numbers come from the hoist loop of
//! the Hoist run window (hook height, the conditions it reports, the heights of the bound limit
//! switches); the crane around them is a generic picture, see `CRANE_PICTURE_LEAVES_OUT`.

use super::NovoApp;
use super::hoist_run::conditions;
use eframe::egui;
use novo_machine::hoist_loop::{HoistModule, LimitKind};
use novo_view3d::camera::ViewPreset;
use novo_view3d::crane::{
    CRANE_PICTURE_LEAVES_OUT, CraneLayout, CraneState, LimitFlag, LimitMark, build, hook_bounds,
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
            bridge_m: layout.runway_m * 0.5,
            trolley_m: layout.span_m * 0.5,
            layout,
            follow: 0,
            built: None,
            revision: 0,
            needs_fit: true,
            pending_preset: None,
            frame_hook: false,
        }
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
        warning: !conditions(m).is_empty(),
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
        let running = self.wb.hoist.is_running();
        let names: Vec<String> = self.wb.hoist.hoist_ids();
        if self.wb.hoist.three_d.follow >= names.len().max(1) {
            self.wb.hoist.three_d.follow = 0;
        }
        let follow = self.wb.hoist.three_d.follow;
        // What the picture needs of the hoist, taken before the window borrows the state mutably.
        let line = self.wb.hoist.module_at(follow).map(hook_line);
        let g = &mut self.wb.hoist.three_d;
        let (bridge, trolley) = (g.bridge_m, g.trolley_m);
        let hoist_state = self
            .wb
            .hoist
            .module_at(follow)
            .map(|m| state_of(Some(m), bridge, trolley));
        let g = &mut self.wb.hoist.three_d;
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
                    ui.label("Bridge");
                    ui.add(
                        egui::DragValue::new(&mut g.bridge_m)
                            .range(0.0..=g.layout.runway_m)
                            .speed(0.05)
                            .suffix(" m"),
                    );
                    ui.label("Trolley");
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
                let state = match &hoist_state {
                    Some(h) => CraneState {
                        bridge_m: g.bridge_m,
                        trolley_m: g.trolley_m,
                        ..h.clone()
                    },
                    None => state_of(None, g.bridge_m, g.trolley_m),
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
                let height = (ui.available_height() - 120.0).max(160.0);
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
                for line in CRANE_PICTURE_LEAVES_OUT {
                    ui.weak(format!("\u{2022} {line}"));
                }
            });
        if !open {
            self.wb.hoist.three_d.open = false;
        }
        if start {
            self.wb.hoist.open = true;
        }
        if self.wb.hoist.three_d.open && running {
            ctx.request_repaint();
        }
    }
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
