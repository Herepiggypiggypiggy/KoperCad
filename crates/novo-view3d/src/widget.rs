//! The egui widget: a 3D view with orbit, pan, zoom, picking and a marquee.
//!
//! The host calls [`install`] once at start-up, keeps one [`View3d`] per
//! visible 3D face, and calls [`View3d::show`] every frame. Nothing is
//! rendered unless the scene, camera, selection or size changed, and after the
//! camera stops the image is refined once (4x MSAA, full scale).

use crate::camera::{Camera, Projection, ViewPreset};
use crate::cube::{self, CubeHit};
use crate::dimension::{self, Role};
use crate::door::DoorAnim;
use crate::feel::{self, Inertia, Transition};
use crate::governor::FrameGovernor;
use crate::labels::{self, Fade, LabelCandidate};
use crate::look::{self, Glyph, Look};
use crate::mesh::MeshLibrary;
use crate::overlay::Overlay;
use crate::pick::{self, Hit};
use crate::render::{BlitBind, Blitter, Frame, Mode, Quality, Renderer, Tier, ViewTarget};
use crate::scene::{GroupId, ItemId, ItemKind, Mark, Scene};
use crate::view_state::ViewState;
use egui::{
    Align2, Color32, FontId, Key, Modifiers, PointerButton, Pos2, Rect, Sense, Stroke, Vec2,
};
use glam::Vec3;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// GPU state of a view not painted for this long is freed.
const IDLE_VIEW_TTL: Duration = Duration::from_secs(30);
/// A dimension line shorter than this on screen is not drawn, points.
const MIN_DIMENSION_PX: f32 = 28.0;
/// Seconds without input before the settled (sharp) frame is drawn.
const SETTLE_AFTER: f64 = 0.25;
static INSTALLED: AtomicBool = AtomicBool::new(false);
static STATS: Mutex<Option<Recorder>> = Mutex::new(None);
/// Renders kept for the percentiles.
const STATS_WINDOW: usize = 120;

/// What the renderer reports about itself, for the app's frame statistics.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderStats {
    /// The graphics adapter, for example `NVIDIA GeForce RTX 3060`.
    pub adapter: String,
    /// The graphics API the adapter is used through, for example `Dx12`.
    pub backend: String,
    /// The quality tier in use: the adapter's, or lower once the frame
    /// governor has stepped down after sustained slow frames.
    pub tier: Tier,
    /// The tier the adapter type gives.
    pub adapter_tier: Tier,
    /// Frames rendered since start (an unchanged view is not rendered again).
    pub renders: u64,
    /// Items drawn, draw calls and triangles of the last render.
    pub visible_items: usize,
    pub draw_calls: u32,
    pub triangles: u64,
    /// Size of the last image, and its MSAA samples.
    pub rendered_px: (u32, u32),
    pub msaa: u32,
    /// CPU time the renderer took (cull, instances, encoding), milliseconds,
    /// median and 95th percentile of the last renders. GPU time is not in it.
    pub cpu_p50_ms: f32,
    pub cpu_p95_ms: f32,
}

struct Recorder {
    adapter: String,
    backend: String,
    tier: Tier,
    adapter_tier: Tier,
    renders: u64,
    cpu_ms: VecDeque<f32>,
    last: crate::render::FrameStats,
    msaa: u32,
}

/// The percentile `p` (0 to 1) of unsorted samples; 0 for none.
pub fn percentile(samples: &[f32], p: f32) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let mut v = samples.to_vec();
    v.sort_by(f32::total_cmp);
    v[((v.len() - 1) as f32 * p).round() as usize]
}

/// The renderer's statistics; `None` before [`install`].
pub fn render_stats() -> Option<RenderStats> {
    let guard = STATS.lock().ok()?;
    let r = guard.as_ref()?;
    let cpu: Vec<f32> = r.cpu_ms.iter().copied().collect();
    Some(RenderStats {
        adapter: r.adapter.clone(),
        backend: r.backend.clone(),
        tier: r.tier,
        adapter_tier: r.adapter_tier,
        renders: r.renders,
        visible_items: r.last.visible_items,
        draw_calls: r.last.draw_calls,
        triangles: r.last.triangles,
        rendered_px: r.last.rendered_px,
        msaa: r.msaa,
        cpu_p50_ms: percentile(&cpu, 0.5),
        cpu_p95_ms: percentile(&cpu, 0.95),
    })
}

/// The tier the adapter type gives; `None` before [`install`].
fn adapter_tier() -> Option<Tier> {
    STATS.lock().ok()?.as_ref().map(|r| r.adapter_tier)
}

fn record(stats: crate::render::FrameStats, msaa: u32, tier: Tier) {
    if let Ok(mut guard) = STATS.lock()
        && let Some(r) = guard.as_mut()
    {
        if r.cpu_ms.len() == STATS_WINDOW {
            r.cpu_ms.pop_front();
        }
        r.cpu_ms.push_back(stats.cpu_ms);
        r.renders += 1;
        r.last = stats;
        r.msaa = msaa;
        r.tier = tier;
    }
}

/// Registers the renderer with egui's wgpu renderer. Call once at start-up.
/// `msaa_samples` is eframe's `NativeOptions::multisampling` (0 or 1 = off).
pub fn install(render_state: &egui_wgpu::RenderState, msaa_samples: u32) {
    let renderer = Renderer::new(&render_state.device);
    let blitter = Blitter::new(
        &render_state.device,
        render_state.target_format,
        msaa_samples.max(1),
    );
    let info = render_state.adapter.get_info();
    let tier = Tier::of(&info);
    if let Ok(mut guard) = STATS.lock() {
        *guard = Some(Recorder {
            adapter: info.name.clone(),
            backend: format!("{:?}", info.backend),
            tier,
            adapter_tier: tier,
            renders: 0,
            cpu_ms: VecDeque::with_capacity(STATS_WINDOW),
            last: Default::default(),
            msaa: 1,
        });
    }
    render_state
        .renderer
        .write()
        .callback_resources
        .insert(Resources {
            renderer,
            blitter,
            tier,
            views: HashMap::new(),
        });
    INSTALLED.store(true, Ordering::Relaxed);
}

