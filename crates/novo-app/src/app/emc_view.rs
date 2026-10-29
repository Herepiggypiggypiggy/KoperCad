//! The EMC view of the 3D cabinet: wires in the colour of their cable class,
//! keep-clear zones round the noisy runs, a glow round the noisy parts,
//! numbered markers where something is likely to go wrong, and optional
//! layers for the exposure map, loop areas, bonding and shield ends. The
//! panel lists the markers and explains the picked one; the legend sits
//! apart so it is always whole.
//!
//! Where the user finds it: View > EMC (Ctrl+K "EMC"), or the EMC button in
//! the 3D toolbar. It is an indicative model, not a field solver, and says
//! so on every card (`docs/EMC.md`).

use std::sync::Arc;

use eframe::egui;
use glam::Vec3;
use novo_core::{Device, DeviceId, Project, StructureId, WireId};
use novo_view3d::Overlay;
use novo_view3d::emc::{self, Class, EmcLayers, Hotspot, Level, Picture, Provenance, Style};
use novo_view3d::scene::{ItemKind, Scene};
use novo_view3d::widget::View3d;

use super::emc_stub::{self, DeviceIn, EndIn, Family, WireIn};

/// Panel width.
const WIDTH: f32 = 340.0;
/// Rows of the hot spot list shown before it scrolls.
const LIST_ROWS: usize = 6;

type Key = (StructureId, u64, u64);

/// A fix being previewed, as the views see it: the project as it would be
/// after the fix, drawn in place of the real one while `after` is on.
pub(crate) struct WhatIfView {
    pub(super) id: u64,
    pub(super) project: Arc<Project>,
    pub(super) hint: String,
    /// One line: findings before and after, what it clears.
    pub(super) summary: String,
    pub(super) after: bool,
}

/// What the person asked for on the what-if banner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WhatIfAction {
    Show { after: bool },
    Apply,
    Cancel,
}

/// What the 3D view keeps of the EMC view between frames.
#[derive(Default)]
pub(crate) struct EmcState {
    pub(super) on: bool,
    pub(super) layers: EmcLayers,
    pub(super) selected: Option<u32>,
    picture: Option<(Key, Arc<Picture>)>,
    overlays: Vec<Overlay>,
    overlay_key: Option<(Key, EmcLayers, Option<u32>, bool)>,
    tinted: Option<((Key, EmcLayers, bool), Arc<Scene>)>,
    /// Wires the plate view leaves out because no duct reaches both ends.
    pub(super) left_out: usize,
    /// The exposure map of the picture, made once per picture.
    field: Option<(Key, Arc<emc::Field>)>,
    /// The fix being previewed (set by the app each frame).
    pub(super) whatif: Option<WhatIfView>,
    /// What the banner asked for; the app takes it.
    pub(super) whatif_action: Option<WhatIfAction>,
    /// Controls the lessons point at, set by the panel and read by the
    /// workspace that drew it: (anchor, where, label, clicked this frame).
    pub(super) marks: Vec<(&'static str, egui::Rect, &'static str, bool)>,
    /// The chip a lesson step points at, scrolled into view once when set.
    pub(super) reveal: Option<&'static str>,
}

impl EmcState {
    /// The project the picture is made from and a salt for its cache key:
    /// the previewed copy while the banner shows "After".
    fn shown<'a>(&self, project: &'a Project) -> (Option<Arc<Project>>, u64, &'a Project) {
        match self.whatif.as_ref().filter(|w| w.after) {
            Some(w) => (Some(w.project.clone()), w.id, project),
            None => (None, 0, project),
        }
    }
}

/// The theme's colours as the overlays take them.
pub(crate) fn style(dark: bool) -> Style {
    Style {
        class: novo_theme::emc::classes(dark),
        ramp: |t| novo_theme::heat::ramp(t),
        earth: novo_theme::emc::earth(dark),
        good: novo_theme::emc::good(dark),
    }
}

