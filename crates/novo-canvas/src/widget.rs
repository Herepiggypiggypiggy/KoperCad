//! egui integration: a canvas widget that pans, zooms, hovers, selects and
//! snaps, and paints through wgpu inside the egui render pass.
//!
//! ```ignore
//! // Once, when the app starts (eframe::CreationContext):
//! novo_canvas::widget::install(cc.wgpu_render_state.as_ref().unwrap(), 1);
//! // Every frame:
//! let out = self.view.show(ui, &self.scene);
//! if let Some(c) = out.cursor { /* c.point is the snapped world cursor */ }
//! ```
//!
//! Default controls: wheel zooms at the cursor, middle drag (or trackpad
//! scroll) pans, right drag pans once it moves past 4 points (a right click
//! stays a click: see [`CanvasOutput::context_click`]), pinch or Ctrl+wheel
//! zooms, double middle click fits the scene. Left click selects (Shift
//! adds, Ctrl toggles), left drag on empty space box-selects (left to right:
//! window, right to left: crossing). Escape climbs one rung of the ladder in
//! [`CanvasView::escape_with`]: cancel the step, leave the tool, clear the
//! selection.

use crate::geom::Aabb;
use crate::grid::{Grid, GridStyle};
use crate::render::{self, CanvasRenderer, FrameData, GpuCanvas, KeyedMesh, TargetConfig};
use crate::scene::{BoxMode, ItemId, Scene};
use crate::selection::{SelectOp, Selection};
use crate::shape::{Primitive, Shape};
use crate::snap::{self, SnapKind, SnapSettings, Snapped};
use crate::style::{Color, Stroke};
use crate::tessellate::{self, Highlight, Mesh, SceneMesh, Tessellator};
use crate::viewport::Viewport;
use glam::Vec2;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Set once [`install_with`] has registered the pipeline.
static INSTALLED: AtomicBool = AtomicBool::new(false);

/// GPU buffers of a canvas not painted for this long are freed (a closed or
/// long hidden page tab). They are rebuilt if the canvas shows again.
const IDLE_CANVAS_TTL: Duration = Duration::from_secs(30);

/// Registers the canvas pipeline with egui's wgpu renderer. Call once at
/// start-up. `msaa_samples` must be the sample count eframe renders with,
/// i.e. `NativeOptions::multisampling` (`0` and `1` both mean off), or
/// pipeline creation fails validation.
pub fn install(render_state: &egui_wgpu::RenderState, msaa_samples: u32) {
    let target = TargetConfig {
        format: render_state.target_format,
        msaa_samples: msaa_samples.max(1),
        depth_format: None,
    };
    install_with(render_state, target);
}

/// Like [`install`] with full control over the target description.
pub fn install_with(render_state: &egui_wgpu::RenderState, target: TargetConfig) {
    let renderer = CanvasRenderer::new(&render_state.device, target);
    render_state
        .renderer
        .write()
        .callback_resources
        .insert(CanvasResources {
            renderer,
            canvases: HashMap::new(),
        });
    INSTALLED.store(true, Ordering::Relaxed);
}

/// Whether the canvas renderer is installed. Without it (no wgpu adapter,
/// or `install` never called) [`CanvasView::show`] paints an error notice
/// instead of the drawing.
pub fn is_installed() -> bool {
    INSTALLED.load(Ordering::Relaxed)
}

struct CanvasResources {
    renderer: CanvasRenderer,
    canvases: HashMap<u64, (GpuCanvas, Instant)>,
}

struct CanvasCallback {
    id: u64,
    frame: FrameData,
}

impl egui_wgpu::CallbackTrait for CanvasCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(res) = resources.get_mut::<CanvasResources>() {
            let CanvasResources { renderer, canvases } = res;
            let now = Instant::now();
            canvases.retain(|&id, (_, used)| id == self.id || now - *used < IDLE_CANVAS_TTL);
            let (canvas, used) = canvases
                .entry(self.id)
                .or_insert_with(|| (renderer.create_canvas(device), now));
            *used = now;
            canvas.prepare(device, queue, &self.frame);
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        if let Some(res) = resources.get::<CanvasResources>()
            && let Some((canvas, _)) = res.canvases.get(&self.id)
        {
            res.renderer.paint(pass, canvas);
        }
    }
}