/// Whether [`install`] ran. Without it (no wgpu adapter) [`View3d::show`] paints a notice.
pub fn is_installed() -> bool {
    INSTALLED.load(Ordering::Relaxed)
}

struct Resources {
    renderer: Renderer,
    blitter: Blitter,
    tier: Tier,
    views: HashMap<u64, Slot>,
}

struct Slot {
    target: ViewTarget,
    key: Option<u64>,
    bind: Option<BlitBind>,
    used: Instant,
}

struct Callback {
    id: u64,
    scene: Arc<Scene>,
    library: Arc<MeshLibrary>,
    camera: Camera,
    state: ViewState,
    look: Look,
    size_px: (u32, u32),
    mode: Mode,
    /// The tier the frame governor stepped down to, if it did.
    governed: Option<Tier>,
    overlays: Arc<Vec<Overlay>>,
    overlay_revision: u64,
    key: u64,
}

impl egui_wgpu::CallbackTrait for Callback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(res) = resources.get_mut::<Resources>() else {
            return Vec::new();
        };
        let Resources {
            renderer,
            blitter,
            tier,
            views,
        } = res;
        let now = Instant::now();
        views.retain(|&id, s| id == self.id || now - s.used < IDLE_VIEW_TTL);
        let slot = views.entry(self.id).or_insert_with(|| Slot {
            target: ViewTarget::new(renderer, device),
            key: None,
            bind: None,
            used: now,
        });
        slot.used = now;
        if slot.key != Some(self.key) {
            let used = self.governed.unwrap_or(*tier);
            let quality = Quality::new(self.mode, used);
            let stats = renderer.render(
                device,
                queue,
                encoder,
                &mut slot.target,
                &Frame {
                    scene: &self.scene,
                    state: &self.state,
                    camera: &self.camera,
                    library: &self.library,
                    look: &self.look,
                    size_px: self.size_px,
                    quality,
                    overlays: &self.overlays,
                    overlay_revision: self.overlay_revision,
                },
            );
            record(stats, quality.msaa, used);
            slot.key = Some(self.key);
        }
        if slot
            .bind
            .as_ref()
            .is_none_or(|b| !b.is_current(&slot.target))
        {
            slot.bind = blitter.bind(device, &slot.target);
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        if let Some(res) = resources.get::<Resources>()
            && let Some(slot) = res.views.get(&self.id)
            && let Some(bind) = &slot.bind
        {
            res.blitter.paint(pass, bind);
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ViewOptions {
    /// Camera moves jump instead of easing (the app's reduced-motion setting).
    pub reduced_motion: bool,
}

/// A click on the view. `id` is `None` on empty space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Click {
    pub id: Option<ItemId>,
    pub modifiers: Modifiers,
    /// Alt + click: the deepest item (repeated clicks step back up).
    pub deepest: bool,
}

impl View3dOutput {
    fn new(rect: Rect) -> Self {
        Self {
            rect,
            hover: None,
            clicked: None,
            marquee: None,
            context: None,
            double_clicked: None,
            plate_point: None,
            item_drag: None,
            camera_changed: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct View3dOutput {
    pub rect: Rect,
    pub hover: Option<ItemId>,
    pub clicked: Option<Click>,
    /// Items inside a finished marquee.
    pub marquee: Option<Vec<ItemId>>,
    /// A short right press: where, and the item under it. The host opens its
    /// context menu.
    pub context: Option<(Pos2, Option<ItemId>)>,
    /// A double click on an item (the view has already fitted it).
    pub double_clicked: Option<ItemId>,
    /// The 2D plate point (x, y down, mm) under the pointer.
    pub plate_point: Option<(f64, f64)>,
    /// A drag that started on a device, rail or duct while
    /// [`View3d::drag_items`] is on: how far the pointer has moved on the
    /// plate since the press, and whether the button is still down.
    pub item_drag: Option<ItemDrag>,
    pub camera_changed: bool,
}

/// A device, rail or duct being dragged across the plate in the view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ItemDrag {
    pub id: ItemId,
    /// The pointer's move on the plate plane since the press (x right, y
    /// down, mm).
    pub delta: (f64, f64),
    /// The button was released this frame.
    pub done: bool,
}

/// View cube: size and distance from the top-right corner of the view, in points.
const CUBE_SIZE: f32 = 88.0;
const CUBE_MARGIN: f32 = 12.0;
/// The cube is left out of views smaller than this on either side.
const CUBE_MIN_VIEW: f32 = 200.0;
/// One mouse wheel notch in egui's scroll points.
const POINTS_PER_NOTCH: f32 = 50.0;

fn color(c: [u8; 4], alpha: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(
        c[0],
        c[1],
        c[2],
        (c[3] as f32 * alpha.clamp(0.0, 1.0)) as u8,
    )
}

/// A chip's 1 px line: copper when selected, steel blue when hovered, the
/// quiet border otherwise.
fn chip_border(selected: bool, hovered: bool, look: &Look, alpha: f32) -> Color32 {
    let edge = if selected {
        look.selection.edge
    } else if hovered {
        look.hover.edge
    } else {
        return color(look.overlay.chip_border, alpha);
    };
    color3(edge, (alpha.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn color3(c: [u8; 3], alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], alpha)
}

/// One 3D view: camera, user state and input handling.
pub struct View3d {
    id: u64,
    pub camera: Camera,
    pub state: ViewState,
    transition: Option<Transition>,
    inertia: Inertia,
    /// Set by a drag that orbited, so release hands its speed to `inertia`.
    orbited: bool,
    /// Distance travelled by the current right drag, for the click threshold.
    orbit_travel: f32,
    /// The current left drag started on the view cube.
    cube_drag: bool,
    /// The camera is in an axis view entered from the cube: the first orbit
    /// returns to perspective.
    auto_ortho: bool,
    needs_fit: bool,
    last_activity: f64,
    marquee_from: Option<Pos2>,
    /// Dragging a device, rail or duct moves it instead of drawing a marquee.
    pub drag_items: bool,
    /// The item under the press of the current left drag, and the plate point
    /// of the press.
    item_grab: Option<(ItemId, (f64, f64))>,
    /// Where the last Alt + click landed and how many times in a row.
    alt_cycle: Option<(Pos2, usize)>,
    last_aspect: f32,
    label_fades: HashMap<ItemId, Fade>,
    overlays: Arc<Vec<Overlay>>,
    overlay_revision: u64,
    door: Option<DoorAnim>,
    /// Steps the render quality down when moving frames stay slow.
    governor: Option<FrameGovernor<Tier>>,
    /// The source scene and its copy with the animated door angle.
    door_scene: Option<(Arc<Scene>, Arc<Scene>)>,
}

impl View3d {
    /// `id` must be unique among the views of the app.
    pub fn new(id: u64) -> Self {
        Self {
            id,
            camera: Camera::default(),
            state: ViewState::default(),
            transition: None,
            inertia: Inertia::default(),
            orbited: false,
            orbit_travel: 0.0,
            cube_drag: false,
            auto_ortho: false,
            needs_fit: true,
            last_activity: f64::NEG_INFINITY,
            marquee_from: None,
            drag_items: false,
            item_grab: None,
            alt_cycle: None,
            last_aspect: 1.0,
            label_fades: HashMap::new(),
            overlays: Arc::new(Vec::new()),
            overlay_revision: 0,
            door: None,
            governor: None,
            door_scene: None,
        }
    }

    /// Turns the camera round the cabinet at `rad_per_s` for this frame and
    /// asks for the next one: a steady load for measuring frame times without
    /// a hand on the mouse. Call it before [`View3d::show`] every frame.
    pub fn spin(&mut self, ctx: &egui::Context, rad_per_s: f32) {
        let dt = ctx.input(|i| i.unstable_dt).clamp(0.0, 0.1);
        self.camera.yaw += rad_per_s * dt;
        self.last_activity = ctx.input(|i| i.time);
        self.transition = None;
        ctx.request_repaint();
    }

    /// Replaces the overlays drawn over the cabinet (see `overlay`); an empty
    /// list clears them. Overlays are not part of the scene: picking, marquee,
    /// labels and the camera fit ignore them. Invalid overlays (mismatched
    /// arrays, out of range indices, non-finite coordinates) are skipped.
    pub fn set_overlays(&mut self, overlays: Vec<Overlay>) {
        if *self.overlays != overlays {
            self.overlays = Arc::new(overlays);
            self.overlay_revision += 1;
        }
    }

    /// Feeds one frame interval to the frame governor while the view moves and
    /// returns the tier it stepped down to, if it did. Never steps up by
    /// itself; `reset_quality` starts again from the adapter's tier.
    fn govern(&mut self, ctx: &egui::Context, moving: bool) -> Option<Tier> {
        if self.governor.is_none() {
            self.governor = adapter_tier().map(|t| FrameGovernor::new(t, false));
        }
        let gov = self.governor.as_mut()?;
        if moving {
            let ms = (ctx.input(|i| i.unstable_dt) * 1000.0).clamp(0.0, 100.0);
            if gov.frame(ms).is_some() {
                ctx.request_repaint();
            }
        }
        (Some(gov.tier) != adapter_tier()).then_some(gov.tier)
    }

    /// Forgets the governor's step-downs: the next moving frames start again
    /// from the adapter's own tier.
    pub fn reset_quality(&mut self) {
        self.governor = None;
    }

    /// Swings the hinge group (the door) to `angle` radians, easing unless
    /// reduced motion is on. This is view state: the project is not touched
    /// and the angle outlives scene rebuilds. `open_angle` gives the signed
    /// angle for "open by N degrees".
    pub fn set_door(&mut self, group: GroupId, angle: f32, now: f64, reduced: bool) {
        match &mut self.door {
            Some(d) if d.group == group => d.set_target(angle, now, reduced),
            _ => {
                // First use: start from the angle the scene has (closed).
                let mut d = DoorAnim::at(group, 0.0);
                d.set_target(angle, now, reduced);
                self.door = Some(d);
            }
        }
    }

    /// The door angle being shown (radians), if the view drives one.
    pub fn door_angle(&self, now: f64) -> Option<f32> {
        self.door.as_ref().map(|d| d.angle(now))
    }

    /// Hands the door back to the scene's own angle.
    pub fn release_door(&mut self) {
        self.door = None;
        self.door_scene = None;
    }

    /// The scene to draw: the given one, or a copy with the door at its
    /// animated angle.
    fn with_door(&mut self, scene: &Arc<Scene>, now: f64, ctx: &egui::Context) -> Arc<Scene> {
        let Some(door) = self.door else {
            return scene.clone();
        };
        let angle = door.angle(now);
        if door.animating(now) {
            ctx.request_repaint();
        }
        if scene.group(door.group).is_none_or(|g| g.angle == angle) {
            return scene.clone();
        }
        let same_source = matches!(&self.door_scene, Some((src, _)) if Arc::ptr_eq(src, scene));
        if !same_source {
            self.door_scene = Some((scene.clone(), Arc::new((**scene).clone())));
        }
        let (_, adjusted) = self.door_scene.as_mut().expect("just set");
        Arc::make_mut(adjusted).set_group_angle(door.group, angle);
        adjusted.clone()
    }

    /// Frames the whole scene on the next frame.
    pub fn request_fit(&mut self) {
        self.needs_fit = true;
    }

    /// Eases to a view cube face from where the camera is (`view.iso` and the cube).
    pub fn go_to(&mut self, preset: ViewPreset, scene: &Scene, now: f64, reduced: bool) {
        let mut to = self.camera;
        feel::enter_standard(&mut to, preset);
        to.fit(&scene.bounds(), self.last_aspect);
        self.auto_ortho = to.projection == Projection::Orthographic;
        self.animate_to(to, now, reduced);
    }

    /// `view.fit`: the selection, or everything when nothing is selected.
    pub fn fit(&mut self, scene: &Scene, now: f64, reduced: bool) {
        let mut b = crate::math::Aabb::EMPTY;
        for id in &self.state.selection {
            if let Some(item) = scene.get(*id) {
                b = b.union(&scene.world_bounds(item));
            }
        }
        if b.is_empty() {
            b = scene.bounds();
        }
        let mut to = self.camera;
        to.fit(&b, self.last_aspect);
        self.animate_to(to, now, reduced);
    }

    fn animate_to(&mut self, to: Camera, now: f64, reduced: bool) {
        self.inertia.stop();
        let mut tr = Transition::new(self.camera, to, reduced);
        if tr.done() {
            self.camera = tr.step(0.0);
            self.transition = None;
        } else {
            self.transition = Some(tr);
        }
        self.last_activity = now;
    }

    /// A click on a view cube region: a face goes to its standard view, an
    /// edge or corner turns to that direction in perspective.
    fn cube_go(&mut self, hit: CubeHit, scene: &Scene, now: f64, reduced: bool) {
        if let Some(preset) = cube::preset_of(&hit) {
            self.go_to(preset, scene, now, reduced);
        } else {
            let mut to = self.camera;
            feel::look_from(&mut to, hit.eye_dir());
            feel::leave_auto_ortho(&mut to);
            self.auto_ortho = false;
            self.animate_to(to, now, reduced);
        }
    }

    fn cube_at(&self, cube_rect: Option<Rect>, p: Pos2) -> Option<CubeHit> {
        let r = cube_rect.filter(|r| r.contains(p))?;
        let half = CUBE_SIZE * 0.5;
        let c = r.center();
        cube::view_cube_hit(&self.camera, [(p.x - c.x) / half, -(p.y - c.y) / half])
    }

    /// Screen position of a world point inside `rect`.
    pub fn project(&self, rect: Rect, scene: &Scene, p: Vec3) -> Option<Pos2> {
        let aspect = rect.width() / rect.height().max(1.0);
        let (ndc, _) = self.camera.project(p, aspect, &scene.bounds())?;
        Some(Pos2::new(
            rect.min.x + (ndc.x * 0.5 + 0.5) * rect.width(),
            rect.min.y + (0.5 - ndc.y * 0.5) * rect.height(),
        ))
    }

    fn ndc(rect: Rect, p: Pos2) -> glam::Vec2 {
        glam::Vec2::new(
            (p.x - rect.min.x) / rect.width() * 2.0 - 1.0,
            1.0 - (p.y - rect.min.y) / rect.height() * 2.0,
        )
    }

    fn pick_at(&self, rect: Rect, scene: &Scene, p: Pos2) -> Option<Hit> {
        let aspect = rect.width() / rect.height().max(1.0);
        let ray = self.camera.ray(Self::ndc(rect, p), aspect);
        pick::pick(scene, &self.state, &ray, 0)
    }

    /// Draws the view in the space `ui` has left and handles its input.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        scene: &Arc<Scene>,
        library: &Arc<MeshLibrary>,
        look: &Look,
        options: ViewOptions,
    ) -> View3dOutput {
        let size = ui.available_size().max(Vec2::splat(32.0));
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let mut out = View3dOutput::new(rect);
        if !is_installed() {
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                "3D view needs a graphics adapter",
                FontId::proportional(14.0),
                ui.visuals().weak_text_color(),
            );
            return out;
        }
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        let shown = self.with_door(scene, now, &ctx);
        let scene = &shown;
        let dt = ctx.input(|i| i.stable_dt).clamp(0.0, 0.1);
        let reduced = options.reduced_motion;
        let aspect = rect.width() / rect.height().max(1.0);
        self.last_aspect = aspect;
        let before = self.camera;
        let bounds = scene.bounds();

        if self.needs_fit && !bounds.is_empty() {
            self.camera.fit(&bounds, aspect);
            self.needs_fit = false;
            self.transition = None;
            self.last_activity = now;
        }

        // Camera transition and orbit momentum.
        if let Some(tr) = &mut self.transition {
            self.camera = tr.step(dt);
            if tr.done() {
                self.transition = None;
            } else {
                ctx.request_repaint();
            }
            self.last_activity = now;
        }
        if response.is_pointer_button_down_on() {
            self.inertia.stop();
        } else if self.inertia.active() {
            self.inertia.step(&mut self.camera, dt);
            ctx.request_repaint();
            self.last_activity = now;
        }

        let (space, alt, scroll, zoom, hover_pos) = ctx.input(|i| {
            (
                i.key_down(Key::Space),
                i.modifiers.alt,
                i.smooth_scroll_delta.y,
                i.zoom_delta(),
                i.pointer.hover_pos(),
            )
        });

        let cube_rect =
            (rect.width() >= CUBE_MIN_VIEW && rect.height() >= CUBE_MIN_VIEW).then(|| {
                Rect::from_min_size(
                    Pos2::new(
                        rect.max.x - CUBE_MARGIN - CUBE_SIZE,
                        rect.min.y + CUBE_MARGIN,
                    ),
                    Vec2::splat(CUBE_SIZE),
                )
            });
        let cube_hover = hover_pos.and_then(|p| self.cube_at(cube_rect, p));
        let pointer_on_cube = response
            .interact_pointer_pos()
            .is_some_and(|p| self.cube_at(cube_rect, p).is_some());

        if response.drag_started_by(PointerButton::Primary) {
            let origin = ctx.input(|i| i.pointer.press_origin());
            self.cube_drag = origin.is_some_and(|p| self.cube_at(cube_rect, p).is_some());
        }
        if response.drag_started_by(PointerButton::Secondary) {
            self.orbit_travel = 0.0;
        }

        // Orbit and pan.
        let drag = response.drag_delta();
        if drag != Vec2::ZERO {
            let secondary = response.dragged_by(PointerButton::Secondary);
            let orbit = secondary
                || self.cube_drag
                || (response.dragged_by(PointerButton::Primary) && alt && !space);
            let pan = response.dragged_by(PointerButton::Middle)
                || (response.dragged_by(PointerButton::Primary) && space && !self.cube_drag);
            if orbit {
                self.transition = None;
                // A short right drag is a click for the context menu.
                self.orbit_travel += drag.length();
                if !secondary || self.orbit_travel >= feel::DRAG_THRESHOLD_PX {
                    if self.auto_ortho {
                        feel::leave_auto_ortho(&mut self.camera);
                        self.auto_ortho = false;
                    }
                    let (dyaw, dpitch) = feel::orbit_delta(drag.x, drag.y);
                    self.camera.orbit(dyaw, dpitch);
                    self.orbited = true;
                }
            } else if pan {
                self.transition = None;
                self.camera.pan(drag.x, drag.y, rect.height());
            }
        }
        if response.drag_stopped() {
            if self.orbited {
                let v = ctx.input(|i| i.pointer.velocity());
                self.inertia = Inertia::release(v.x, v.y, reduced);
            }
            self.orbited = false;
            self.cube_drag = false;
        }

        // Zoom towards the pointer.
        if response.hovered() && (scroll != 0.0 || zoom != 1.0) {
            let factor = feel::zoom_factor(scroll / POINTS_PER_NOTCH) / zoom;
            let pointer = hover_pos.unwrap_or(rect.center());
            let anchor = self.pick_at(rect, scene, pointer).map(|h| h.point);
            self.transition = None;
            self.inertia.stop();
            self.camera
                .zoom_at(factor, Self::ndc(rect, pointer), aspect, anchor);
        }

        // Marquee: a plain left drag.
        let plain_left =
            response.dragged_by(PointerButton::Primary) && !alt && !space && !self.cube_drag;
        if response.drag_started_by(PointerButton::Primary) && !alt && !space && !self.cube_drag {
            let press = ctx.input(|i| i.pointer.press_origin());
            let grabbed = press
                .filter(|_| self.drag_items)
                .and_then(|p| self.pick_at(rect, scene, p).map(|h| (p, h)))
                .filter(|(_, h)| {
                    matches!(
                        h.id.kind,
                        ItemKind::Device | ItemKind::Rail | ItemKind::Duct
                    )
                });
            match grabbed {
                Some((p, hit)) => {
                    let ray = self.camera.ray(Self::ndc(rect, p), aspect);
                    let start = ray
                        .plane_hit(Vec3::ZERO, Vec3::Z)
                        .map(|t| scene.frame.plate(ray.at(t)));
                    self.item_grab = start.map(|s| (hit.id, s));
                }
                None => self.marquee_from = press,
            }
        }
        // Escape drops the item: nothing moves, and the rest of the drag does
        // nothing (it does not turn into a marquee).
        if self.item_grab.is_some() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.item_grab = None;
        }
        if let Some((id, start)) = self.item_grab {
            let released = response.drag_stopped_by(PointerButton::Primary);
            let now = hover_pos
                .or_else(|| ctx.input(|i| i.pointer.latest_pos()))
                .and_then(|p| {
                    let ray = self.camera.ray(Self::ndc(rect, p), aspect);
                    ray.plane_hit(Vec3::ZERO, Vec3::Z)
                        .map(|t| scene.frame.plate(ray.at(t)))
                });
            if let Some(now) = now {
                out.item_drag = Some(ItemDrag {
                    id,
                    delta: (now.0 - start.0, now.1 - start.1),
                    done: released,
                });
            }
            if released || !response.dragged() {
                self.item_grab = None;
            }
        }
        if let (true, Some(from), Some(to)) = (
            plain_left && self.item_grab.is_none(),
            self.marquee_from,
            hover_pos,
        ) {
            let band = Rect::from_two_pos(from, to);
            let edge = look.selection.edge;
            ui.painter().rect(
                band,
                0.0,
                color3(edge, 28),
                Stroke::new(1.0, color3(edge, 255)),
                egui::StrokeKind::Inside,
            );
        }
        if response.drag_stopped_by(PointerButton::Primary)
            && let (Some(from), Some(to)) = (self.marquee_from.take(), hover_pos)
            && (to - from).length() > feel::DRAG_THRESHOLD_PX
        {
            out.marquee = Some(pick::marquee(
                scene,
                &self.state,
                &self.camera,
                aspect,
                Self::ndc(rect, from),
                Self::ndc(rect, to),
            ));
        }

        // Hover, plate point.
        let hovering = response.hovered() && !response.dragged() && cube_hover.is_none();
        let hit = hover_pos
            .filter(|p| rect.contains(*p))
            .and_then(|p| self.pick_at(rect, scene, p));
        let new_hover = if hovering { hit.map(|h| h.id) } else { None };
        if self.state.hover != new_hover {
            self.state.hover = new_hover;
        }
        out.hover = new_hover;
        if let Some(p) = hover_pos.filter(|p| rect.contains(*p)) {
            let ray = self.camera.ray(Self::ndc(rect, p), aspect);
            out.plate_point = ray
                .plane_hit(Vec3::ZERO, Vec3::Z)
                .map(|t| scene.frame.plate(ray.at(t)));
        }

        // Clicks.
        let modifiers = ctx.input(|i| i.modifiers);
        if response.clicked_by(PointerButton::Primary)
            && let Some(p) = response.interact_pointer_pos()
        {
            if let Some(h) = self.cube_at(cube_rect, p) {
                self.cube_go(h, scene, now, reduced);
            } else {
                let ray = self.camera.ray(Self::ndc(rect, p), aspect);
                if alt {
                    let k = match self.alt_cycle {
                        Some((at, k)) if (at - p).length() < 4.0 => k + 1,
                        _ => 0,
                    };
                    self.alt_cycle = Some((p, k));
                    let hit = pick::pick_from_back(scene, &self.state, &ray, k);
                    out.clicked = Some(Click {
                        id: hit.map(|h| h.id),
                        modifiers,
                        deepest: true,
                    });
                } else {
                    self.alt_cycle = None;
                    out.clicked = Some(Click {
                        id: self.pick_at(rect, scene, p).map(|h| h.id),
                        modifiers,
                        deepest: false,
                    });
                }
            }
        }
        if response.double_clicked_by(PointerButton::Primary)
            && !pointer_on_cube
            && let Some(p) = response.interact_pointer_pos()
            && let Some(hit) = self.pick_at(rect, scene, p)
        {
            out.double_clicked = Some(hit.id);
            if let Some(item) = scene.get(hit.id) {
                let mut to = self.camera;
                to.fit(&scene.world_bounds(item), aspect);
                self.animate_to(to, now, reduced);
            }
        }
        if response.secondary_clicked()
            && !pointer_on_cube
            && let Some(p) = response.interact_pointer_pos()
        {
            out.context = Some((p, self.pick_at(rect, scene, p).map(|h| h.id)));
        }

        // Keys: only while the pointer is over the view and no text field has the keyboard.
        if response.hovered() && !ctx.egui_wants_keyboard_input() {
            let (fit, esc) = ctx.input(|i| {
                (
                    i.key_pressed(Key::F) || i.key_pressed(Key::Home),
                    i.key_pressed(Key::Escape),
                )
            });
            if fit {
                self.fit(scene, now, reduced);
            }
            if esc {
                // The ladder: a gesture in progress, then the selection. The
                // host closes its own popups before this view sees the key.
                if self.marquee_from.take().is_some() {
                    // nothing else
                } else if !self.state.selection.is_empty() {
                    self.state.selection.clear();
                }
            }
        }

        let moved = self.camera != before;
        out.camera_changed = moved;
        if moved || response.dragged() || scroll != 0.0 {
            self.last_activity = now;
        }

        // Quality: sharp once the camera has been still for a moment.
        let idle = now - self.last_activity;
        let mode = if idle < SETTLE_AFTER {
            ctx.request_repaint_after(Duration::from_secs_f64(SETTLE_AFTER - idle + 0.01));
            Mode::Interactive
        } else {
            Mode::Settled
        };

        // Frame governor: only frames of a moving view count (an idle view
        // repaints rarely, which is not slowness).
        let governed = self.govern(&ctx, idle <= f64::EPSILON);

        let ppp = ctx.pixels_per_point();
        let size_px = (
            (rect.width() * ppp).round().max(1.0) as u32,
            (rect.height() * ppp).round().max(1.0) as u32,
        );
        let key = frame_key(
            scene,
            library,
            &self.camera,
            &self.state,
            size_px,
            (mode, governed),
            self.overlay_revision,
        );
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            rect,
            Callback {
                id: self.id,
                scene: scene.clone(),
                library: library.clone(),
                camera: self.camera,
                state: self.state.clone(),
                look: *look,
                size_px,
                mode,
                governed,
                overlays: self.overlays.clone(),
                overlay_revision: self.overlay_revision,
                key,
            },
        ));

        self.paint_overlay(ui, rect, scene, look, dt, reduced);
        if let Some(cr) = cube_rect {
            self.paint_cube(&ui.painter_at(rect), cr, cube_hover, look);
        }
        out
    }

    /// Tag label chips and mark glyphs over the image.
    fn paint_overlay(
        &mut self,
        ui: &egui::Ui,
        rect: Rect,
        scene: &Scene,
        look: &Look,
        dt: f32,
        reduced: bool,
    ) {
        let painter = ui.painter_at(rect);
        let ppp = ui.ctx().pixels_per_point();
        let aspect = rect.width() / rect.height().max(1.0);
        let bounds = scene.bounds();
        let vp = self.camera.view_proj(aspect, &bounds);
        let eye = self.camera.eye();
        let mut frustum_ok = Vec::new();
        crate::cull::visible(scene, &self.state, &vp, &mut frustum_ok);

        struct Chip {
            id: ItemId,
            text: Arc<str>,
            centre: Pos2,
            size: Vec2,
            selected: bool,
            hovered: bool,
        }
        let mut chips: Vec<Chip> = Vec::new();
        let mut candidates: Vec<LabelCandidate> = Vec::new();
        // Tags are monospace 12 pt (3d-look.md section 5).
        let font = FontId::monospace(12.0);
        for i in frustum_ok {
            let item = &scene.items[i];
            let world = scene.world_matrix(item);
            let front = world.transform_point3(Vec3::new(0.0, 0.0, item.size.z));
            let dist = (front - eye).length();
            // Size of the item's narrower face edge on screen, in points.
            let size_pt = crate::cull::screen_size_px(
                &self.camera,
                item.size.x.min(item.size.y) * 0.5,
                dist,
                rect.height() * ppp,
            ) / ppp;
            if let (true, Some(label)) = (self.state.display.labels, &item.label)
                && let Some(p) = self.project(rect, scene, front)
            {
                let galley =
                    painter.layout_no_wrap(label.to_string(), font.clone(), Color32::PLACEHOLDER);
                let size = galley.size() + Vec2::new(8.0, 4.0);
                let selected = self.state.selection.contains(&item.id);
                let hovered = self.state.hover == Some(item.id);
                candidates.push(LabelCandidate {
                    id: chips.len() as u64,
                    rect: [p.x - size.x * 0.5, p.y - size.y * 0.5, size.x, size.y],
                    device_px: size_pt,
                    selected,
                    hovered,
                });
                chips.push(Chip {
                    id: item.id,
                    text: label.clone(),
                    centre: p,
                    size,
                    selected,
                    hovered,
                });
            }
            if item.mark != Mark::None && size_pt >= 12.0 {
                let style = look.mark(item.mark);
                let text = match style.glyph {
                    Glyph::Cross => "\u{00d7}",
                    Glyph::Bang => "!",
                    Glyph::DashedEdge => "?",
                    Glyph::None => continue,
                };
                let top = world.transform_point3(Vec3::new(0.0, item.size.y * 0.5, item.size.z));
                if let Some(p) = self.project(rect, scene, top) {
                    let c = style
                        .edge
                        .unwrap_or(look.overlay.chip_text[..3].try_into().unwrap());
                    let col = color3(c, 255);
                    painter.circle(p, 7.0, color(look.overlay.chip, 1.0), Stroke::new(1.5, col));
                    painter.text(
                        p,
                        Align2::CENTER_CENTER,
                        text,
                        FontId::proportional(11.0),
                        col,
                    );
                }
            }
        }

        // Label chips: which ones show, then fade them in and out.
        let shown = labels::visible_labels(&candidates);
        let mut touched: HashMap<ItemId, Fade> = HashMap::new();
        for (n, chip) in chips.iter().enumerate() {
            let on = shown.contains(&(n as u64));
            let mut fade = self
                .label_fades
                .get(&chip.id)
                .copied()
                .unwrap_or_else(Fade::label);
            fade.step(on, dt, reduced);
            if fade.value > 0.0 {
                let a = fade.value;
                let r = Rect::from_center_size(chip.centre, chip.size);
                painter.rect(
                    r,
                    3.0,
                    color(look.overlay.chip, a),
                    Stroke::new(1.0, chip_border(chip.selected, chip.hovered, look, a)),
                    egui::StrokeKind::Inside,
                );
                painter.text(
                    chip.centre,
                    Align2::CENTER_CENTER,
                    chip.text.as_ref(),
                    font.clone(),
                    color(look.overlay.chip_text, a),
                );
                if (fade.value < 1.0 && on) || (fade.value > 0.0 && !on) {
                    ui.ctx().request_repaint();
                }
                touched.insert(chip.id, fade);
            } else if on {
                ui.ctx().request_repaint();
                touched.insert(chip.id, fade);
            }
        }
        self.label_fades = touched;
        if self.state.display.dimensions {
            self.paint_dimensions(&painter, rect, scene, look);
        }
    }

    /// Measuring lines with end ticks and the length on a chip (see
    /// `dimension`). Lines too short on screen to carry their text are left
    /// out; zoom in and they appear.
    fn paint_dimensions(&self, painter: &egui::Painter, rect: Rect, scene: &Scene, look: &Look) {
        let font = FontId::monospace(11.0);
        for d in dimension::dimensions(scene, &self.state.selection) {
            let (Some(a), Some(b)) = (
                self.project(rect, scene, d.a),
                self.project(rect, scene, d.b),
            ) else {
                continue;
            };
            let galley = painter.layout_no_wrap(d.text.clone(), font.clone(), Color32::PLACEHOLDER);
            let chip = galley.size() + Vec2::new(8.0, 4.0);
            let along = b - a;
            let len = along.length();
            if len < chip.x.max(MIN_DIMENSION_PX) {
                continue;
            }
            let col = match d.role {
                Role::Overall => color(look.overlay.chip_text, 0.7),
                Role::Size => color(look.overlay.chip_text, 1.0),
                Role::Gap | Role::Edge => color3(look.selection.edge, 255),
            };
            let stroke = Stroke::new(1.0, col);
            painter.line_segment([a, b], stroke);
            let n = Vec2::new(-along.y, along.x) / len * 4.0;
            for p in [a, b] {
                painter.line_segment([p - n, p + n], stroke);
            }
            let mid = a + along * 0.5;
            painter.rect(
                Rect::from_center_size(mid, chip),
                3.0,
                color(look.overlay.chip, 1.0),
                Stroke::new(1.0, col),
                egui::StrokeKind::Inside,
            );
            painter.text(mid, Align2::CENTER_CENTER, &d.text, font.clone(), col);
        }
    }

    /// The view cube (3d-look.md section 6): nine regions per visible face,
    /// the one under the pointer in steel blue.
    fn paint_cube(
        &self,
        painter: &egui::Painter,
        cube_rect: Rect,
        hover: Option<CubeHit>,
        look: &Look,
    ) {
        let half = CUBE_SIZE * 0.5;
        let c = cube_rect.center();
        let to_screen = |q: [f32; 2]| Pos2::new(c.x + q[0] * half, c.y - q[1] * half);
        let chip: [u8; 3] = [
            look.overlay.chip[0],
            look.overlay.chip[1],
            look.overlay.chip[2],
        ];
        let text: [u8; 3] = [
            look.overlay.chip_text[0],
            look.overlay.chip_text[1],
            look.overlay.chip_text[2],
        ];
        let border = Stroke::new(1.0, color(look.overlay.chip_border, 1.0));
        for cell in cube::visible_cells(&self.camera) {
            let mut pts: Vec<Pos2> = cell.quad.iter().map(|q| to_screen(*q)).collect();
            // Screen y points down, so a counter clockwise quad can come out
            // clockwise; the tessellator wants one winding.
            let area: f32 = (0..4)
                .map(|i| {
                    let (a, b) = (pts[i], pts[(i + 1) % 4]);
                    a.x * b.y - b.x * a.y
                })
                .sum();
            if area < 0.0 {
                pts.reverse();
            }
            let base = look::mix(chip, text, 0.06 + 0.14 * cell.facing);
            let fill = if hover.is_some_and(|h| h.dir == cell.dir) {
                look::mix(base, look.hover.edge, 0.65)
            } else {
                base
            };
            painter.add(egui::Shape::convex_polygon(pts, color3(fill, 235), border));
        }
        // Names go on after every cell, so no neighbouring cell paints over
        // them, and are laid on the face so they turn with it.
        for label in cube::visible_labels(&self.camera) {
            let Some(name) = cube::face_name(label.dir) else {
                continue;
            };
            let fade = ((label.facing - 0.15) / 0.45).clamp(0.0, 1.0);
            if fade <= 0.0 {
                continue;
            }
            paint_face_label(
                painter,
                name,
                label,
                c,
                half,
                color3(text, (255.0 * fade) as u8),
            );
        }
    }
}