fn color(c: [u8; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(c[0], c[1], c[2])
}

/// The family of a device, from the library key of its main symbol (the
/// first symbol when it has none marked main).
fn family_of_device(project: &Project, device: u64) -> Family {
    let symbols = project.symbols_of_device(DeviceId::from(device));
    symbols
        .iter()
        .find(|s| matches!(s.role, novo_core::SymbolRole::Device { main: true, .. }))
        .or(symbols.first())
        .map_or(Family::Other, |s| emc_stub::family_of(&s.symbol.0.clone()))
}

/// One wire as the stub reads it: its ends, colour and section from the
/// project, its path as given. `stripe`: drawn bi-colour, a protective conductor.
fn wire_in(project: &Project, raw: u64, stripe: bool, path: Vec<[f32; 3]>) -> Option<WireIn> {
    let mw = project.wire(WireId::from(raw))?;
    let end = |e: novo_core::Endpoint| {
        let pin = e.as_pin()?;
        let sym = project.symbol(pin.symbol)?;
        let device = novo_routing::model::device_of_pin(project, pin).map(|d| d.raw());
        let key = sym.symbol.0.clone();
        Some(EndIn {
            device,
            family: emc_stub::family_of(&key),
            pin: sym
                .pins
                .get(usize::from(pin.pin))
                .map(|p| p.name.clone())
                .unwrap_or_default(),
            key,
        })
    };
    let label = mw
        .number
        .clone()
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| format!("#{raw}"));
    let colour = if stripe {
        Some("green-yellow".to_owned())
    } else {
        mw.color.clone()
    };
    Some(WireIn {
        id: raw,
        label,
        ends: (end(mw.from), end(mw.to)),
        colour,
        section_mm2: novo_routing::model::wire_section_mm2(project, mw),
        path,
    })
}

/// Reads the placed devices and routed wires of a plate out of the built
/// scene and the project.
fn inputs(project: &Project, scene: &Scene) -> (Vec<DeviceIn>, Vec<WireIn>) {
    let frame = scene.frame;
    let devices = scene
        .items
        .iter()
        .filter(|i| i.id.kind == ItemKind::Device)
        .filter_map(|i| {
            let d = project.get::<Device>(DeviceId::from(i.id.raw))?;
            let b = scene.world_bounds(i);
            let (x0, y0) = frame.plate(b.min);
            let (x1, y1) = frame.plate(b.max);
            let (x, y) = (x0.min(x1) as f32, y0.min(y1) as f32);
            Some(DeviceIn {
                id: i.id.raw,
                tag: d
                    .tag
                    .device_code()
                    .map_or_else(|| format!("device {}", i.id.raw), |c| format!("-{c}")),
                family: family_of_device(project, i.id.raw),
                rect: [x, y, (x1 - x0).abs() as f32, (y1 - y0).abs() as f32],
                height: i.size.z,
            })
        })
        .collect();
    let wires = scene
        .wires
        .iter()
        .filter_map(|w| {
            let path = w
                .points
                .iter()
                .map(|p| {
                    let (x, y) = frame.plate(*p);
                    [x as f32, y as f32, p.z]
                })
                .collect();
            wire_in(project, w.id.raw, w.stripe.is_some(), path)
        })
        .collect();
    (devices, wires)
}

impl EmcState {
    /// The picture of the mounting plate for the Cabinet's 2D view, from the
    /// routes the Cabinet already computed and the placements of the plate.
    pub(super) fn sync_plate(
        &mut self,
        project: &Project,
        id: StructureId,
        routes: impl FnOnce() -> Vec<super::cabinet_routes::RouteLine>,
    ) {
        let (copy, salt, project) = self.shown(project);
        let project = copy.as_deref().unwrap_or(project);
        let key: Key = (id, project.revision(), salt);
        if self.picture.as_ref().map(|p| p.0) == Some(key) {
            return;
        }
        let routes = routes();
        self.left_out = routes.iter().filter(|r| r.free_air).count();
        let Some(layout) = novo_layout::model::load(project, id) else {
            self.left_out = 0;
            self.picture = None;
            return;
        };
        let devices: Vec<DeviceIn> = layout
            .placements
            .iter()
            .filter(|p| !layout.off_plate.contains_key(&p.id))
            .filter_map(|p| {
                let d = project.get::<Device>(DeviceId::from(p.id))?;
                let r = p.footprint();
                Some(DeviceIn {
                    id: p.id,
                    tag: d
                        .tag
                        .device_code()
                        .map_or_else(|| format!("device {}", p.id), |c| format!("-{c}")),
                    family: family_of_device(project, p.id),
                    rect: [r.x as f32, r.y as f32, r.w as f32, r.h as f32],
                    height: layout.depths.get(&p.id).copied().unwrap_or(60.0) as f32,
                })
            })
            .collect();
        let wires: Vec<WireIn> = routes
            .iter()
            .filter(|r| !r.free_air)
            .filter_map(|r| {
                let path = r.points.iter().map(|&(x, y)| [x, y, 10.0]).collect();
                wire_in(project, r.wire, false, path)
            })
            .collect();
        let size = [layout.plate.width as f32, layout.plate.height as f32];
        let drawn = wires.iter().map(|w| w.id).collect();
        let pic = super::emc_adapter::from_project(project, id, size, &drawn)
            .unwrap_or_else(|| emc_stub::build(size, &devices, &wires));
        if let Some(sel) = self.selected
            && pic.hotspot(sel).is_none()
        {
            self.selected = None;
        }
        self.picture = Some((key, Arc::new(pic)));
    }

