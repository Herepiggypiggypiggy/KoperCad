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
use crate::marks::{self, MarkKind, Marks, Placed};
use crate::render::{self, CanvasRenderer, FrameData, GpuCanvas, KeyedMesh, TargetConfig};
use crate::scene::{BoxMode, Hit, ItemId, Scene};
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
    /// Items related to the selection (what a coil switches, a net).
    pub related: Highlight,
    pub band_window: Color,
    pub band_crossing: Color,
    pub snap_marker: Color,
    /// Mark colours (docs/DESIGN.md §5). Copper is never a mark colour.
    /// The defaults read on the canvas's default white sheet; the app's dark
    /// theme maps its own (agreed with the layout thread): proposal INFO at
    /// 55%, override #989691, added #7FB086, modified #D6A55C and removed
    /// #E0766C at 50%, mark text #C4C2BD.
    pub proposal: Color,
    pub warning: Color,
    pub error: Color,
    pub override_mark: Color,
    pub added: Color,
    pub modified: Color,
    pub removed: Color,
    /// Text of mark hints and hover notes.
    pub mark_text: Color,
}

impl Default for CanvasStyle {
    fn default() -> Self {
        Self {
            background: Color::WHITE,
            grid_minor: Color::rgb(200, 205, 212),
            grid_major: Color::rgb(160, 168, 180),
            selection: Highlight::selection(),
            hover: Highlight::hover(),
            related: Highlight {
                color: Color::rgb(0, 170, 160),
                extra_width: 2.5,
                fill_alpha: 40,
            },
            band_window: Color::rgb(40, 110, 230),
            band_crossing: Color::rgb(30, 160, 80),
            snap_marker: Color::rgb(230, 60, 160),
            proposal: Color::rgba(70, 110, 160, 140),
            warning: Color::rgb(214, 150, 40),
            error: Color::rgb(200, 50, 45),
            override_mark: Color::rgb(130, 130, 135),
            added: Color::rgb(40, 150, 70),
            modified: Color::rgb(214, 150, 40),
            removed: Color::rgba(200, 60, 60, 150),
            mark_text: Color::rgb(40, 40, 45),
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
    /// Text on screen is never smaller than this many points: smaller
    /// labels are enlarged to it, or hidden by priority when that does not
    /// fit (see [`crate::declutter`]). 0 turns the floor off.
    pub min_text_px: f32,
    /// Screen points per world unit that [`Self::reveal`] zooms in to at
    /// least: 4 shows 2.5 mm text at 10 points.
    pub reveal_zoom: f32,
    /// Screen points a revealed target keeps from the canvas edges.
    pub reveal_margin: f32,
    /// A font family with a bold face, for [`Label::bold`](crate::Label)
    /// text. When it is `None` or not registered with egui, bold is faked
    /// with a second pass shifted along the baseline.
    pub bold_family: Option<egui::FontFamily>,
    /// At most this many labels are drawn per frame, the largest first;
    /// text costs CPU time per glyph, and a page dense enough to exceed it
    /// is too small to read anyway.
    pub label_budget: usize,
    /// Keep the drawing on whole device pixels ([`Viewport::snap_to_pixels`]).
    pub pixel_snap: bool,
    /// Proposals, problems, overrides and changes drawn over the drawing.
    pub marks: Marks,
    tessellator: Tessellator,
    mesh_cache: tessellate::ItemMeshCache,
    scene_mesh: Option<Arc<SceneMesh>>,
    highlight: Option<Arc<KeyedMesh>>,
    highlight_source: (u64, u64, u64),
    related: Vec<ItemId>,
    related_revision: u64,
    hovered: Option<ItemId>,
    hovered_hit: Option<Hit>,
    band: Option<Vec2>,
    /// What the box being dragged would select now (see `CanvasOutput`).
    band_ids: Vec<ItemId>,
    right_drag: Option<RightDrag>,
    fit_pending: Option<Fit>,
    /// Screen points at the bottom of the canvas that something floats
    /// over (the tool dock); a fit leaves them clear.
    pub fit_reserve_bottom: f32,
    /// The last fit, until the person moves the view or a few frames pass:
    /// panels that open or resize after the first frame change the canvas
    /// size, and the fit is redone for the new size.
    fit_settle: Option<FitSettle>,
    /// What to bring into view; see [`Self::reveal`].
    reveal_pending: Option<PendingReveal>,
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
    /// The pick under the cursor, including which label when text is nearest.
    pub hit: Option<Hit>,
    pub selection_changed: bool,
    /// World position of a primary click, after snapping.
    pub clicked: Option<Snapped>,
    /// What a primary click hit, with the label when it landed on text, so
    /// the editor can select just a component's text.
    pub clicked_hit: Option<Hit>,
    /// World position of a primary double click, after snapping: on empty
    /// canvas it starts a wire.
    pub double_clicked: Option<Snapped>,
    /// World position of a right click that did not turn into a pan (moved
    /// less than [`RIGHT_PAN_THRESHOLD`]), after snapping. Open context menus
    /// on this rather than `response.secondary_clicked()`, whose click
    /// distance differs from the pan threshold.
    pub context_click: Option<Snapped>,
    /// What a built-in Escape press did (see [`Interaction::escape_on_hover`]).
    pub escape: Option<EscapeOutcome>,
    /// Marks (indices into `CanvasView::marks.list`) under the cursor's
    /// glyph; their text is shown as a hover note.
    pub hovered_marks: Vec<usize>,
    /// While a selection box is dragged: how many items it would select if
    /// released now, for an "N selected" hint.
    pub band_count: Option<usize>,
}

/// `a` to `b` cut into dashes of length `dash` with `gap` between them.
fn dashes(a: Vec2, b: Vec2, dash: f32, gap: f32) -> Vec<[Vec2; 2]> {
    let len = a.distance(b);
    if len <= 0.0 || dash <= 0.0 {
        return Vec::new();
    }
    let dir = (b - a) / len;
    let mut out = Vec::new();
    let mut t = 0.0;
    while t < len {
        let e = (t + dash).min(len);
        out.push([a + dir * t, a + dir * e]);
        t = e + gap.max(f32::EPSILON);
    }
    out
}

/// Most items a selection box lights up while it is dragged.
const BAND_HIGHLIGHT_MAX: usize = 2000;

/// Frames a [`CanvasView::reveal`] waits for its target to appear in the
/// scene before it is dropped.
pub const REVEAL_WAIT_FRAMES: u8 = 30;

/// Screen points kept around the drawing when fitting to it.
const FIT_CONTENT_MARGIN: f32 = 64.0;
/// The most a fit to content zooms in, in screen points per mm: 2.5 mm
/// text is then 15 points.
/// How much closer than the whole sheet a content fit may be before the sheet is shown instead.
const FIT_SHEET_WITHIN: f32 = 1.5;
/// Within this much closer than the whole sheet, the sheet is shown whatever
/// the drawing's share of it.
const FIT_SHEET_ALWAYS: f32 = 2.0;
/// Below this share of the sheet's area the drawing is fitted on its own.
const FIT_SHEET_COVERS: f32 = 0.25;
const FIT_CONTENT_MAX_ZOOM: f32 = 6.0;

/// How many frames after a fit the view follows the canvas size.
const FIT_SETTLE_FRAMES: u32 = 120;

#[derive(Clone, Copy, Debug)]
struct FitSettle {
    kind: Fit,
    size: Vec2,
    /// The bottom reserve the fit was made with: the dock reports its height
    /// after the canvas is drawn, so the first fit often used a stale one.
    reserve: f32,
    /// The view the fit left; a different one means something else (a
    /// reveal, a jump) moved it and the fit must not be redone over it.
    center: Vec2,
    zoom: f32,
    frames: u32,
}

impl FitSettle {
    fn of(kind: Fit, viewport: &Viewport, reserve: f32) -> FitSettle {
        FitSettle {
            kind,
            size: viewport.size,
            reserve,
            center: viewport.center,
            zoom: viewport.zoom,
            frames: FIT_SETTLE_FRAMES,
        }
    }
}

/// What a pending fit brings into view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fit {
    /// The drawing (on a page), else the whole scene.
    Content,
    /// The whole sheet.
    Sheet,
}

/// A reveal waiting for a frame whose scene holds its target.
struct PendingReveal {
    ids: Vec<ItemId>,
    area: Aabb,
    /// Frames left before it is dropped.
    frames: u8,
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
            min_text_px: crate::declutter::MIN_TEXT_PX,
            label_budget: 3000,
            pixel_snap: true,
            marks: Marks::default(),
            tessellator: Tessellator::default(),
            mesh_cache: tessellate::ItemMeshCache::default(),
            scene_mesh: None,
            highlight: None,
            highlight_source: (0, u64::MAX, 0),
            related: Vec::new(),
            related_revision: 0,
            hovered_hit: None,
            hovered: None,
            band: None,
            band_ids: Vec::new(),
            right_drag: None,
            fit_pending: Some(Fit::Content),
            fit_reserve_bottom: 0.0,
            fit_settle: None,
            reveal_pending: None,
            reveal_zoom: 4.0,
            reveal_margin: 48.0,
            bold_family: None,
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