/// Colours of everything the canvas draws itself.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasStyle {
    pub background: Color,
    pub grid_minor: Color,
    pub grid_major: Color,
    pub selection: Highlight,
    pub hover: Highlight,
    pub band_window: Color,
    pub band_crossing: Color,
    pub snap_marker: Color,
}

impl Default for CanvasStyle {
    fn default() -> Self {
        Self {
            background: Color::WHITE,
            grid_minor: Color::rgb(200, 205, 212),
            grid_major: Color::rgb(160, 168, 180),
            selection: Highlight::selection(),
            hover: Highlight::hover(),
            band_window: Color::rgb(40, 110, 230),
            band_crossing: Color::rgb(30, 160, 80),
            snap_marker: Color::rgb(230, 60, 160),
        }
    }
}

/// Which built-in interactions are on. Editors (wire drawing, moving
/// symbols) switch selection off while a tool is active and read the
/// returned [`CanvasOutput`] instead.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interaction {
    pub pan: bool,
    pub zoom: bool,
    pub select: bool,
    pub hover: bool,
    /// Show a marker where the cursor snaps.
    pub snap_marker: bool,
    /// Run [`CanvasView::escape`] when Escape is pressed while the pointer
    /// is over the canvas. Apps that route Escape themselves (whenever the
    /// drawing has keyboard focus) turn this off and call `escape` or
    /// `escape_with` directly, so one key press climbs one rung.
    pub escape_on_hover: bool,
}

impl Default for Interaction {
    fn default() -> Self {
        Self {
            pan: true,
            zoom: true,
            select: true,
            hover: true,
            snap_marker: true,
            escape_on_hover: true,
        }
    }
}

/// A right drag pans only after the pointer has moved this many points, so
/// a plain right click stays a click (for context menus).
pub const RIGHT_PAN_THRESHOLD: f32 = 4.0;

/// What one Escape press did, top rung first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EscapeOutcome {
    /// A step in progress was cancelled: a selection band, or the tool's
    /// current step (the wire being drawn, a move in progress).
    CancelledStep,
    /// The active tool was left, back to selection.
    LeftTool,
    /// The selection was cleared.
    ClearedSelection,
    /// Nothing to undo; the app may pass Escape on.
    Nothing,
}

/// The tool side of the Escape ladder. Editing tools (WS-06) implement it;
/// [`CanvasView::escape_with`] asks them before clearing the selection.
pub trait EscapeTarget {
    /// Cancels the step in progress. Returns false when there is none.
    fn cancel_step(&mut self) -> bool;
    /// Leaves the tool. Returns false when no tool is active.
    fn leave_tool(&mut self) -> bool;
}

#[derive(Clone, Copy, Debug)]
struct RightDrag {
    origin: egui::Pos2,
    panning: bool,
}

static NEXT_CANVAS_ID: AtomicU64 = AtomicU64::new(1);

/// State of one canvas on screen (keep one per open page).
pub struct CanvasView {
    id: u64,
    pub viewport: Viewport,
    pub grid: Grid,
    pub snap: SnapSettings,
    pub selection: Selection,
    pub style: CanvasStyle,
    pub interaction: Interaction,
    /// Pick tolerance in screen points.
    pub pick_tolerance: f32,
    /// Zoom factor per wheel notch.
    pub wheel_zoom_step: f32,
    /// Labels smaller than this many screen points are not drawn.
    pub min_label_size: f32,
    tessellator: Tessellator,
    mesh_cache: tessellate::ItemMeshCache,
    scene_mesh: Option<Arc<SceneMesh>>,
    highlight: Option<Arc<KeyedMesh>>,
    highlight_source: (u64, u64),
    hovered: Option<ItemId>,
    band: Option<Vec2>,
    right_drag: Option<RightDrag>,
    fit_pending: bool,
    stats: FrameStats,
}