    /// Brings the picture and the overlays up to date. True when the overlays
    /// changed, so the host redraws them.
    pub(super) fn sync(
        &mut self,
        project: &Project,
        id: StructureId,
        revision: u64,
        scene: &Scene,
        dark: bool,
    ) -> bool {
        if !self.on {
            let had = !self.overlays.is_empty();
            self.overlays.clear();
            self.overlay_key = None;
            return had;
        }
        let (copy, salt, project) = self.shown(project);
        let project = copy.as_deref().unwrap_or(project);
        let key: Key = (id, revision ^ (salt << 44), scene.revision);
        if self.picture.as_ref().map(|p| p.0) != Some(key) {
            let (devices, wires) = inputs(project, scene);
            let size = [scene.frame.width, scene.frame.height];
            let drawn = wires.iter().map(|w| w.id).collect();
            let pic = super::emc_adapter::from_project(project, id, size, &drawn)
                .unwrap_or_else(|| emc_stub::build(size, &devices, &wires));
            if let Some(sel) = self.selected
                && pic.hotspot(sel).is_none()
            {
                self.selected = None;
            }
            self.picture = Some((key, Arc::new(pic)));
        }
        let want = (key, self.layers, self.selected, dark);
        if self.overlay_key == Some(want) {
            return false;
        }
        let pic = &self.picture.as_ref().expect("built above").1;
        self.overlays = emc::overlays(pic, &scene.frame, &style(dark), &self.layers, self.selected);
        self.overlay_key = Some(want);
        true
    }

    pub(super) fn overlays(&self) -> &[Overlay] {
        &self.overlays
    }

    /// The exposure map of the current picture, made once per picture.
    pub(super) fn exposure(&mut self) -> Option<Arc<emc::Field>> {
        let (key, pic) = self.picture.as_ref()?;
        if self.field.as_ref().map(|f| f.0) != Some(*key) {
            self.field = Some((*key, Arc::new(emc::field::exposure(pic))));
        }
        self.field.as_ref().map(|f| f.1.clone())
    }

    pub(super) fn picture(&self) -> Option<&Arc<Picture>> {
        self.picture.as_ref().map(|p| &p.1)
    }

    /// The scene with wires and devices in their EMC colours, when the view
    /// is on and the Classes layer too.
    pub(super) fn tinted(&mut self, scene: &Arc<Scene>, dark: bool) -> Option<Arc<Scene>> {
        if !self.on || !self.layers.classes {
            return None;
        }
        let (key, pic) = self.picture.as_ref()?;
        let want = (*key, self.layers, dark);
        if self.tinted.as_ref().map(|t| t.0) != Some(want) {
            let tinted = emc::tint_scene(scene, pic, &style(dark), &self.layers, key.1 ^ 0x5EC);
            self.tinted = Some((want, Arc::new(tinted)));
        }
        self.tinted.as_ref().map(|t| t.1.clone())
    }
}

