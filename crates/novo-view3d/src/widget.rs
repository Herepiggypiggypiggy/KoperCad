//! The egui widget: a 3D view with orbit, pan, zoom, picking and a marquee.
//!
//! The host calls [`install`] once at start-up, keeps one [`View3d`] per
//! visible 3D face, and calls [`View3d::show`] every frame. Nothing is
//! rendered unless the scene, camera, selection or size changed, and after the
//! camera stops the image is refined once (4x MSAA, full scale).

use crate::camera::{Camera, ViewPreset, ease_out};
use crate::look::{Glyph, Look};
use crate::mesh::MeshLibrary;
use crate::pick::{self, Hit};
use crate::render::{BlitBind, Blitter, Frame, Mode, Quality, Renderer, Tier, ViewTarget};
use crate::scene::{ItemId, Mark, Scene};
use crate::view_state::ViewState;
use egui::{
    Align2, Color32, FontId, Key, Modifiers, PointerButton, Pos2, Rect, Sense, Stroke, Vec2,
};
use glam::Vec3;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// GPU state of a view not painted for this long is freed.
const IDLE_VIEW_TTL: Duration = Duration::from_secs(30);
/// Seconds without input before the settled (sharp) frame is drawn.
const SETTLE_AFTER: f64 = 0.25;
const ANIM_SECONDS: f64 = 0.25;
static INSTALLED: AtomicBool = AtomicBool::new(false);

/// Registers the renderer with egui's wgpu renderer. Call once at start-up.
/// `msaa_samples` is eframe's `NativeOptions::multisampling` (0 or 1 = off).
pub fn install(render_state: &egui_wgpu::RenderState, msaa_samples: u32) {
    let renderer = Renderer::new(&render_state.device);
    let blitter = Blitter::new(
        &render_state.device,
        render_state.target_format,
        msaa_samples.max(1),
    );
    let tier = Tier::of(&render_state.adapter.get_info());
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
            renderer.render(
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
                    quality: Quality::new(self.mode, *tier),
                },
            );
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
    pub camera_changed: bool,
}

struct Anim {
    from: Camera,
    to: Camera,
    start: f64,
}

/// One 3D view: camera, user state and input handling.
pub struct View3d {
    id: u64,
    pub camera: Camera,
    pub state: ViewState,
    anim: Option<Anim>,
    needs_fit: bool,
    last_activity: f64,
    marquee_from: Option<Pos2>,
    /// Where the last Alt + click landed and how many times in a row.
    alt_cycle: Option<(Pos2, usize)>,
    last_aspect: f32,
}

impl View3d {
    /// `id` must be unique among the views of the app.
    pub fn new(id: u64) -> Self {
        Self {
            id,
            camera: Camera::default(),
            state: ViewState::default(),
            anim: None,
            needs_fit: true,
            last_activity: f64::NEG_INFINITY,
            marquee_from: None,
            alt_cycle: None,
            last_aspect: 1.0,
        }
    }

    /// Frames the whole scene on the next frame.
    pub fn request_fit(&mut self) {
        self.needs_fit = true;
    }