/// Writes `name` on a cube face: the glyph mesh of the text is mapped onto the
/// face's right and up vectors, so it is foreshortened and turned with it.
fn paint_face_label(
    painter: &egui::Painter,
    name: &str,
    label: cube::FaceLabel,
    c: Pos2,
    half: f32,
    tint: Color32,
) {
    let galley = painter.layout_no_wrap(name.to_owned(), FontId::proportional(14.0), tint);
    let size = galley.size();
    if size.x <= 0.0 {
        return;
    }
    let tex = painter.ctx().fonts(|f| f.font_image_size());
    let norm = Vec2::new(1.0 / tex[0] as f32, 1.0 / tex[1] as f32);
    // The text fills about 70 % of the face width.
    let k = 0.7 / size.x;
    let map = |p: Pos2| {
        let (a, b) = ((p.x - size.x * 0.5) * k, -(p.y - size.y * 0.5) * k);
        let x = label.center[0] + label.right[0] * a + label.up[0] * b;
        let y = label.center[1] + label.right[1] * a + label.up[1] * b;
        Pos2::new(c.x + x * half, c.y - y * half)
    };
    let mut mesh = egui::Mesh::default();
    for row in &galley.rows {
        let base = mesh.vertices.len() as u32;
        mesh.indices
            .extend(row.visuals.mesh.indices.iter().map(|i| i + base));
        mesh.vertices.extend(
            row.visuals
                .mesh
                .vertices
                .iter()
                .map(|v| egui::epaint::Vertex {
                    pos: map(row.pos + v.pos.to_vec2()),
                    uv: (v.uv.to_vec2() * norm).to_pos2(),
                    color: if v.color == Color32::PLACEHOLDER {
                        tint
                    } else {
                        v.color
                    },
                }),
        );
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// Everything that changes the picture, hashed: when it is the same as the
/// last frame the renderer does nothing.
fn frame_key(
    scene: &Scene,
    library: &MeshLibrary,
    camera: &Camera,
    state: &ViewState,
    size_px: (u32, u32),
    (mode, governed): (Mode, Option<Tier>),
    overlay_revision: u64,
) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (
        scene.revision,
        scene.pose_revision,
        library.revision(),
        overlay_revision,
    )
        .hash(&mut h);
    for v in [
        camera.target.x,
        camera.target.y,
        camera.target.z,
        camera.yaw,
        camera.pitch,
        camera.distance,
        camera.fov_y,
    ] {
        v.to_bits().hash(&mut h);
    }
    (camera.projection as u8, size_px, mode as u8).hash(&mut h);
    governed.map(|t| t as u8 + 1).hash(&mut h);
    (&state.selection, state.hover, &state.hidden, &state.related).hash(&mut h);
    (state.display.edges, state.display.labels).hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod chip_tests {
    use super::*;

    #[test]
    fn a_chip_border_follows_selection_then_hover() {
        let look = crate::look::Look::default();
        let quiet = chip_border(false, false, &look, 1.0);
        let hover = chip_border(false, true, &look, 1.0);
        let selected = chip_border(true, true, &look, 1.0);
        assert_eq!(selected, color3(look.selection.edge, 255), "copper wins");
        assert_eq!(hover, color3(look.hover.edge, 255));
        assert_ne!(quiet, hover);
        assert_ne!(selected, hover);
    }
}

#[cfg(test)]
mod stats_tests {
    use super::*;

    /// The recorder is process-wide: tests that set it run one at a time.
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

    fn recorder(tier: Tier) -> Recorder {
        Recorder {
            adapter: "Test".into(),
            backend: "Vulkan".into(),
            tier,
            adapter_tier: tier,
            renders: 0,
            cpu_ms: VecDeque::new(),
            last: Default::default(),
            msaa: 1,
        }
    }

    #[test]
    fn percentiles_pick_the_sample_at_the_rank() {
        assert_eq!(percentile(&[], 0.5), 0.0);
        assert_eq!(percentile(&[5.0], 0.95), 5.0);
        let v: Vec<f32> = (1..=100).map(|i| i as f32).collect();
        assert_eq!(percentile(&v, 0.0), 1.0);
        assert_eq!(percentile(&v, 1.0), 100.0);
        assert_eq!(percentile(&v, 0.5), 51.0);
    }

    #[test]
    fn recording_fills_the_statistics_and_keeps_a_window() {
        let _guard = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        *STATS.lock().unwrap() = Some(recorder(Tier::Software));
        for i in 0..(STATS_WINDOW + 30) {
            record(
                crate::render::FrameStats {
                    cpu_ms: i as f32,
                    draw_calls: 11,
                    ..Default::default()
                },
                4,
                Tier::Integrated,
            );
        }
        let s = render_stats().unwrap();
        assert_eq!((s.tier, s.adapter_tier), (Tier::Integrated, Tier::Software));
        assert_eq!(s.renders as usize, STATS_WINDOW + 30);
        assert_eq!((s.draw_calls, s.msaa), (11, 4));
        // Only the last window counts: samples 30 to 149.
        assert!(s.cpu_p50_ms > 80.0 && s.cpu_p95_ms > 140.0, "{s:?}");
        *STATS.lock().unwrap() = None;
    }

    /// Runs `frames` egui frames `dt` seconds apart, calling the governor
    /// with `moving` each time; returns the tiers it reported.
    fn drive(
        view: &mut View3d,
        ctx: &egui::Context,
        frames: usize,
        dt: f64,
        moving: bool,
    ) -> Vec<Option<Tier>> {
        let mut out = Vec::new();
        for i in 0..frames {
            let input = egui::RawInput {
                time: Some(100.0 + i as f64 * dt),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |_| out.push(view.govern(ctx, moving)));
            output.textures_delta.clear();
        }
        out
    }

    #[test]
    fn slow_moving_frames_step_the_quality_down_and_a_still_view_does_not() {
        let _guard = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        *STATS.lock().unwrap() = Some(recorder(Tier::Discrete));
        let ctx = egui::Context::default();
        let mut view = View3d::new(7);
        // A view that is not moving repaints rarely: not slowness.
        let still = drive(&mut view, &ctx, 200, 0.2, false);
        assert!(still.iter().all(Option::is_none), "{still:?}");
        // 40 ms frames while moving: down to Integrated after two seconds,
        // down to Software after two more, then it stays.
        let slow = drive(&mut view, &ctx, 400, 0.04, true);
        assert_eq!(slow.first(), Some(&None), "starts on the adapter's tier");
        let seen: Vec<Tier> = slow.iter().flatten().copied().fold(Vec::new(), |mut v, t| {
            if v.last() != Some(&t) {
                v.push(t);
            }
            v
        });
        assert_eq!(seen, vec![Tier::Integrated, Tier::Software]);
        assert_eq!(slow.last(), Some(&Some(Tier::Software)));
        // It never steps up by itself, however fast the frames get.
        let fast = drive(&mut view, &ctx, 400, 0.008, true);
        assert_eq!(fast.last(), Some(&Some(Tier::Software)));
        // Resetting starts again from the adapter's tier.
        view.reset_quality();
        let again = drive(&mut view, &ctx, 3, 0.008, true);
        assert!(again.iter().all(Option::is_none), "{again:?}");
        *STATS.lock().unwrap() = None;
    }
}