/// Black or white text, whichever reads on `bg`.
pub(super) fn text_on(bg: [u8; 3]) -> egui::Color32 {
    let lin = |v: u8| {
        let s = f32::from(v) / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    let l = 0.2126 * lin(bg[0]) + 0.7152 * lin(bg[1]) + 0.0722 * lin(bg[2]);
    if l > 0.28 {
        egui::Color32::from_rgb(0x14, 0x14, 0x16)
    } else {
        egui::Color32::WHITE
    }
}

/// Numbered badges on the markers; a click picks one. Returns the picked id.
pub(super) fn paint_badges(
    ui: &egui::Ui,
    view: &View3d,
    rect: egui::Rect,
    scene: &Scene,
    pic: &Picture,
    selected: Option<u32>,
    dark: bool,
) -> Option<u32> {
    let st = style(dark);
    let pal = crate::theme::ui_palette();
    let painter = ui.painter().with_clip_rect(rect);
    let font = egui::FontId::proportional(12.0);
    let mut picked = None;
    // Placed worst first; a badge that would cover another moves down, then
    // drawn weakest first so the worst is on top.
    let mut spots: Vec<&Hotspot> = pic.hotspots.iter().collect();
    spots.sort_by(|a, b| b.risk.total_cmp(&a.risk));
    let mut placed: Vec<(&Hotspot, egui::Rect, [u8; 3])> = Vec::new();
    for h in spots {
        let at = scene
            .frame
            .world(f64::from(h.at[0]), f64::from(h.at[1]), f64::from(h.at[2]));
        let Some(p) = view.project(rect, scene, Vec3::new(at.x, at.y, at.z + 14.0)) else {
            continue;
        };
        let size = painter
            .layout_no_wrap(emc::label(h), font.clone(), egui::Color32::WHITE)
            .size()
            + egui::vec2(12.0, 6.0);
        // Left of its point: device tags sit to the right of a device's centre
        // line, so the badge does not cover them.
        let mut chip = egui::Rect::from_center_size(p - egui::vec2(size.x * 0.5 + 8.0, 0.0), size);
        for _ in 0..8 {
            if placed
                .iter()
                .any(|(_, r, _)| r.expand(1.0).intersects(chip))
            {
                chip = chip.translate(egui::vec2(0.0, size.y + 2.0));
            } else {
                break;
            }
        }
        placed.push((h, chip, st.risk(0.55 + 0.45 * h.risk)));
    }
    for (h, chip, fill) in placed.into_iter().rev() {
        let size = chip.size();
        let text = text_on(fill);
        // A short stalk to the point the badge speaks about.
        painter.line_segment(
            [
                chip.right_center(),
                chip.right_center() + egui::vec2(8.0, 0.0),
            ],
            egui::Stroke::new(1.5, color(fill)),
        );
        let galley = painter.layout_no_wrap(emc::label(h), font.clone(), text);
        let on = selected == Some(h.id);
        painter.rect(
            chip,
            size.y * 0.5,
            color(fill),
            egui::Stroke::new(
                if on { 2.5 } else { 1.5 },
                if on { pal.accent } else { pal.panel },
            ),
            egui::StrokeKind::Outside,
        );
        painter.galley(chip.min + egui::vec2(6.0, 3.0), galley, text);
        let r = ui.interact(
            chip,
            egui::Id::new(("emc_badge", h.id)),
            egui::Sense::click(),
        );
        if r.clicked() {
            picked = Some(h.id);
        }
        r.on_hover_text(format!("{}: {}", emc::label(h), h.title));
    }
    picked
}

fn risk_word(risk: f32) -> &'static str {
    if risk >= 0.7 {
        "high"
    } else if risk >= 0.4 {
        "medium"
    } else {
        "low"
    }
}

/// A canvas shorter than this (points) starts the legend rolled up.
const SHORT_CANVAS: f32 = 700.0;