    /// Fits what was drawn into view on the next frame, with a margin (on a
    /// page: the drawing, not the empty sheet around it; elsewhere the whole
    /// scene).
    pub fn zoom_to_fit(&mut self) {
        self.fit_pending = Some(Fit::Content);
    }

    /// Fits the whole sheet into view on the next frame.
    pub fn zoom_to_sheet(&mut self) {
        self.fit_pending = Some(Fit::Sheet);
    }

    /// Brings `ids` into view for a jump, on the first frame whose scene
    /// holds any of them (a jump to another page waits for its scene,
    /// up to [`REVEAL_WAIT_FRAMES`]): the view stays put when they are
    /// already [`Self::reveal_margin`] inside it, else centres on them,
    /// zooming in to [`Self::reveal_zoom`] or out until they fit (see
    /// [`Viewport::reveal`]).
    pub fn reveal(&mut self, ids: impl IntoIterator<Item = ItemId>) {
        self.reveal_pending = Some(PendingReveal {
            ids: ids.into_iter().collect(),
            area: Aabb::EMPTY,
            frames: REVEAL_WAIT_FRAMES,
        });
    }

    /// Like [`Self::reveal`], for a world rectangle rather than items (a
    /// proposal, a problem's location).
    pub fn reveal_area(&mut self, area: Aabb) {
        self.reveal_pending = Some(PendingReveal {
            ids: Vec::new(),
            area,
            frames: REVEAL_WAIT_FRAMES,
        });
    }

    pub fn hovered(&self) -> Option<ItemId> {
        self.hovered
    }

    /// Highlights items related to the selection, such as the contacts a
    /// selected coil switches. The app fills this from a model query each
    /// time the selection changes; it is drawn under the selection.
    pub fn set_related(&mut self, ids: impl IntoIterator<Item = ItemId>) {
        let ids: Vec<ItemId> = ids.into_iter().collect();
        if ids != self.related {
            self.related = ids;
            self.related_revision += 1;
        }
    }