/// Numbers for a status bar or a performance overlay.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameStats {
    pub items: usize,
    pub scene_vertices: usize,
    pub scene_triangles: usize,
    /// Milliseconds spent tessellating the scene, when it changed this frame.
    pub tessellate_ms: Option<f32>,
}

/// What happened on the canvas this frame.
pub struct CanvasOutput {
    pub response: egui::Response,
    /// Cursor in world units after snapping, when the pointer is over the canvas.
    pub cursor: Option<Snapped>,
    /// Cursor in world units without snapping.
    pub cursor_raw: Option<Vec2>,
    pub hovered: Option<ItemId>,
    pub selection_changed: bool,
    /// World position of a primary click, after snapping.
    pub clicked: Option<Snapped>,
    /// World position of a right click that did not turn into a pan (moved
    /// less than [`RIGHT_PAN_THRESHOLD`]), after snapping. Open context menus
    /// on this rather than `response.secondary_clicked()`, whose click
    /// distance differs from the pan threshold.
    pub context_click: Option<Snapped>,
    /// What a built-in Escape press did (see [`Interaction::escape_on_hover`]).
    pub escape: Option<EscapeOutcome>,
}

impl Default for CanvasView {
    fn default() -> Self {
        Self::new()
    }
}

impl CanvasView {
    pub fn new() -> Self {
        Self {
            id: NEXT_CANVAS_ID.fetch_add(1, Ordering::Relaxed),
            viewport: Viewport::default(),
            grid: Grid::default(),
            snap: SnapSettings::default(),
            selection: Selection::new(),
            style: CanvasStyle::default(),
            interaction: Interaction::default(),
            pick_tolerance: 5.0,
            wheel_zoom_step: 1.2,
            min_label_size: 4.0,
            tessellator: Tessellator::default(),
            mesh_cache: tessellate::ItemMeshCache::default(),
            scene_mesh: None,
            highlight: None,
            highlight_source: (0, u64::MAX),
            hovered: None,
            band: None,
            right_drag: None,
            fit_pending: true,
            stats: FrameStats::default(),
        }
    }

    /// One Escape press without a tool: cancels a selection band in
    /// progress, else clears the selection.
    pub fn escape(&mut self) -> EscapeOutcome {
        self.escape_with(None)
    }

    /// One Escape press: cancels the step in progress (a selection band,
    /// then the tool's step), else leaves the tool, else clears the
    /// selection. Call it whenever the drawing has keyboard focus.
    pub fn escape_with(&mut self, tool: Option<&mut dyn EscapeTarget>) -> EscapeOutcome {
        if self.band.take().is_some() {
            return EscapeOutcome::CancelledStep;
        }
        if let Some(tool) = tool {
            if tool.cancel_step() {
                return EscapeOutcome::CancelledStep;
            }
            if tool.leave_tool() {
                return EscapeOutcome::LeftTool;
            }
        }
        if !self.selection.is_empty() {
            self.selection.clear();
            return EscapeOutcome::ClearedSelection;
        }
        EscapeOutcome::Nothing
    }

    /// Fits the scene into view on the next frame.
    pub fn zoom_to_fit(&mut self) {
        self.fit_pending = true;
    }

    pub fn hovered(&self) -> Option<ItemId> {
        self.hovered
    }

    pub fn stats(&self) -> FrameStats {
        self.stats
    }

    /// Fills the available space with the canvas.
    pub fn show(&mut self, ui: &mut egui::Ui, scene: &Scene) -> CanvasOutput {
        let size = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        self.show_in_rect(ui, rect, response, scene)
    }