/// The legend: what each colour on screen means, for the layers that are on.
/// A panel of its own at the bottom left, fitted to its content, so it is
/// never scrolled out of reach.
pub(super) fn legend(ctx: &egui::Context, view: egui::Rect, state: &EmcState, dark: bool) {
    let Some(pic) = state.picture() else {
        return;
    };
    let st = style(dark);
    let pal = crate::theme::ui_palette();
    // On a short canvas the open legend covers the devices it explains, so it
    // starts rolled up (once; the person's own fold is kept after that).
    let seen = egui::Id::new("emc_legend_first_look");
    if view.height() < SHORT_CANVAS && !ctx.data(|d| d.get_temp::<bool>(seen).unwrap_or(false)) {
        ctx.data_mut(|d| d.insert_temp(seen, true));
        let mut fold = egui::collapsing_header::CollapsingState::load_with_default_open(
            ctx,
            super::panel::id("view3d_emc_legend").with("collapsing"),
            false,
        );
        fold.set_open(false);
        fold.store(ctx);
    }
    super::panel::Panel::new(
        "view3d_emc_legend",
        "Legend",
        view,
        super::panel::Side::BottomLeft,
        250.0,
    )
    .fit()
    .show(ctx, |ui| {
        let layers = &state.layers;
        if layers.classes {
            for class in Class::ALL {
                let n = pic.runs.iter().filter(|r| r.class == class).count();
                if n == 0 {
                    continue;
                }
                ui.horizontal(|ui| {
                    let (r, _) =
                        ui.allocate_exact_size(egui::vec2(26.0, 10.0), egui::Sense::hover());
                    ui.painter().rect_filled(r, 5.0, color(st.colour(class)));
                    ui.label(format!("{}  ({n})", class.name()));
                });
            }
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(26.0, 10.0), egui::Sense::hover());
                ui.painter()
                    .rect_filled(r, 5.0, egui::Color32::from_gray(0x6B));
                ui.label("Not classed");
            });
        }
        if state.left_out > 0 {
            let n = state.left_out;
            ui.label(
                egui::RichText::new(format!(
                    "{n} wire{} with no duct route {} left out of this view (see Route all)",
                    if n == 1 { "" } else { "s" },
                    if n == 1 { "is" } else { "are" },
                ))
                .small()
                .color(pal.warning),
            );
        }
        if layers.halos {
            ui.label(
                egui::RichText::new("Soft band round a run: keep sensitive wires out")
                    .small()
                    .color(pal.text_secondary),
            );
        }
        if layers.risk {
            match pic.field.as_ref().and_then(|f| f.ends.as_ref()) {
                Some((lo, mid, hi)) => {
                    heading(ui, "Exposure map (model)");
                    ramp_bar(ui, &st, lo, Some(mid), hi);
                    ui.label(
                        egui::RichText::new(
                            "Magnetic field of the drawn currents: a model, order of magnitude. The planning value is at the tick.",
                        )
                        .small()
                        .color(pal.text_secondary),
                    );
                }
                None => {
                    heading(ui, "Exposure map");
                    ramp_bar(ui, &st, "calm", None, "heavily leaned on");
                    ui.label(
                        egui::RichText::new("Exposure index: distances only")
                            .small()
                            .color(pal.text_secondary),
                    );
                }
            }
        }
        if layers.spots {
            heading(ui, "Hot spots");
            ramp_bar(ui, &st, "low risk", None, "high risk");
        }
        if layers.bonding {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(26.0, 10.0), egui::Sense::hover());
                ui.painter()
                    .rect_filled(r.with_max_x(r.center().x), 3.0, color(st.earth.0));
                ui.painter()
                    .rect_filled(r.with_min_x(r.center().x), 3.0, color(st.earth.1));
                ui.label("Earth and bonding");
            });
            if pic.bonds.iter().any(|b| b.quality != emc::Quality::Good) {
                ui.label(
                    egui::RichText::new("Glowing spot on an outline: a weak or missing bond")
                        .small()
                        .color(pal.text_secondary),
                );
            }
        }
        if layers.shields && !pic.shields.is_empty() {
            heading(ui, "Cable screen ends");
            for line in [
                "Plain ring: connected all round",
                "Ring with a tail: pigtail, the longer, the worse",
                "Ring with a cross: not connected",
            ] {
                ui.label(
                    egui::RichText::new(line)
                        .small()
                        .color(pal.text_secondary),
                );
            }
        }
        if layers.loops {
            ui.label(
                egui::RichText::new("Shaded sheet: area a circuit encloses")
                    .small()
                    .color(pal.text_secondary),
            );
        }
    });
}

fn heading(ui: &mut egui::Ui, text: &str) {
    ui.add_space(2.0);
    ui.label(egui::RichText::new(text).small().strong());
}

fn ramp_bar(ui: &mut egui::Ui, st: &Style, from: &str, mid: Option<&str>, to: &str) {
    let pal = crate::theme::ui_palette();
    let (r, _) = ui.allocate_exact_size(egui::vec2(180.0, 10.0), egui::Sense::hover());
    let n = 24;
    for i in 0..n {
        let t0 = i as f32 / n as f32;
        let seg = egui::Rect::from_min_max(
            egui::pos2(r.left() + r.width() * t0, r.top()),
            egui::pos2(
                r.left() + r.width() * (i + 1) as f32 / n as f32 + 0.5,
                r.bottom(),
            ),
        );
        ui.painter().rect_filled(seg, 0.0, color(st.risk(t0)));
    }
    let (row, _) = ui.allocate_exact_size(egui::vec2(180.0, 14.0), egui::Sense::hover());
    let font = egui::FontId::proportional(11.0);
    let p = ui.painter();
    if mid.is_some() {
        let x = r.center().x;
        p.line_segment(
            [egui::pos2(x, r.top()), egui::pos2(x, r.bottom() + 3.0)],
            egui::Stroke::new(1.5, pal.text_primary),
        );
    }
    p.text(
        row.left_center(),
        egui::Align2::LEFT_CENTER,
        from,
        font.clone(),
        pal.text_secondary,
    );
    if let Some(m) = mid {
        p.text(
            row.center(),
            egui::Align2::CENTER_CENTER,
            m,
            font.clone(),
            pal.text_secondary,
        );
    }
    p.text(
        row.right_center(),
        egui::Align2::RIGHT_CENTER,
        to,
        font,
        pal.text_secondary,
    );
}