    pub fn related(&self) -> &[ItemId] {
        &self.related
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

    /// Brings the scene into view: the whole of it, or on a page the
    /// drawing alone (never closer than [`FIT_CONTENT_MAX_ZOOM`], so a lone
    /// symbol is not blown up to fill the screen).
    fn fit_scene(&mut self, scene: &Scene, kind: Fit) {
        let reserve = self
            .fit_reserve_bottom
            .clamp(0.0, self.viewport.size.y * 0.3);
        self.viewport.size.y -= reserve;
        self.fit_clear(scene, kind);
        self.viewport.size.y += reserve;
        // The fit centred in the part above the reserve: the canvas centre
        // sits half the reserve lower.
        self.viewport.center.y += reserve * 0.5 / self.viewport.zoom;
    }

    /// A close fit on the drawing must not leave a stretch of empty canvas
    /// beside the sheet while the sheet's far columns are cut off: on an
    /// axis where the whole sheet fits, the sheet is centred; where it does
    /// not, the view stays inside the sheet.
    fn keep_sheet_in_view(&mut self, sheet: &Aabb) {
        let zoom = self.viewport.zoom;
        let margin = 24.0 / zoom;
        let half = self.viewport.size / zoom * 0.5;
        let (lo, hi) = (
            sheet.min - Vec2::splat(margin),
            sheet.max + Vec2::splat(margin),
        );
        let pick = |c: f32, half: f32, lo: f32, hi: f32| {
            if hi - lo <= 2.0 * half {
                (lo + hi) * 0.5
            } else {
                c.clamp(lo + half, hi - half)
            }
        };
        self.viewport.center = Vec2::new(
            pick(self.viewport.center.x, half.x, lo.x, hi.x),
            pick(self.viewport.center.y, half.y, lo.y, hi.y),
        );
    }

    fn fit_clear(&mut self, scene: &Scene, kind: Fit) {
        let content = scene.content_bounds();
        if kind == Fit::Content && scene.sheet().is_some() && !content.is_empty() {
            self.viewport.fit(&content, FIT_CONTENT_MARGIN);
            let on_content = (self.viewport.center, self.viewport.zoom);
            self.viewport.fit(&scene.bounds(), 24.0);
            // A drawing that fills most of the sheet (under 1.5 times
            // closer than the whole sheet) shows the sheet, centred, rather
            // than a corner of it with the rest cut off.
            let sheet = scene.bounds().size();
            let covers = content.size().x * content.size().y / (sheet.x * sheet.y).max(1.0);
            // A narrow column of drawing on a big sheet is not "most of the
            // sheet", whatever its zoom ratio: it is shown close.
            // Within [`FIT_SHEET_ALWAYS`] the sheet is shown even then: a
            // fit that cuts the sheet's foot hides the title block and the
            // revision row for a drawing at most twice as large.
            let closer = on_content.1 / self.viewport.zoom.max(f32::EPSILON);
            if closer > FIT_SHEET_ALWAYS && (closer > FIT_SHEET_WITHIN || covers < FIT_SHEET_COVERS)
            {
                (self.viewport.center, self.viewport.zoom) = on_content;
                if self.viewport.zoom > FIT_CONTENT_MAX_ZOOM {
                    self.viewport.zoom = FIT_CONTENT_MAX_ZOOM;
                    self.viewport.center = content.center();
                }
                self.keep_sheet_in_view(&scene.bounds());
            }
        } else {
            self.viewport.fit(&scene.bounds(), 24.0);
        }
    }

    /// Redoes the last fit when the canvas changed size since, as long as
    /// the person has not moved the view (`moved`) and only for a while.
    fn settle_fit(&mut self, scene: &Scene, moved: bool) {
        let Some(mut settle) = self.fit_settle.take() else {
            return;
        };
        // Within a pixel is still the fit: the view is snapped to whole
        // pixels while drawing, which nudges the centre by a fraction.
        let nudge = (self.viewport.center - settle.center).abs().max_element() * settle.zoom;
        let displaced = nudge > 1.0 || self.viewport.zoom != settle.zoom;
        if moved || displaced || settle.frames == 0 || self.viewport.size.x <= 1.0 {
            return;
        }
        settle.frames -= 1;
        if (self.viewport.size - settle.size).abs().max_element() > 0.5
            || (self.fit_reserve_bottom - settle.reserve).abs() > 0.5
        {
            self.fit_scene(scene, settle.kind);
            settle = FitSettle {
                frames: settle.frames,
                ..FitSettle::of(settle.kind, &self.viewport, self.fit_reserve_bottom)
            };
        }
        self.fit_settle = Some(settle);
    }

    fn show_in_rect(
        &mut self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        response: egui::Response,
        scene: &Scene,
    ) -> CanvasOutput {
        self.viewport.size = Vec2::new(rect.width(), rect.height()).max(Vec2::ONE);
        if rect.width() > 1.0
            && let Some(kind) = self.fit_pending.take()
        {
            self.fit_scene(scene, kind);
            self.fit_settle = Some(FitSettle::of(kind, &self.viewport, self.fit_reserve_bottom));
        } else {
            let moved = response.dragged()
                || response.clicked()
                || ui.input(|i| i.smooth_scroll_delta != egui::Vec2::ZERO || i.zoom_delta() != 1.0);
            self.settle_fit(scene, moved);
        }
        if rect.width() > 1.0
            && let Some(mut pending) = self.reveal_pending.take()
        {
            let bounds = pending
                .ids
                .iter()
                .filter_map(|id| scene.item_bounds(*id))
                .fold(pending.area, |b, i| b.union(&i));
            if !bounds.is_empty() {
                self.viewport
                    .reveal(&bounds, self.reveal_margin, self.reveal_zoom);
            } else if pending.frames > 1 {
                pending.frames -= 1;
                self.reveal_pending = Some(pending);
            }
        }
        let to_local = |p: egui::Pos2| Vec2::new(p.x - rect.min.x, p.y - rect.min.y);
        let hover_local = response.hover_pos().map(to_local);
        let context_click = self.handle_navigation(ui, &response, hover_local, scene);

        let tol = self.viewport.screen_to_world_len(self.pick_tolerance);
        let cursor_raw = hover_local.map(|p| self.viewport.screen_to_world(p));
        let cursor = cursor_raw
            .map(|w| snap::snap(scene, &self.viewport, Some(&self.grid), &self.snap, w, &[]));

        let hit = match cursor_raw {
            Some(w) if self.band.is_none() => scene.pick(w, tol, self.viewport.zoom),
            _ => None,
        };
        self.hovered_hit = hit.filter(|_| self.interaction.hover);
        self.hovered = self.hovered_hit.map(|h| h.id);

        let mut selection_changed = false;
        let mut clicked = None;
        let mut clicked_hit = None;
        let mut double_clicked = None;
        let (shift, command) = ui.ctx().input(|i| (i.modifiers.shift, i.modifiers.command));
        let op = SelectOp::from_modifiers(shift, command);

        if response.double_clicked_by(egui::PointerButton::Primary) {
            double_clicked = cursor;
        }
        if response.clicked_by(egui::PointerButton::Primary) {
            clicked = cursor;
            clicked_hit = cursor_raw.and_then(|w| scene.pick(w, tol, self.viewport.zoom));
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
        // What the box would select now, for the live highlight.
        self.band_ids = match (self.band, cursor_raw) {
            (Some(start), Some(end)) => scene.box_select(
                &Aabb::from_corners(start, end),
                BoxMode::from_drag(start, end),
                self.viewport.zoom,
            ),
            _ => Vec::new(),
        };
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

        if self.pixel_snap {
            self.viewport.snap_to_pixels(ui.ctx().pixels_per_point());
        }
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
                 KoperCAD needs Direct3D 12, Vulkan or Metal \
                 (on Linux, install the Mesa Vulkan drivers).",
                egui::FontId::proportional(15.0),
                ui.visuals().error_fg_color,
            );
        }
        let hovered_marks = self.paint_marks(ui, rect, scene, hover_local);

        CanvasOutput {
            response,
            cursor,
            cursor_raw,
            hovered: self.hovered,
            hit,
            selection_changed,
            clicked,
            clicked_hit,
            double_clicked,
            context_click: context_click.then_some(cursor).flatten(),
            escape,
            hovered_marks,
            band_count: self.band.map(|_| self.band_ids.len()),
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
                        self.fit_scene(scene, Fit::Content);
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

        // Selection and related highlights, cached on their revisions.
        let source = (
            scene.revision(),
            self.selection.revision(),
            self.related_revision,
        );
        if self.selection.is_empty() && self.related.is_empty() {
            self.highlight = None;
        } else if self.highlight.is_none() || self.highlight_source != source {
            let mut mesh = Mesh::default();
            for id in &self.related {
                if !self.selection.contains(*id)
                    && let Some(item) = scene.get(*id)
                {
                    self.tessellator
                        .item_highlighted(&mut mesh, item, &self.style.related);
                }
            }
            for id in self.selection.ids() {
                if let Some(item) = scene.get(*id) {
                    self.tessellator
                        .item_highlighted(&mut mesh, item, &self.style.selection);
                }
            }
            self.highlight = Some(Arc::new(KeyedMesh {
                key: (source.0.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ source.1)
                    .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                    ^ source.2,
                mesh,
            }));
        }
        self.highlight_source = source;

        let mut under = Mesh::default();
        render::background(&mut under, &self.viewport, self.style.background);
        self.grid_mesh(&mut under, scene, pixels_per_point);

        let mut over = Mesh::default();
        if let Some(hit) = self.hovered_hit
            && let Some(item) = scene.get(hit.id)
        {
            match hit.label.and_then(|i| item.labels.get(i)) {
                // Hovering text outlines just that text, so a tag reads as
                // its own object.
                Some(label) => {
                    let outline = Primitive::stroked(
                        Shape::polyline(label.corners().to_vec(), true),
                        Stroke::screen(1.5, self.style.hover.color),
                    );
                    self.tessellator.primitive(&mut over, &outline);
                }
                None if !self.selection.contains(hit.id) => {
                    self.tessellator
                        .item_highlighted(&mut over, item, &self.style.hover);
                }
                None => {}
            }
        }
        // Handles keep their size on screen, so they are tessellated for
        // this zoom every frame (only those of items in view).
        for id in scene.query(&self.viewport.visible_world()) {
            let Some(item) = scene.get(id) else { continue };
            for h in &item.handles {
                let r = self.viewport.screen_to_world_len(h.half);
                if h.ring {
                    let ring =
                        Primitive::stroked(Shape::circle(h.at, r), Stroke::screen(1.2, h.color));
                    self.tessellator.primitive(&mut over, &ring);
                    continue;
                }
                let shape = if h.round {
                    Shape::circle(h.at, r)
                } else {
                    Shape::rect(h.at - Vec2::splat(r), h.at + Vec2::splat(r))
                };
                self.tessellator
                    .primitive(&mut over, &Primitive::filled(shape, h.color));
            }
        }
        if let (Some(start), Some(end)) = (self.band, cursor_raw) {
            let color = match BoxMode::from_drag(start, end) {
                BoxMode::Window => self.style.band_window,
                BoxMode::Crossing => self.style.band_crossing,
            };
            // Items the box would select light up as they do under the
            // hover, up to a cap that keeps the frame cheap.
            if self.band_ids.len() <= BAND_HIGHLIGHT_MAX {
                for id in &self.band_ids {
                    if let Some(item) = scene.get(*id) {
                        self.tessellator
                            .item_highlighted(&mut over, item, &self.style.hover);
                    }
                }
            }
            let fill = Primitive {
                shape: Shape::rect(start, end),
                stroke: None,
                fill: Some(color.with_alpha(40)),
            };
            self.tessellator.primitive(&mut over, &fill);
            let (lo, hi) = (start.min(end), start.max(end));
            let corners = [lo, Vec2::new(hi.x, lo.y), hi, Vec2::new(lo.x, hi.y)];
            let edge = Stroke::screen(1.0, color);
            for i in 0..4 {
                let (a, b) = (corners[i], corners[(i + 1) % 4]);
                match BoxMode::from_drag(start, end) {
                    // Enclosing box: a solid outline.
                    BoxMode::Window => {
                        tessellate::stroke_run(&mut over, &[a, b], &edge);
                    }
                    // Crossing box: dashed.
                    BoxMode::Crossing => {
                        let dash = self.viewport.screen_to_world_len(6.0);
                        let gap = self.viewport.screen_to_world_len(4.0);
                        for [p, q] in dashes(a, b, dash, gap) {
                            tessellate::stroke_run(&mut over, &[p, q], &edge);
                        }
                    }
                }
            }
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
    /// Paints the marks over the drawing and its text, and returns the marks
    /// under the cursor's glyph (whose note is shown beside it).
    fn paint_marks(
        &self,
        ui: &egui::Ui,
        rect: egui::Rect,
        scene: &Scene,
        cursor: Option<Vec2>,
    ) -> Vec<usize> {
        let placed = marks::layout(&self.marks, scene, &self.viewport);
        if placed.is_empty() {
            return Vec::new();
        }
        let painter = ui.painter_at(rect);
        let c32 = |c: Color| {
            let [r, g, b, a] = c.0;
            egui::Color32::from_rgba_unmultiplied(r, g, b, a)
        };
        let at = |v: Vec2| rect.min + egui::vec2(v.x, v.y);
        let st = &self.style;
        let font = egui::FontId::proportional(11.0);
        for p in &placed {
            match p {
                Placed::Outline { kind, rect: r, .. } => {
                    let color = c32(match kind {
                        MarkKind::Proposal => st.proposal,
                        MarkKind::Added => st.added,
                        MarkKind::Modified => st.modified,
                        _ => st.removed,
                    });
                    let corners = [
                        at(r.min),
                        at(Vec2::new(r.max.x, r.min.y)),
                        at(r.max),
                        at(Vec2::new(r.min.x, r.max.y)),
                        at(r.min),
                    ];
                    let stroke = egui::Stroke::new(1.5, color);
                    if kind.is_dashed() {
                        painter.extend(egui::Shape::dashed_line(&corners, stroke, 5.0, 3.0));
                    } else {
                        painter.add(egui::Shape::line(corners.to_vec(), stroke));
                    }
                }
                Placed::Glyph { kind, pos, marks } => {
                    let c = at(*pos);
                    let h = marks::GLYPH_SIZE * 0.5;
                    match kind {
                        MarkKind::Warning => {
                            painter.add(egui::Shape::convex_polygon(
                                vec![
                                    c + egui::vec2(0.0, -h),
                                    c + egui::vec2(h, h * 0.8),
                                    c + egui::vec2(-h, h * 0.8),
                                ],
                                c32(st.warning),
                                egui::Stroke::NONE,
                            ));
                        }
                        MarkKind::Error => {
                            let pts = (0..8)
                                .map(|i| {
                                    let a = std::f32::consts::TAU * (i as f32 + 0.5) / 8.0;
                                    c + egui::vec2(a.cos(), a.sin()) * h
                                })
                                .collect();
                            painter.add(egui::Shape::convex_polygon(
                                pts,
                                c32(st.error),
                                egui::Stroke::NONE,
                            ));
                        }
                        _ => {
                            let pts = vec![
                                c + egui::vec2(0.0, -h),
                                c + egui::vec2(h, 0.0),
                                c + egui::vec2(0.0, h),
                                c + egui::vec2(-h, 0.0),
                            ];
                            painter.add(egui::Shape::closed_line(
                                pts,
                                egui::Stroke::new(1.5, c32(st.override_mark)),
                            ));
                        }
                    }
                    if marks.len() > 1 {
                        painter.text(
                            c + egui::vec2(h + 2.0, -h),
                            egui::Align2::LEFT_TOP,
                            marks.len().to_string(),
                            font.clone(),
                            c32(st.mark_text),
                        );
                    }
                }
            }
        }

        let note = |pos: egui::Pos2, text: &str| {
            let galley = painter.layout_no_wrap(text.to_owned(), font.clone(), c32(st.mark_text));
            let r = egui::Rect::from_min_size(pos, galley.size()).expand(3.0);
            painter.rect_filled(r, 2.0, c32(st.background).gamma_multiply(0.92));
            painter.galley(pos, galley, c32(st.mark_text));
        };
        // The hint goes on the nearest proposal only.
        if let Some(Placed::Outline { rect: r, mark, .. }) =
            marks::nearest_proposal(&placed, cursor)
            && let Some(m) = self.marks.list.get(*mark)
            && !m.text.is_empty()
        {
            note(at(Vec2::new(r.min.x, r.max.y + 4.0)), &m.text);
        }
        let Some(Placed::Glyph { pos, marks, .. }) =
            cursor.and_then(|c| marks::glyph_at(&placed, c))
        else {
            return Vec::new();
        };
        let text: Vec<&str> = marks
            .iter()
            .filter_map(|&i| self.marks.list.get(i))
            .map(|m| m.text.as_str())
            .filter(|t| !t.is_empty())
            .collect();
        if !text.is_empty() {
            note(at(*pos + Vec2::new(10.0, 6.0)), &text.join("\n"));
        }
        marks.clone()
    }

    fn paint_labels(&self, ui: &egui::Ui, rect: egui::Rect, scene: &Scene) {
        // Above this size glyphs get expensive to rasterise and the label
        // fills the screen anyway.
        const MAX_SIZE: f32 = 512.0;
        let zoom = self.viewport.zoom;
        // Zoomed out past reading even the tallest text: nothing to visit.
        // With the text floor on, small text is lifted instead of dropped, so
        // the cull is lower than `min_label_size`.
        let cull = crate::declutter::candidate_size(self.min_text_px, self.min_label_size);
        if scene.max_label_height() * zoom < cull {
            return;
        }
        let painter = ui.painter_at(rect);
        let bold_family = painter.fonts(|fonts| self.bold_face(fonts.definitions()));
        // Collect what is readable first, so a dense page draws its largest
        // text within the budget instead of whichever labels come first.
        let mut todo: Vec<(f32, ItemId, usize)> = Vec::new();
        for id in scene.query(&self.viewport.visible_world()) {
            let Some(item) = scene.get(id) else { continue };
            if item.labels.is_empty() || !scene.layer(item.layer).is_some_and(|l| l.visible) {
                continue;
            }
            for (i, l) in item.labels.iter().enumerate() {
                let size = l.height * zoom;
                if size >= cull && size <= MAX_SIZE && !l.text.is_empty() {
                    todo.push((size, id, i));
                }
            }
        }
        if todo.len() > self.label_budget {
            todo.select_nth_unstable_by(self.label_budget, |a, b| b.0.total_cmp(&a.0));
            todo.truncate(self.label_budget);
        }
        // The floor: lift small text to it, hide what cannot fit.
        let mut boosts = vec![1.0_f32; todo.len()];
        let mut shown: Vec<bool> = vec![true; todo.len()];
        if self.min_text_px > 0.0 {
            let candidates = todo
                .iter()
                .enumerate()
                .filter_map(|(key, &(size, id, i))| {
                    let l = &scene.get(id)?.labels[i];
                    let b = l.bounds();
                    let a = self.viewport.world_to_screen(l.pos);
                    let (p, q) = (
                        self.viewport.world_to_screen(b.min),
                        self.viewport.world_to_screen(b.max),
                    );
                    Some(crate::declutter::Candidate {
                        key,
                        priority: l.priority,
                        size,
                        anchor: a,
                        bounds: Aabb::from_corners(p, q),
                    })
                })
                .collect();
            shown = vec![false; todo.len()];
            for placed in crate::declutter::plan(candidates, self.min_text_px) {
                shown[placed.key] = true;
                boosts[placed.key] = placed.boost;
            }
        }
        for (n, (size, id, i)) in todo.into_iter().enumerate() {
            if !shown[n] {
                continue;
            }
            let size = size * boosts[n];
            let Some(item) = scene.get(id) else { continue };
            let l = &item.labels[i];
            let highlight = if self.selection.contains(id) {
                Some(self.style.selection.color)
            } else if self.hovered == Some(id) {
                Some(self.style.hover.color)
            } else {
                None
            };
            // Quantise sizes (12 steps per octave) so continuous zooming
            // does not rasterise a new font size every frame.
            let size = 2f32.powf((size.log2() * 12.0).round() / 12.0);
            let [r, g, b, a] = highlight.filter(|_| !l.keep_color).unwrap_or(l.color).0;
            let color = egui::Color32::from_rgba_unmultiplied(r, g, b, a);
            let family = match (&bold_family, l.bold) {
                (Some(f), true) => f.clone(),
                _ => egui::FontFamily::Proportional,
            };
            let galley =
                painter.layout_no_wrap(l.text.clone(), egui::FontId::new(size, family), color);
            let f = l.anchor.fractions();
            let offset = egui::vec2(f.x * galley.size().x, f.y * galley.size().y);
            let s = self.viewport.world_to_screen(l.pos);
            let anchor = rect.min + egui::vec2(s.x, s.y);
            let rot = egui::emath::Rot2::from_angle(l.rotation);
            let at = anchor - rot * offset;
            if l.bold && bold_family.is_none() {
                // No bold face is loaded: a second pass shifted along the
                // baseline thickens the strokes.
                let shift = rot * egui::vec2((size * crate::text::BOLD_SHIFT).max(0.5), 0.0);
                painter.add(
                    egui::epaint::TextShape::new(at + shift, galley.clone(), color)
                        .with_angle(l.rotation),
                );
            }
            painter.add(egui::epaint::TextShape::new(at, galley, color).with_angle(l.rotation));
        }
    }

    /// [`Self::bold_family`] when egui knows it (laying out text in an
    /// unknown family panics).
    fn bold_face(&self, fonts: &egui::FontDefinitions) -> Option<egui::FontFamily> {
        self.bold_family
            .clone()
            .filter(|f| fonts.families.contains_key(f))
    }

    fn grid_mesh(&self, mesh: &mut Mesh, scene: &Scene, pixels_per_point: f32) {
        // A page is painted here, under everything: the sheet fill, then the
        // grid dots inside it, so strokes and text of the scene cover them.
        let sheet = scene.sheet();
        if let Some(sh) = sheet
            && let Some(fill) = sh.fill
        {
            let r = sh.rect;
            tessellate::fill_polygon(
                mesh,
                &[
                    r.min,
                    Vec2::new(r.max.x, r.min.y),
                    r.max,
                    Vec2::new(r.min.x, r.max.y),
                ],
                fill,
                true,
            );
        }
        let Some(g) = self.grid.visible_lines(&self.viewport) else {
            return;
        };
        let vis = self.viewport.visible_world();
        if let Some(sh) = sheet
            && self.grid.style == GridStyle::Dots
        {
            let zoom = self.viewport.zoom;
            let major_step = (self.grid.spacing * self.grid.major_every.max(1) as f32).max(g.step);
            // Quieter when zoomed out (DESIGN section 4): dots need 8 px
            // between them, so only the majors remain at first and then
            // none.
            let (minor, major) = (g.step * zoom >= 8.0, major_step * zoom >= 8.0);
            let boxes: Vec<Aabb> = scene
                .query(&vis)
                .into_iter()
                .filter_map(|id| scene.get(id))
                .flat_map(|item| item.labels.iter().map(crate::text::Label::bounds))
                .collect();
            let ppp = pixels_per_point.max(f32::EPSILON);
            for d in g.dots(&sh.rect, &sh.lines, &boxes, minor, major) {
                // One device pixel for a minor dot, two for a major one.
                let w = if d.major { 2.0 } else { 1.0 } / ppp;
                let half = self.viewport.screen_to_world_len(w * 0.5);
                let c = if d.major {
                    self.style.grid_major
                } else {
                    self.style.grid_minor
                };
                tessellate::stroke_run(
                    mesh,
                    &[d.pos - Vec2::new(half, 0.0), d.pos + Vec2::new(half, 0.0)],
                    &Stroke::screen(w, c),
                );
            }
            return;
        }
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
            SnapKind::OnLine => Shape::polyline(
                [
                    p + Vec2::new(0.0, -r),
                    p + Vec2::new(r, 0.0),
                    p + Vec2::new(0.0, r),
                    p + Vec2::new(-r, 0.0),
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
    use super::dashes;

    #[test]
    fn a_dashed_edge_has_gaps_and_ends_on_the_corner() {
        let d = dashes(Vec2::ZERO, Vec2::new(20.0, 0.0), 6.0, 4.0);
        assert_eq!(d.len(), 2);
        assert_eq!(d[0], [Vec2::ZERO, Vec2::new(6.0, 0.0)]);
        assert_eq!(d[1][0], Vec2::new(10.0, 0.0));
        assert_eq!(d[1][1], Vec2::new(16.0, 0.0));
        assert!(dashes(Vec2::ZERO, Vec2::ZERO, 6.0, 4.0).is_empty());
    }
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

    fn left(x: f32, y: f32, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos: egui::pos2(x, y),
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        }
    }

    /// However the press and release are spread over frames, with or
    /// without a move first (X servers and test drivers differ), a click on
    /// an item selects it.
    #[test]
    fn a_click_selects_whatever_the_event_timing() {
        let seqs: Vec<(&str, Vec<Vec<egui::Event>>)> = vec![
            (
                "move, press, release frames",
                vec![
                    vec![moved(0.0, 0.0)],
                    vec![left(0.0, 0.0, true)],
                    vec![left(0.0, 0.0, false)],
                ],
            ),
            (
                "move then press+release same frame",
                vec![
                    vec![moved(0.0, 0.0)],
                    vec![left(0.0, 0.0, true), left(0.0, 0.0, false)],
                ],
            ),
            (
                "move+press+release same frame",
                vec![vec![
                    moved(0.0, 0.0),
                    left(0.0, 0.0, true),
                    left(0.0, 0.0, false),
                ]],
            ),
            (
                "press+release, no move",
                vec![vec![left(0.0, 0.0, true), left(0.0, 0.0, false)]],
            ),
            (
                "press, release, no move",
                vec![vec![left(0.0, 0.0, true)], vec![left(0.0, 0.0, false)]],
            ),
            (
                "focus, move, press, release",
                vec![
                    vec![egui::Event::WindowFocused(true), moved(0.0, 0.0)],
                    vec![left(0.0, 0.0, true)],
                    vec![left(0.0, 0.0, false)],
                ],
            ),
        ];
        for (name, frames) in seqs {
            let (ctx, mut view, scene) = setup();
            let p = view.viewport.world_to_screen(Vec2::new(50.0, 0.0));
            for evs in frames {
                let evs = evs
                    .into_iter()
                    .map(|e| match e {
                        egui::Event::PointerMoved(_) => moved(p.x, p.y),
                        egui::Event::PointerButton {
                            button,
                            pressed,
                            modifiers,
                            ..
                        } => egui::Event::PointerButton {
                            pos: egui::pos2(p.x, p.y),
                            button,
                            pressed,
                            modifiers,
                        },
                        e => e,
                    })
                    .collect();
                frame(&ctx, &mut view, &scene, evs);
            }
            frame(&ctx, &mut view, &scene, vec![]);
            assert_eq!(view.selection.ids(), &[ItemId(1)], "{name}");
        }
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
    fn related_items_are_highlighted() {
        let (_ctx, mut view, scene) = setup();
        let build = |view: &mut CanvasView| drop(view.build_frame(1.0, &scene, None, None));
        assert!(view.highlight.is_none());
        view.set_related([ItemId(1)]);
        let rev = view.related_revision;
        view.set_related([ItemId(1)]);
        assert_eq!(view.related_revision, rev, "unchanged sets keep the cache");
        build(&mut view);
        let key = view.highlight.as_ref().map(|h| h.key);
        assert!(
            view.highlight
                .as_ref()
                .is_some_and(|h| !h.mesh.indices.is_empty())
        );
        view.set_related([]);
        build(&mut view);
        assert!(view.highlight.is_none());
        view.set_related([ItemId(1)]);
        build(&mut view);
        assert_ne!(view.highlight.as_ref().map(|h| h.key), key);
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

    /// A reveal waits for a frame whose scene holds its target (a jump to
    /// another page waits for that page's scene), is used up by it, and
    /// is dropped when the target never comes.
    #[test]
    fn reveal_centres_an_item_out_of_view_on_the_next_frame() {
        let (ctx, mut view, scene) = setup();
        view.viewport.center = Vec2::new(5000.0, 5000.0);
        view.viewport.zoom = 4.0;
        view.reveal([ItemId(99)]);
        for _ in 0..REVEAL_WAIT_FRAMES {
            frame(&ctx, &mut view, &scene, vec![]);
        }
        assert_eq!(view.viewport.center, Vec2::new(5000.0, 5000.0));
        assert!(view.reveal_pending.is_none());
        // The target arrives a frame late: the reveal waits for it.
        view.reveal([ItemId(2)]);
        frame(&ctx, &mut view, &scene, vec![]);
        assert_eq!(view.viewport.center, Vec2::new(5000.0, 5000.0));
        let mut later = self::scene();
        later.insert(
            ItemId(2),
            Item::new(crate::LayerId(0)).with(Primitive::stroked(
                Shape::line(Vec2::new(300.0, 0.0), Vec2::new(320.0, 0.0)),
                Stroke::default(),
            )),
        );
        frame(&ctx, &mut view, &later, vec![]);
        let at = view.viewport.world_to_screen(Vec2::new(310.0, 0.0));
        assert!((at - Vec2::new(400.0, 300.0)).length() < 1.0, "{at}");
        // An area is revealed like an item.
        view.reveal_area(Aabb::new(Vec2::new(-505.0, -5.0), Vec2::new(-495.0, 5.0)));
        frame(&ctx, &mut view, &scene, vec![]);
        let at = view.viewport.world_to_screen(Vec2::new(-500.0, 0.0));
        assert!((at - Vec2::new(400.0, 300.0)).length() < 1.0, "{at}");
        view.viewport.center = Vec2::new(5000.0, 5000.0);
        view.reveal([ItemId(1)]);
        frame(&ctx, &mut view, &scene, vec![]);
        let at = view.viewport.world_to_screen(Vec2::new(50.0, 0.0));
        assert!((at - Vec2::new(400.0, 300.0)).length() < 1.0, "{at}");
        // Panning away afterwards is not undone.
        view.viewport.center = Vec2::new(-700.0, 0.0);
        frame(&ctx, &mut view, &scene, vec![]);
        assert!(view.viewport.center.x < -600.0);
    }

    /// The bold family is used only when egui has it registered; any
    /// other setting falls back to faux bold rather than panicking in
    /// layout.
    #[test]
    fn bold_face_only_when_registered() {
        let strong = egui::FontFamily::Name("strong".into());
        let mut fonts = egui::FontDefinitions::default();
        let regular = fonts.families[&egui::FontFamily::Proportional].clone();
        fonts.families.insert(strong.clone(), regular);
        let mut view = CanvasView::new();
        assert_eq!(view.bold_face(&fonts), None);
        view.bold_family = Some(egui::FontFamily::Name("missing".into()));
        assert_eq!(view.bold_face(&fonts), None);
        view.bold_family = Some(strong.clone());
        assert_eq!(view.bold_face(&fonts), Some(strong));
    }

    /// A page: an unselectable sheet frame over 420 x 297 mm and one drawn
    /// item in its left corner.
    fn page_scene() -> Scene {
        let mut s = Scene::new();
        s.set_sheet(Some(crate::scene::Sheet {
            rect: Aabb::new(Vec2::ZERO, Vec2::new(420.0, 297.0)),
            fill: None,
            lines: Vec::new(),
        }));
        s.insert(
            ItemId(1),
            Item::new(crate::LayerId(0))
                .with(Primitive::stroked(
                    Shape::rect(Vec2::ZERO, Vec2::new(420.0, 297.0)),
                    Stroke::default(),
                ))
                .unselectable(),
        );
        s.insert(
            ItemId(2),
            Item::new(crate::LayerId(0)).with(Primitive::stroked(
                Shape::rect(Vec2::new(20.0, 20.0), Vec2::new(80.0, 120.0)),
                Stroke::default(),
            )),
        );
        s
    }

    #[test]
    fn fit_brings_the_drawing_into_view_and_fit_sheet_the_paper() {
        let scene = page_scene();
        let mut view = CanvasView::new();
        view.viewport.size = Vec2::new(800.0, 600.0);
        view.fit_scene(&scene, Fit::Sheet);
        let sheet_zoom = view.viewport.zoom;
        assert!(
            view.viewport
                .visible_world()
                .contains_point(Vec2::new(410.0, 290.0))
        );
        view.fit_scene(&scene, Fit::Content);
        assert!(
            view.viewport.zoom > sheet_zoom * 2.0,
            "closer than the sheet"
        );
        let seen = view.viewport.visible_world();
        assert!(seen.contains_point(Vec2::new(20.0, 20.0)));
        assert!(seen.contains_point(Vec2::new(80.0, 120.0)));
        assert!(!seen.contains_point(Vec2::new(410.0, 290.0)));
    }

    #[test]
    fn a_drawing_that_fills_most_of_the_sheet_fits_the_whole_sheet_centred() {
        let mut scene = page_scene();
        scene.insert(
            ItemId(3),
            Item::new(crate::LayerId(0)).with(Primitive::stroked(
                Shape::rect(Vec2::new(10.0, 10.0), Vec2::new(300.0, 250.0)),
                Stroke::default(),
            )),
        );
        let mut view = CanvasView::new();
        view.viewport.size = Vec2::new(800.0, 600.0);
        view.fit_scene(&scene, Fit::Content);
        let seen = view.viewport.visible_world();
        assert!(seen.contains_point(Vec2::new(5.0, 5.0)));
        assert!(seen.contains_point(Vec2::new(415.0, 292.0)));
    }

    fn column_scene(top: f32, bottom: f32) -> Scene {
        let mut scene = page_scene();
        scene.insert(
            ItemId(3),
            Item::new(crate::LayerId(0)).with(Primitive::stroked(
                Shape::rect(Vec2::new(15.0, top), Vec2::new(78.0, bottom)),
                Stroke::default(),
            )),
        );
        scene
    }

    #[test]
    fn a_short_narrow_column_on_a_big_sheet_is_fitted_close() {
        // 62 x 100 on 420 x 297: a tenth of the sheet and more than twice as
        // large close up as the whole sheet is.
        let scene = column_scene(16.0, 116.0);
        let mut view = CanvasView::new();
        view.viewport.size = Vec2::new(1600.0, 836.0);
        view.fit_scene(&scene, Fit::Content);
        let seen = view.viewport.visible_world();
        assert!(
            !seen.contains_point(Vec2::new(410.0, 290.0)),
            "sheet corner out of view"
        );
        assert!(view.viewport.zoom > 3.0, "{}", view.viewport.zoom);
    }

    #[test]
    fn a_tall_narrow_column_shows_the_whole_sheet_with_its_title_block() {
        // 62 x 187: nearly the sheet's height, so a close fit is hardly closer
        // than the sheet, and it would cut off the foot with the title block.
        let scene = column_scene(16.0, 203.0);
        let mut view = CanvasView::new();
        view.viewport.size = Vec2::new(1600.0, 836.0);
        view.fit_scene(&scene, Fit::Content);
        let seen = view.viewport.visible_world();
        assert!(seen.contains_point(Vec2::new(410.0, 290.0)), "{seen:?}");
        assert!(seen.contains_point(Vec2::new(10.0, 10.0)), "{seen:?}");
    }

    #[test]
    fn a_fit_leaves_the_reserved_bottom_strip_clear() {
        let scene = page_scene();
        let mut view = CanvasView::new();
        view.viewport.size = Vec2::new(800.0, 600.0);
        view.fit_reserve_bottom = 80.0;
        view.fit_scene(&scene, Fit::Sheet);
        let bottom = view.viewport.world_to_screen(Vec2::new(210.0, 297.0)).y;
        assert!(bottom <= 600.0 - 80.0 + 0.5, "sheet bottom at {bottom}");
        let top = view.viewport.world_to_screen(Vec2::ZERO).y;
        assert!(top >= 0.0);
    }

    #[test]
    fn a_dock_that_reports_its_height_after_the_fit_still_gets_its_strip() {
        let scene = page_scene();
        let mut view = CanvasView::new();
        view.viewport.size = Vec2::new(800.0, 600.0);
        view.fit_scene(&scene, Fit::Sheet);
        view.fit_settle = Some(FitSettle::of(
            Fit::Sheet,
            &view.viewport,
            view.fit_reserve_bottom,
        ));
        // The dock is drawn after the canvas and only now says how tall it is.
        view.fit_reserve_bottom = 80.0;
        view.settle_fit(&scene, false);
        let bottom = view.viewport.world_to_screen(Vec2::new(210.0, 297.0)).y;
        assert!(bottom <= 600.0 - 80.0 + 0.5, "sheet bottom at {bottom}");
    }

    #[test]
    fn a_fit_follows_the_canvas_when_a_panel_takes_width_after_it() {
        let scene = page_scene();
        let mut view = CanvasView::new();
        view.viewport.size = Vec2::new(1280.0, 700.0);
        view.fit_scene(&scene, Fit::Content);
        view.fit_settle = Some(FitSettle::of(
            Fit::Content,
            &view.viewport,
            view.fit_reserve_bottom,
        ));
        // The outline opens and takes 300 points.
        view.viewport.size = Vec2::new(980.0, 700.0);
        view.settle_fit(&scene, false);
        let seen = view.viewport.visible_world();
        assert!(seen.contains_point(Vec2::new(20.0, 20.0)));
        assert!(seen.contains_point(Vec2::new(80.0, 120.0)));
    }

    #[test]
    fn a_view_the_person_moved_is_left_alone_when_the_canvas_changes() {
        let scene = page_scene();
        let mut view = CanvasView::new();
        view.viewport.size = Vec2::new(1280.0, 700.0);
        view.fit_scene(&scene, Fit::Content);
        view.fit_settle = Some(FitSettle::of(
            Fit::Content,
            &view.viewport,
            view.fit_reserve_bottom,
        ));
        let before = view.viewport.center;
        view.viewport.size = Vec2::new(980.0, 700.0);
        view.settle_fit(&scene, true);
        assert_eq!(view.viewport.center, before);
        assert!(view.fit_settle.is_none());
    }

    #[test]
    fn a_reveal_after_the_fit_is_not_undone_by_a_later_resize() {
        let scene = page_scene();
        let mut view = CanvasView::new();
        view.viewport.size = Vec2::new(1280.0, 700.0);
        view.fit_scene(&scene, Fit::Content);
        view.fit_settle = Some(FitSettle::of(
            Fit::Content,
            &view.viewport,
            view.fit_reserve_bottom,
        ));
        // Something outside the fit moves the view (a reveal or a jump).
        view.viewport.reveal(
            &Aabb::new(Vec2::new(300.0, 250.0), Vec2::new(320.0, 270.0)),
            48.0,
            4.0,
        );
        let revealed = view.viewport.center;
        view.viewport.size = Vec2::new(980.0, 700.0);
        view.settle_fit(&scene, false);
        assert_eq!(view.viewport.center, revealed);
        assert!(view.fit_settle.is_none());
    }

    #[test]
    fn a_lone_small_symbol_is_not_blown_up_to_fill_the_screen() {
        let mut scene = page_scene();
        scene.insert(
            ItemId(2),
            Item::new(crate::LayerId(0)).with(Primitive::stroked(
                Shape::rect(Vec2::new(20.0, 20.0), Vec2::new(22.0, 22.0)),
                Stroke::default(),
            )),
        );
        let mut view = CanvasView::new();
        view.viewport.size = Vec2::new(800.0, 600.0);
        view.fit_scene(&scene, Fit::Content);
        assert!(view.viewport.zoom <= FIT_CONTENT_MAX_ZOOM + 1e-4);
    }

    #[test]
    fn an_empty_page_fits_the_sheet() {
        let mut scene = page_scene();
        scene.remove(ItemId(2));
        let mut view = CanvasView::new();
        view.viewport.size = Vec2::new(800.0, 600.0);
        view.fit_scene(&scene, Fit::Content);
        assert!(
            view.viewport
                .visible_world()
                .contains_point(Vec2::new(410.0, 290.0))
        );
    }
}