    fn show_in_rect(
        &mut self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        response: egui::Response,
        scene: &Scene,
    ) -> CanvasOutput {
        self.viewport.size = Vec2::new(rect.width(), rect.height()).max(Vec2::ONE);
        if self.fit_pending && rect.width() > 1.0 {
            self.viewport.fit(&scene.bounds(), 24.0);
            self.fit_pending = false;
        }
        let to_local = |p: egui::Pos2| Vec2::new(p.x - rect.min.x, p.y - rect.min.y);
        let hover_local = response.hover_pos().map(to_local);

        let context_click = self.handle_navigation(ui, &response, hover_local, scene);

        let tol = self.viewport.screen_to_world_len(self.pick_tolerance);
        let cursor_raw = hover_local.map(|p| self.viewport.screen_to_world(p));
        let cursor = cursor_raw
            .map(|w| snap::snap(scene, &self.viewport, Some(&self.grid), &self.snap, w, &[]));

        self.hovered = match cursor_raw {
            Some(w) if self.interaction.hover && self.band.is_none() => {
                scene.pick(w, tol, self.viewport.zoom).map(|h| h.id)
            }
            _ => None,
        };

        let mut selection_changed = false;
        let mut clicked = None;
        let (shift, command) = ui.ctx().input(|i| (i.modifiers.shift, i.modifiers.command));
        let op = SelectOp::from_modifiers(shift, command);

        if response.clicked_by(egui::PointerButton::Primary) {
            clicked = cursor;
            if self.interaction.select
                && let Some(w) = cursor_raw
            {
                let rev = self.selection.revision();
                match scene.pick(w, tol, self.viewport.zoom) {
                    Some(hit) => self.selection.apply([hit.id], op),
                    None if op == SelectOp::Replace => self.selection.clear(),
                    None => {}
                }
                selection_changed |= self.selection.revision() != rev;
            }
        }

        if self.interaction.select {
            if response.drag_started_by(egui::PointerButton::Primary) {
                let origin = ui.ctx().input(|i| i.pointer.press_origin()).map(to_local);
                if let Some(o) = origin {
                    let w = self.viewport.screen_to_world(o);
                    if scene.pick(w, tol, self.viewport.zoom).is_none() {
                        self.band = Some(w);
                    }
                }
            }
            if response.drag_stopped_by(egui::PointerButton::Primary)
                && let (Some(start), Some(end)) = (self.band.take(), cursor_raw)
            {
                let mode = BoxMode::from_drag(start, end);
                let ids =
                    scene.box_select(&Aabb::from_corners(start, end), mode, self.viewport.zoom);
                let rev = self.selection.revision();
                self.selection.apply(ids, op);
                selection_changed |= self.selection.revision() != rev;
            }
        } else {
            self.band = None;
        }
        if !ui.ctx().input(|i| i.pointer.primary_down()) && !response.dragged() {
            self.band = None;
        }
        let mut escape = None;
        if self.interaction.escape_on_hover
            && response.hovered()
            && ui.ctx().input(|i| i.key_pressed(egui::Key::Escape))
        {
            let rev = self.selection.revision();
            escape = Some(self.escape());
            selection_changed |= self.selection.revision() != rev;
        }

        let rev = self.selection.revision();
        self.selection.retain_existing(scene);
        selection_changed |= self.selection.revision() != rev;

        if is_installed() {
            let frame = self.build_frame(ui.ctx().pixels_per_point(), scene, cursor, cursor_raw);
            ui.painter().add(egui_wgpu::Callback::new_paint_callback(
                rect,
                CanvasCallback { id: self.id, frame },
            ));
            self.paint_labels(ui, rect, scene);
        } else {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "The drawing cannot be shown: no graphics adapter was found.\n\
                 NovoElectric needs Direct3D 12, Vulkan or Metal \
                 (on Linux, install the Mesa Vulkan drivers).",
                egui::FontId::proportional(15.0),
                ui.visuals().error_fg_color,
            );
        }