/// The EMC panel: honesty line, layers, the hot spots and the picked one.
pub(super) fn panel(
    ctx: &egui::Context,
    view: egui::Rect,
    top: f32,
    state: &mut EmcState,
    tag_of: &dyn Fn(u64) -> String,
) {
    let Some(pic) = state.picture().cloned() else {
        return;
    };
    let pal = crate::theme::ui_palette();
    let mut close = false;
    super::panel::Panel::new("view3d_emc_panel", "EMC", view, super::panel::Side::Right, WIDTH)
        .top(top)
        .height(720.0)
        .above_card()
        .fit()
        .dockable()
        .show(ctx, |ui| {
            // Stays clear of the lesson card in the corner, so its chips and
            // cards are never under it.
            let corner = super::panel::reserved_bottom_right(ctx);
            egui::ScrollArea::vertical()
                .max_height(
                    (view.height() - top - 70.0 - corner).max(120.0),
                )
                .auto_shrink([true, true])
                .show(ui, |ui| {
                    ui.set_width(WIDTH - 24.0);
            ui.label(
                egui::RichText::new("Indicative model, not a field solver. It never shows compliance.")
                    .small()
                    .strong()
                    .color(pal.warning),
            );
            let note = match pic.provenance {
                Provenance::Engine => None,
                p => Some(p),
            };
            if let Some(p) = note {
                ui.label(egui::RichText::new(p.about()).small().color(pal.text_secondary));
            }
            if pic.provenance == Provenance::Stub {
                ui.label(
                    egui::RichText::new(
                        "If this view and Check > EMC check differ, the check governs; this plate has nothing for the rule engine yet.",
                    )
                    .small()
                    .strong()
                    .color(pal.text_secondary),
                );
            }
            whatif_banner(ui, state);
            ui.add_space(4.0);
            layer_chips(ui, &mut state.layers, &pic, &mut state.marks, state.reveal);
            ui.separator();
            egui::CollapsingHeader::new("How far to trust this")
                .id_salt("emc_trust")
                .show(ui, |ui| {
                    for a in &pic.assumptions {
                        ui.label(egui::RichText::new(format!("Assumed: {a}")).small().color(pal.text_secondary));
                    }
                    ui.label(egui::RichText::new("Not checked:").small().strong());
                    for n in &pic.not_checked {
                        ui.label(egui::RichText::new(format!("• {n}")).small().color(pal.text_secondary));
                    }
                });
            if let Some(h) = state.selected.and_then(|id| pic.hotspot(id)) {
                card(ui, h, &pic, tag_of);
                ui.separator();
            }
            if pic.hotspots.is_empty() {
                ui.label("No finding: no noisy run lies beside a sensitive one and no noisy part stands close to a sensitive one. This is a design aid, not a test result.");
            } else {
                let worst = pic.worst();
                ui.label(
                    egui::RichText::new(format!(
                        "{} hot spot{}, worst {} ({:.2})",
                        pic.hotspots.len(),
                        if pic.hotspots.len() == 1 { "" } else { "s" },
                        risk_word(worst),
                        worst
                    ))
                    .strong(),
                );
                egui::ScrollArea::vertical()
                    .id_salt("emc_rows")
                    .max_height(28.0 * LIST_ROWS as f32)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        for h in pic.ranked() {
                            let on = state.selected == Some(h.id);
                            let resp = novo_theme::widgets::row(ui, on, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(emc::label(h)).strong());
                                    ui.label(&h.title);
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.label(egui::RichText::new(format!("{:.2}", h.risk)).color(pal.text_secondary));
                                    });
                                });
                            });
                            if resp.clicked() {
                                state.selected = if on { None } else { Some(h.id) };
                            }
                        }
                    });
            }
            ui.separator();
            if ui.button("Close EMC view").clicked() {
                close = true;
            }
                });
        });
    if close {
        state.on = false;
    }
}