    /// Eases to a view cube face from where the camera is (`view.iso` and the cube).
    pub fn go_to(&mut self, preset: ViewPreset, scene: &Scene, now: f64, reduced: bool) {
        let mut to = self.camera;
        to.set_preset(preset);
        to.fit(&scene.bounds(), self.last_aspect);
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
        if reduced {
            self.camera = to;
            self.anim = None;
        } else {
            self.anim = Some(Anim {
                from: self.camera,
                to,
                start: now,
            });
        }
        self.last_activity = now;
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
        let aspect = rect.width() / rect.height().max(1.0);
        self.last_aspect = aspect;
        let before = self.camera;
        let bounds = scene.bounds();

        if self.needs_fit && !bounds.is_empty() {
            self.camera.fit(&bounds, aspect);
            self.needs_fit = false;
            self.anim = None;
            self.last_activity = now;
        }

        // Camera animation.
        if let Some(a) = &self.anim {
            let t = ((now - a.start) / ANIM_SECONDS) as f32;
            self.camera = Camera::lerp(&a.from, &a.to, ease_out(t));
            if t >= 1.0 {
                self.camera = a.to;
                self.anim = None;
            } else {
                ctx.request_repaint();
            }
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

        // Orbit and pan.
        let drag = response.drag_delta();
        if drag != Vec2::ZERO {
            let orbit = response.dragged_by(PointerButton::Secondary)
                || (response.dragged_by(PointerButton::Primary) && alt && !space);
            let pan = response.dragged_by(PointerButton::Middle)
                || (response.dragged_by(PointerButton::Primary) && space);
            if orbit {
                self.anim = None;
                self.camera.orbit(-drag.x * 0.008, drag.y * 0.008);
            } else if pan {
                self.anim = None;
                self.camera.pan(drag.x, drag.y, rect.height());
            }
        }

        // Zoom towards the pointer.
        if response.hovered() && (scroll != 0.0 || zoom != 1.0) {
            let factor = (-scroll * 0.0012).exp() / zoom;
            let pointer = hover_pos.unwrap_or(rect.center());
            let anchor = self.pick_at(rect, scene, pointer).map(|h| h.point);
            self.anim = None;
            self.camera
                .zoom_at(factor, Self::ndc(rect, pointer), aspect, anchor);
        }

        // Marquee: a plain left drag.
        let plain_left = response.dragged_by(PointerButton::Primary) && !alt && !space;
        if response.drag_started_by(PointerButton::Primary) && !alt && !space {
            self.marquee_from = ctx.input(|i| i.pointer.press_origin());
        }
        if let (true, Some(from), Some(to)) = (plain_left, self.marquee_from, hover_pos) {
            let band = Rect::from_two_pos(from, to);
            ui.painter().rect(
                band,
                0.0,
                Color32::from_rgba_unmultiplied(255, 168, 92, 28),
                Stroke::new(1.0, Color32::from_rgb(255, 168, 92)),
                egui::StrokeKind::Inside,
            );
        }
        if response.drag_stopped_by(PointerButton::Primary)
            && let (Some(from), Some(to)) = (self.marquee_from.take(), hover_pos)
            && (to - from).length() > 4.0
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
        let hovering = response.hovered() && !response.dragged();
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
        if response.double_clicked_by(PointerButton::Primary)
            && let Some(p) = response.interact_pointer_pos()
            && let Some(hit) = self.pick_at(rect, scene, p)
        {
            out.double_clicked = Some(hit.id);
            if let Some(item) = scene.get(hit.id) {
                let mut to = self.camera;
                to.fit(&scene.world_bounds(item), aspect);
                self.animate_to(to, now, options.reduced_motion);
            }
        }
        if response.secondary_clicked()
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
                self.fit(scene, now, options.reduced_motion);
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

        let ppp = ctx.pixels_per_point();
        let size_px = (
            (rect.width() * ppp).round().max(1.0) as u32,
            (rect.height() * ppp).round().max(1.0) as u32,
        );
        let key = frame_key(scene, library, &self.camera, &self.state, size_px, mode);
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
                key,
            },
        ));

        self.paint_overlay(ui, rect, scene, look);
        out
    }

    /// Tag labels and mark glyphs over the image.
    fn paint_overlay(&self, ui: &egui::Ui, rect: Rect, scene: &Scene, look: &Look) {
        let painter = ui.painter_at(rect);
        let aspect = rect.width() / rect.height().max(1.0);
        let bounds = scene.bounds();
        let vp = self.camera.view_proj(aspect, &bounds);
        let eye = self.camera.eye();
        let mut frustum_ok = Vec::new();
        crate::cull::visible(scene, &self.state, &vp, &mut frustum_ok);
        for i in frustum_ok {
            let item = &scene.items[i];
            let world = scene.world_matrix(item);
            let front = world.transform_point3(Vec3::new(0.0, 0.0, item.size.z));
            let dist = (front - eye).length();
            // Size of the item's narrower face edge on screen, in points.
            let size_px = crate::cull::screen_size_px(
                &self.camera,
                item.size.x.min(item.size.y) * 0.5,
                dist,
                rect.height() * ui.ctx().pixels_per_point(),
            ) / ui.ctx().pixels_per_point();
            if let (true, Some(label), true) =
                (self.state.display.labels, &item.label, size_px >= 24.0)
                && let Some(p) = self.project(rect, scene, front)
            {
                painter.text(
                    p + Vec2::new(0.0, 1.0),
                    Align2::CENTER_CENTER,
                    label.as_ref(),
                    FontId::proportional(11.0),
                    Color32::from_black_alpha(200),
                );
                painter.text(
                    p,
                    Align2::CENTER_CENTER,
                    label.as_ref(),
                    FontId::proportional(11.0),
                    Color32::WHITE,
                );
            }
            if item.mark != Mark::None && size_px >= 12.0 {
                let style = look.mark(item.mark);
                let text = match style.glyph {
                    Glyph::Cross => "\u{00d7}",
                    Glyph::Bang => "!",
                    Glyph::DashedEdge => "?",
                    Glyph::None => continue,
                };
                let top = world.transform_point3(Vec3::new(0.0, item.size.y * 0.5, item.size.z));
                if let Some(p) = self.project(rect, scene, top) {
                    let c = style.edge.unwrap_or([255, 255, 255]);
                    let col = Color32::from_rgb(c[0], c[1], c[2]);
                    painter.circle(
                        p,
                        7.0,
                        Color32::from_black_alpha(200),
                        Stroke::new(1.5, col),
                    );
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
    }
}

/// Everything that changes the picture, hashed: when it is the same as the
/// last frame the renderer does nothing.
fn frame_key(
    scene: &Scene,
    library: &MeshLibrary,
    camera: &Camera,
    state: &ViewState,
    size_px: (u32, u32),
    mode: Mode,
) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (scene.revision, scene.pose_revision, library.revision()).hash(&mut h);
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
    (&state.selection, state.hover, &state.hidden, &state.related).hash(&mut h);
    (state.display.edges, state.display.labels).hash(&mut h);
    h.finish()
}