        CanvasOutput {
            response,
            cursor,
            cursor_raw,
            hovered: self.hovered,
            selection_changed,
            clicked,
            context_click: context_click.then_some(cursor).flatten(),
            escape,
        }
    }

    fn handle_navigation(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        hover: Option<Vec2>,
        scene: &Scene,
    ) -> bool {
        if self.interaction.pan && response.dragged_by(egui::PointerButton::Middle) {
            let d = response.drag_delta();
            self.viewport.pan_by_screen(Vec2::new(d.x, d.y));
        }
        let context_click = self.right_drag_pan(ui, response);
        let Some(anchor) = hover else {
            return context_click;
        };
        let (events, zoom_delta) = ui.ctx().input(|i| (i.events.clone(), i.zoom_delta()));
        if self.interaction.zoom && zoom_delta != 1.0 {
            self.viewport.zoom_at(anchor, zoom_delta);
        }
        for e in events {
            match e {
                egui::Event::MouseWheel {
                    unit,
                    delta,
                    modifiers,
                    ..
                } if !modifiers.command => match unit {
                    egui::MouseWheelUnit::Line | egui::MouseWheelUnit::Page => {
                        if self.interaction.zoom {
                            let steps = if unit == egui::MouseWheelUnit::Page {
                                delta.y * 3.0
                            } else {
                                delta.y
                            };
                            self.viewport
                                .zoom_at(anchor, self.wheel_zoom_step.powf(steps));
                        }
                    }
                    egui::MouseWheelUnit::Point => {
                        if self.interaction.pan {
                            self.viewport.pan_by_screen(Vec2::new(delta.x, delta.y));
                        }
                    }
                },
                egui::Event::PointerButton {
                    button: egui::PointerButton::Middle,
                    pressed: true,
                    ..
                } => {
                    // Double middle click fits, as in most CAD programs.
                    let now = ui.ctx().input(|i| i.time);
                    let last = ui
                        .ctx()
                        .data(|d| d.get_temp::<f64>(egui::Id::new(("novo_mmb", self.id))));
                    if last.is_some_and(|t| now - t < 0.35) {
                        self.viewport.fit(&scene.bounds(), 24.0);
                    }
                    ui.ctx()
                        .data_mut(|d| d.insert_temp(egui::Id::new(("novo_mmb", self.id)), now));
                }
                _ => {}
            }
        }
        context_click
    }

    /// Right drag pans once the pointer has moved past the threshold;
    /// returns true for a right click that never did.
    fn right_drag_pan(&mut self, ui: &egui::Ui, response: &egui::Response) -> bool {
        let (pressed, down, pos) = ui.ctx().input(|i| {
            (
                i.pointer.button_pressed(egui::PointerButton::Secondary),
                i.pointer.secondary_down(),
                i.pointer.latest_pos(),
            )
        });
        if pressed
            && response.contains_pointer()
            && let Some(origin) = pos
        {
            self.right_drag = Some(RightDrag {
                origin,
                panning: false,
            });
        }
        let Some(mut drag) = self.right_drag else {
            return false;
        };
        if !down {
            self.right_drag = None;
            // A click released off the canvas is not a click on it.
            return !drag.panning && response.contains_pointer();
        }
        if let Some(pos) = pos {
            if drag.panning {
                let d = ui.ctx().input(|i| i.pointer.delta());
                if self.interaction.pan {
                    self.viewport.pan_by_screen(Vec2::new(d.x, d.y));
                }
            } else if pos.distance(drag.origin) > RIGHT_PAN_THRESHOLD {
                drag.panning = true;
                // Catch up with the movement below the threshold.
                let d = pos - drag.origin;
                if self.interaction.pan {
                    self.viewport.pan_by_screen(Vec2::new(d.x, d.y));
                }
            }
        }
        self.right_drag = Some(drag);
        false
    }

    fn build_frame(
        &mut self,
        pixels_per_point: f32,
        scene: &Scene,
        cursor: Option<Snapped>,
        cursor_raw: Option<Vec2>,
    ) -> FrameData {
        let mut tess_ms = None;
        if self
            .scene_mesh
            .as_ref()
            .is_none_or(|m| m.revision != scene.revision())
        {
            let t = std::time::Instant::now();
            self.scene_mesh = Some(Arc::new(
                self.tessellator.scene_cached(scene, &mut self.mesh_cache),
            ));
            tess_ms = Some(t.elapsed().as_secs_f32() * 1000.0);
        }
        let sm = self.scene_mesh.clone().unwrap_or_default();
        self.stats = FrameStats {
            items: scene.len(),
            scene_vertices: sm.mesh.vertices.len(),
            scene_triangles: sm.mesh.indices.len() / 3,
            tessellate_ms: tess_ms.or(self.stats.tessellate_ms),
        };

        // Selection highlight, cached on (scene, selection) revisions.
        let source = (scene.revision(), self.selection.revision());
        if self.selection.is_empty() {
            self.highlight = None;
        } else if self.highlight.is_none() || self.highlight_source != source {
            let mut mesh = Mesh::default();
            for id in self.selection.ids() {
                if let Some(item) = scene.get(*id) {
                    self.tessellator
                        .item_highlighted(&mut mesh, item, &self.style.selection);
                }
            }
            self.highlight = Some(Arc::new(KeyedMesh {
                key: source.0.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ source.1,
                mesh,
            }));
        }
        self.highlight_source = source;

        let mut under = Mesh::default();
        render::background(&mut under, &self.viewport, self.style.background);
        self.grid_mesh(&mut under);

        let mut over = Mesh::default();
        if let Some(id) = self.hovered
            && !self.selection.contains(id)
            && let Some(item) = scene.get(id)
        {
            self.tessellator
                .item_highlighted(&mut over, item, &self.style.hover);
        }
        if let (Some(start), Some(end)) = (self.band, cursor_raw) {
            let color = match BoxMode::from_drag(start, end) {
                BoxMode::Window => self.style.band_window,
                BoxMode::Crossing => self.style.band_crossing,
            };
            let rect = Primitive {
                shape: Shape::rect(start, end),
                stroke: Some(Stroke::screen(1.0, color)),
                fill: Some(color.with_alpha(40)),
            };
            self.tessellator.primitive(&mut over, &rect);
        }
        if self.interaction.snap_marker
            && let Some(c) = cursor
            && c.kind != SnapKind::Free
        {
            self.snap_marker(&mut over, c);
        }

        FrameData {
            viewport: self.viewport,
            pixels_per_point,
            scene_ranges: sm.visible_ranges(scene),
            scene: Some(sm),
            highlight: self.highlight.clone(),
            under,
            over,
        }
    }

    /// Draws item labels with egui's text renderer, on top of the geometry.
    fn paint_labels(&self, ui: &egui::Ui, rect: egui::Rect, scene: &Scene) {
        // Above this size glyphs get expensive to rasterise and the label
        // fills the screen anyway.
        const MAX_SIZE: f32 = 512.0;
        const MAX_LABELS: usize = 20_000;
        let painter = ui.painter_at(rect);
        let zoom = self.viewport.zoom;
        let mut drawn = 0;
        for id in scene.query(&self.viewport.visible_world()) {
            let Some(item) = scene.get(id) else { continue };
            if item.labels.is_empty() || !scene.layer(item.layer).is_some_and(|l| l.visible) {
                continue;
            }
            let highlight = if self.selection.contains(id) {
                Some(self.style.selection.color)
            } else if self.hovered == Some(id) {
                Some(self.style.hover.color)
            } else {
                None
            };
            for l in &item.labels {
                let size = l.height * zoom;
                if size < self.min_label_size || size > MAX_SIZE || l.text.is_empty() {
                    continue;
                }
                // Quantise sizes (12 steps per octave) so continuous zooming
                // does not rasterise a new font size every frame.
                let size = 2f32.powf((size.log2() * 12.0).round() / 12.0);
                let [r, g, b, a] = highlight.unwrap_or(l.color).0;
                let color = egui::Color32::from_rgba_unmultiplied(r, g, b, a);
                let galley =
                    painter.layout_no_wrap(l.text.clone(), egui::FontId::proportional(size), color);
                let f = l.anchor.fractions();
                let offset = egui::vec2(f.x * galley.size().x, f.y * galley.size().y);
                let s = self.viewport.world_to_screen(l.pos);
                let anchor = rect.min + egui::vec2(s.x, s.y);
                let rot = egui::emath::Rot2::from_angle(l.rotation);
                painter.add(
                    egui::epaint::TextShape::new(anchor - rot * offset, galley, color)
                        .with_angle(l.rotation),
                );
                drawn += 1;
                if drawn >= MAX_LABELS {
                    return;
                }
            }
        }
    }

    fn grid_mesh(&self, mesh: &mut Mesh) {
        let Some(g) = self.grid.visible_lines(&self.viewport) else {
            return;
        };
        let vis = self.viewport.visible_world();
        match self.grid.style {
            GridStyle::Lines => {
                for &(x, major) in &g.xs {
                    let c = if major {
                        self.style.grid_major
                    } else {
                        self.style.grid_minor
                    };
                    tessellate::stroke_run(
                        mesh,
                        &[Vec2::new(x, vis.min.y), Vec2::new(x, vis.max.y)],
                        &Stroke::screen(1.0, c.with_alpha(110)),
                    );
                }
                for &(y, major) in &g.ys {
                    let c = if major {
                        self.style.grid_major
                    } else {
                        self.style.grid_minor
                    };
                    tessellate::stroke_run(
                        mesh,
                        &[Vec2::new(vis.min.x, y), Vec2::new(vis.max.x, y)],
                        &Stroke::screen(1.0, c.with_alpha(110)),
                    );
                }
            }
            GridStyle::Dots => {
                // A dot is a stroke one point long and 1.5 points wide.
                let half = self.viewport.screen_to_world_len(0.75);
                for &(y, my) in &g.ys {
                    for &(x, mx) in &g.xs {
                        let major = mx && my;
                        let c = if major {
                            self.style.grid_major
                        } else {
                            self.style.grid_minor
                        };
                        let w = if major { 2.0 } else { 1.5 };
                        tessellate::stroke_run(
                            mesh,
                            &[Vec2::new(x - half, y), Vec2::new(x + half, y)],
                            &Stroke::screen(w, c),
                        );
                    }
                }
            }
        }
    }

    fn snap_marker(&self, mesh: &mut Mesh, c: Snapped) {
        let r = self.viewport.screen_to_world_len(5.0);
        let p = c.point;
        let stroke = Stroke::screen(1.5, self.style.snap_marker);
        let shape = match c.kind {
            SnapKind::Connection => Shape::circle(p, r),
            SnapKind::Endpoint => Shape::rect(p - Vec2::splat(r), p + Vec2::splat(r)),
            SnapKind::Center => Shape::circle(p, r * 0.7),
            SnapKind::Midpoint => Shape::polyline(
                [
                    p + Vec2::new(0.0, -r),
                    p + Vec2::new(r, r * 0.8),
                    p + Vec2::new(-r, r * 0.8),
                ],
                true,
            ),
            SnapKind::Grid => {
                let h = r * 0.6;
                tessellate::stroke_run(mesh, &[p - Vec2::X * h, p + Vec2::X * h], &stroke);
                tessellate::stroke_run(mesh, &[p - Vec2::Y * h, p + Vec2::Y * h], &stroke);
                return;
            }
            SnapKind::Free => return,
        };
        self.tessellator
            .primitive(mesh, &Primitive::stroked(shape, stroke));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Item;
    use crate::shape::Shape;

    fn scene() -> Scene {
        let mut s = Scene::new();
        s.insert(
            ItemId(1),
            Item::new(crate::LayerId(0)).with(Primitive::stroked(
                Shape::line(Vec2::ZERO, Vec2::new(100.0, 0.0)),
                Stroke::default(),
            )),
        );
        s
    }

    /// Runs one frame of a canvas filling an 800x600 window.
    fn frame(
        ctx: &egui::Context,
        view: &mut CanvasView,
        scene: &Scene,
        events: Vec<egui::Event>,
    ) -> (bool, Option<EscapeOutcome>) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            events,
            ..Default::default()
        };
        let mut out = (false, None);
        let mut output = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| {
                    let o = view.show(ui, scene);
                    out = (o.context_click.is_some(), o.escape);
                });
        });
        output.textures_delta.clear();
        out
    }

    fn moved(x: f32, y: f32) -> egui::Event {
        egui::Event::PointerMoved(egui::pos2(x, y))
    }

    fn right(x: f32, y: f32, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos: egui::pos2(x, y),
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: Default::default(),
        }
    }

    fn setup() -> (egui::Context, CanvasView, Scene) {
        let ctx = egui::Context::default();
        let mut view = CanvasView::new();
        let scene = scene();
        frame(&ctx, &mut view, &scene, vec![moved(400.0, 300.0)]);
        (ctx, view, scene)
    }

    #[test]
    fn right_click_without_drag_is_a_context_click() {
        let (ctx, mut view, scene) = setup();
        let center = view.viewport.center;
        frame(&ctx, &mut view, &scene, vec![right(400.0, 300.0, true)]);
        frame(&ctx, &mut view, &scene, vec![moved(402.0, 301.0)]);
        let (context, _) = frame(&ctx, &mut view, &scene, vec![right(402.0, 301.0, false)]);
        assert!(context);
        assert_eq!(view.viewport.center, center);
    }

    #[test]
    fn right_drag_past_threshold_pans() {
        let (ctx, mut view, scene) = setup();
        let center = view.viewport.center;
        frame(&ctx, &mut view, &scene, vec![right(400.0, 300.0, true)]);
        frame(&ctx, &mut view, &scene, vec![moved(430.0, 300.0)]);
        frame(&ctx, &mut view, &scene, vec![moved(450.0, 300.0)]);
        let (context, _) = frame(&ctx, &mut view, &scene, vec![right(450.0, 300.0, false)]);
        assert!(!context);
        // The page followed the pointer by the full 50 points.
        let moved_world = (center - view.viewport.center).x * view.viewport.zoom;
        assert!((moved_world - 50.0).abs() < 1e-3, "{moved_world}");
    }

    #[test]
    fn escape_ladder() {
        struct Tool {
            step: bool,
            active: bool,
        }
        impl EscapeTarget for Tool {
            fn cancel_step(&mut self) -> bool {
                std::mem::take(&mut self.step)
            }
            fn leave_tool(&mut self) -> bool {
                std::mem::take(&mut self.active)
            }
        }
        let mut view = CanvasView::new();
        view.selection.apply([ItemId(1)], SelectOp::Replace);
        let mut tool = Tool {
            step: true,
            active: true,
        };
        let mut press = |view: &mut CanvasView| view.escape_with(Some(&mut tool));
        assert_eq!(press(&mut view), EscapeOutcome::CancelledStep);
        assert_eq!(press(&mut view), EscapeOutcome::LeftTool);
        assert_eq!(press(&mut view), EscapeOutcome::ClearedSelection);
        assert_eq!(press(&mut view), EscapeOutcome::Nothing);
        assert!(view.selection.is_empty());
    }

    #[test]
    fn escape_on_hover_climbs_one_rung() {
        let (ctx, mut view, scene) = setup();
        view.selection.apply([ItemId(1)], SelectOp::Replace);
        let key = egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        };
        let (_, esc) = frame(&ctx, &mut view, &scene, vec![key.clone()]);
        assert_eq!(esc, Some(EscapeOutcome::ClearedSelection));
        view.interaction.escape_on_hover = false;
        view.selection.apply([ItemId(1)], SelectOp::Replace);
        let (_, esc) = frame(&ctx, &mut view, &scene, vec![key]);
        assert_eq!(esc, None);
        assert_eq!(view.selection.len(), 1);
    }
}