/// The before/after switch of a previewed fix, with Apply and Cancel.
fn whatif_banner(ui: &mut egui::Ui, state: &mut EmcState) {
    let Some(w) = state.whatif.as_ref() else {
        return;
    };
    let pal = crate::theme::ui_palette();
    let (hint, summary, after) = (w.hint.clone(), w.summary.clone(), w.after);
    let mut action = None;
    let mut marks: Vec<(&'static str, egui::Rect, &'static str, bool)> = Vec::new();
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.label(
            egui::RichText::new("Previewing a fix")
                .strong()
                .color(pal.warning),
        );
        ui.label(egui::RichText::new(hint).small());
        ui.horizontal(|ui| {
            let before_chip = novo_theme::widgets::filter_toggle(ui, "Before", !after);
            marks.push((
                "emc.whatif.before",
                before_chip.rect,
                "Before",
                before_chip.clicked(),
            ));
            if before_chip.clicked() {
                action = Some(WhatIfAction::Show { after: false });
            }
            let after_chip = novo_theme::widgets::filter_toggle(ui, "After", after);
            marks.push((
                "emc.whatif.after",
                after_chip.rect,
                "After",
                after_chip.clicked(),
            ));
            if after_chip.clicked() {
                action = Some(WhatIfAction::Show { after: true });
            }
        });
        ui.label(
            egui::RichText::new(summary)
                .small()
                .color(pal.text_secondary),
        );
        ui.label(
            egui::RichText::new(if after {
                "Showing the cabinet as it would be. Nothing is changed yet."
            } else {
                "Showing the cabinet as it is."
            })
            .small()
            .color(pal.text_secondary),
        );
        ui.horizontal(|ui| {
            let apply = ui.button("Apply (one undo step)");
            marks.push(("emc.whatif.apply", apply.rect, "Apply", apply.clicked()));
            if apply.clicked() {
                action = Some(WhatIfAction::Apply);
            }
            if ui.button("Cancel").clicked() {
                action = Some(WhatIfAction::Cancel);
            }
        });
    });
    state.marks.extend(marks);
    if action.is_some() {
        state.whatif_action = action;
    }
}

fn layer_chips(
    ui: &mut egui::Ui,
    l: &mut EmcLayers,
    pic: &Picture,
    marks: &mut Vec<(&'static str, egui::Rect, &'static str, bool)>,
    reveal: Option<&'static str>,
) {
    let nothing = "Nothing of this kind in this cabinet";
    let chip =
        |ui: &mut egui::Ui, on: &mut bool, text: &str, tip: &str, enabled: bool, off: &str| {
            // Not `add_enabled_ui`: a scope per chip cannot wrap onto the next row.
            if !enabled {
                // "n/a" in the label: an unavailable chip must not look like
                // one that is merely switched off.
                return ui
                    .add_enabled(false, egui::Button::new(format!("{text} n/a")))
                    .on_disabled_hover_text(off);
            }
            let r = novo_theme::widgets::filter_toggle(ui, text, *on).on_hover_text(tip);
            if r.clicked() {
                *on = !*on;
            }
            r
        };
    ui.horizontal_wrapped(|ui| {
        // A chip moves to the next row instead of wrapping its own text.
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        chip(ui, &mut l.classes, "Classes", "Wires in the colour of their cable class, devices by role", true, nothing);
        chip(ui, &mut l.halos, "Keep-clear", "A soft band round each noisy run: keep sensitive wires out of it", true, nothing);
        chip(ui, &mut l.sources, "Noise glow", "A glow round each noisy part, as far as its loudness reaches (an index, not a field)", true, nothing);
        chip(ui, &mut l.spots, "Hot spots", "Numbered markers where something is likely to go wrong", true, nothing);
        let exposure_tip = if pic.field.is_some() {
            "The magnetic field of the currents in the drawn wires, a little above the plate. A model with stated assumptions, not a measurement"
        } else {
            "How strongly each point of the plate is leaned on by the noisy parts and runs. Geometry only"
        };
        chip(ui, &mut l.risk, "Exposure map", exposure_tip, true, nothing);
        chip(ui, &mut l.loops, "Loops", "The area each pair of go and return wires encloses", !pic.loops.is_empty(), nothing);
        let bonding = chip(ui, &mut l.bonding, "Bonding", "Case-to-plate bonding of drives and filters, from the stated plate finish or bonding", !pic.bonds.is_empty(), if pic.provenance == Provenance::Engine { "No bonding drawn: no drive or filter on this plate has a stated plate finish or bonding" } else { nothing });
        let shields = chip(ui, &mut l.shields, "Shields", "Where each cable screen ends", !pic.shields.is_empty(), nothing);
        // A short panel (beside the lesson card) scrolls to the chip the step names.
        let seen = egui::Id::new("emc_plate_revealed");
        if reveal.is_some() && ui.ctx().data(|d| d.get_temp::<Option<&'static str>>(seen)) != Some(reveal) {
            match reveal {
                Some("emc.plate.bonding") => bonding.scroll_to_me(Some(egui::Align::Center)),
                Some("emc.plate.shields") => shields.scroll_to_me(Some(egui::Align::Center)),
                _ => {}
            }
        }
        ui.ctx().data_mut(|d| d.insert_temp(seen, reveal));
        marks.push(("emc.plate.bonding", bonding.rect, "Bonding", bonding.clicked()));
        marks.push(("emc.plate.shields", shields.rect, "Shields", shields.clicked()));
    });
}

fn card(ui: &mut egui::Ui, h: &Hotspot, pic: &Picture, tag_of: &dyn Fn(u64) -> String) {
    let pal = crate::theme::ui_palette();
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("{}  {}", emc::label(h), h.title))
                .strong()
                .size(15.0),
        );
    });
    ui.horizontal_wrapped(|ui| {
        let tip = h.level.about();
        let lvl = match h.level {
            Level::Rule => "Rule",
            Level::Index => "Index",
            Level::Estimate => "Estimate",
            Level::Field => "Field model",
        };
        ui.label(egui::RichText::new(lvl).small().strong().color(pal.accent))
            .on_hover_text(tip);
        ui.label(
            egui::RichText::new(if h.rule.is_empty() {
                h.kind.name().to_string()
            } else {
                format!("{} · {}", h.kind.name(), h.rule)
            })
            .small()
            .color(pal.text_secondary),
        );
        if let Some(band) = &h.band {
            ui.label(
                egui::RichText::new(format!("Valid {band}"))
                    .small()
                    .color(pal.text_secondary),
            );
        }
    });
    if !h.source.is_empty() {
        ui.label(
            egui::RichText::new(format!("Source: {}", h.source))
                .small()
                .color(pal.text_secondary),
        );
    }
    ui.label(&h.why);
    egui::Grid::new(("emc_numbers", h.id))
        .num_columns(2)
        .show(ui, |ui| {
            for (name, value) in &h.numbers {
                ui.label(egui::RichText::new(name).color(pal.text_secondary));
                ui.label(value);
                ui.end_row();
            }
        });
    for a in &h.assumptions {
        ui.label(
            egui::RichText::new(format!(
                "Assumed ({}): {}",
                if a.user_set { "set by you" } else { "default" },
                a.text
            ))
            .small()
            .color(pal.text_secondary),
        );
    }
    let mut involved: Vec<String> = h
        .runs
        .iter()
        .filter_map(|r| {
            pic.run(*r)
                .map(|r| format!("wire {} ({})", r.label, r.class.short()))
        })
        .collect();
    involved.extend(h.devices.iter().map(|d| tag_of(*d)));
    if !involved.is_empty() {
        ui.label(
            egui::RichText::new(format!("Involves: {}", involved.join(", ")))
                .small()
                .color(pal.text_secondary),
        );
    }
    if !h.fixes.is_empty() {
        ui.add_space(2.0);
        ui.label(egui::RichText::new("What would help").strong());
        if h.fixes.iter().any(|f| !f.applicable) {
            ui.label(
                egui::RichText::new(if pic.provenance == Provenance::Engine {
                    "These buttons do not change the project. Apply a fix from Check > EMC check."
                } else {
                    "Advice only for now: the buttons do not change the project yet."
                })
                .small()
                .color(pal.text_secondary),
            );
        }
        for f in &h.fixes {
            ui.horizontal_wrapped(|ui| {
                let r = ui.add_enabled(f.applicable, egui::Button::new(&f.label));
                r.on_hover_text(&f.detail).on_disabled_hover_text(&f.detail);
            });
        }
    }
}
