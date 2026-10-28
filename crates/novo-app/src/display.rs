//! Window and surface geometry on odd displays (KC-01, KC-04, KC-11..13).
//!
//! Windows can report a window size that no graphics surface can have: moving
//! the window between a mirrored portrait 4K monitor and a landscape one gave
//! a requested surface of 3378 x 32711 pixels, and wgpu panics above the
//! device limit. Two defences work together:
//!
//! * the graphics device logs a configure error instead of panicking
//!   ([`install_error_guard`], and a larger texture limit in `gpu.rs`);
//! * every frame [`track`] looks at the window, and a size that is too big for
//!   the surface, has an absurd aspect, or no longer fits after the window
//!   changed monitor or scale, is replaced by a sane one.
//!
//! The geometry is written to the log whenever it changes and to the problem
//! report, because the cause on a mirrored monitor is not proven yet.

use eframe::egui::{self, Context, Vec2, ViewportCommand};
use eframe::egui_wgpu::wgpu;

/// Largest surface side in pixels the window may ask for when the device
/// limit is not known yet; `gpu.rs` requests textures of this size.
pub const MAX_SURFACE_SIDE: f32 = 8192.0;

/// The device's real limit, set by [`install_error_guard`]: a weak GPU may
/// allow less than [`MAX_SURFACE_SIDE`].
static SURFACE_LIMIT: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(MAX_SURFACE_SIDE as u32);

fn surface_limit() -> f32 {
    SURFACE_LIMIT.load(std::sync::atomic::Ordering::Relaxed) as f32
}

/// The size a window gets when the reported one is unusable.
const DEFAULT_SIZE: Vec2 = Vec2::new(1280.0, 800.0);

/// Longest side divided by shortest side above which a size is not believed.
const MAX_ASPECT: f32 = 6.0;

/// What the window looked like in one frame, in points unless noted.
#[derive(Clone, Copy, Debug)]
pub struct Geometry {
    pub inner: Vec2,
    pub monitor: Option<Vec2>,
    pub pixels_per_point: f32,
}

impl Geometry {
    /// Bitwise, so a NaN from the system compares equal to itself and
    /// egui's asserting `Vec2` equality is never reached.
    fn bits(&self) -> [u32; 5] {
        let m = self.monitor.unwrap_or(Vec2::NAN);
        [
            self.inner.x.to_bits(),
            self.inner.y.to_bits(),
            m.x.to_bits(),
            m.y.to_bits(),
            self.pixels_per_point.to_bits(),
        ]
    }

    fn monitor_bits(&self) -> [u32; 3] {
        let b = self.bits();
        [b[2], b[3], b[4]]
    }
}

/// The size to put the window at instead of `g.inner`, or `None` when the
/// window is usable. `monitor_changed` is true on the first frame and when
/// the monitor or the scale changed: then a window larger than the screen is
/// also made to fit (the old rule of `fit_window_to_screen`).
pub fn corrected_size(g: Geometry, monitor_changed: bool) -> Option<Vec2> {
    let ppp = if g.pixels_per_point.is_finite() && g.pixels_per_point > 0.0 {
        g.pixels_per_point
    } else {
        1.0
    };
    let min = Vec2::from(crate::MIN_WINDOW);
    let mut size = g.inner;
    let believable = size.x.is_finite()
        && size.y.is_finite()
        && size.min_elem() > 0.0
        && size.max_elem() / size.min_elem() <= MAX_ASPECT;
    if !believable {
        size = DEFAULT_SIZE;
    }
    let cap = Vec2::splat(surface_limit() / ppp);
    size = size.min(cap);
    if monitor_changed
        && let Some(monitor) = g.monitor
        && monitor.min_elem() > 0.0
    {
        // The taskbar and the title bar need room, as on a small screen.
        let room = (monitor - Vec2::new(0.0, 80.0)).max(min);
        size = size.min(room);
    }
    let size = size.max(min);
    // Component-wise: egui's `Vec2` equality asserts on NaN.
    (size.x != g.inner.x || size.y != g.inner.y).then_some(size)
}

/// Called every frame from `NovoApp::ui`.
pub fn track(ctx: &Context) {
    let (inner, monitor, maximized, fullscreen) = ctx.input(|i| {
        let v = i.viewport();
        (
            v.inner_rect.map(|r| r.size()),
            v.monitor_size,
            v.maximized,
            v.fullscreen,
        )
    });
    let Some(inner) = inner else { return };
    let geometry = Geometry {
        inner,
        monitor,
        pixels_per_point: ctx.pixels_per_point(),
    };
    let id = egui::Id::new("display_geometry_seen");
    let previous: Option<Geometry> = ctx.data(|d| d.get_temp(id));
    if previous.is_some_and(|p| p.bits() == geometry.bits()) {
        return;
    }
    let monitor_changed = previous.is_none_or(|p| p.monitor_bits() != geometry.monitor_bits());
    ctx.data_mut(|d| d.insert_temp(id, geometry));
    if monitor_changed {
        let line = describe(&geometry);
        log::info!("Display: {line}");
        crate::diag::set_display(line);
    }
    // A window the user maximized fills the screen on purpose.
    let user_sized = maximized == Some(true) || fullscreen == Some(true);
    if let Some(size) = corrected_size(geometry, monitor_changed && !user_sized) {
        log::warn!(
            "Window size {:.0} x {:.0} points is not usable here; set to {:.0} x {:.0}",
            inner.x,
            inner.y,
            size.x,
            size.y
        );
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(size));
    }
    if monitor_changed {
        // Repaint at once so the surface and the pointer map agree again.
        ctx.request_repaint();
    }
}

/// One line for the log and the problem report.
pub fn describe(g: &Geometry) -> String {
    let px = g.inner * g.pixels_per_point;
    let monitor = g.monitor.map_or_else(
        || "unknown".to_owned(),
        |m| format!("{:.0} x {:.0} points", m.x, m.y),
    );
    format!(
        "window {:.0} x {:.0} points = {:.0} x {:.0} pixels at scale {:.2}, monitor {monitor}",
        g.inner.x, g.inner.y, px.x, px.y, g.pixels_per_point
    )
}

/// Makes a graphics error (for example a surface the driver refuses) a log
/// line instead of a panic. [`track`] then repairs the window size.
pub fn install_error_guard(device: &wgpu::Device) {
    let limit = device.limits().max_texture_dimension_2d.max(512);
    SURFACE_LIMIT.store(
        limit.min(MAX_SURFACE_SIDE as u32),
        std::sync::atomic::Ordering::Relaxed,
    );
    device.on_uncaptured_error(std::sync::Arc::new(|error: wgpu::Error| {
        log::error!("Graphics error, continuing: {error}");
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geo(w: f32, h: f32, ppp: f32, monitor: Option<(f32, f32)>) -> Geometry {
        Geometry {
            inner: Vec2::new(w, h),
            monitor: monitor.map(|(x, y)| Vec2::new(x, y)),
            pixels_per_point: ppp,
        }
    }

    #[test]
    fn the_crash_size_from_the_mirrored_monitor_is_replaced() {
        // KC-01: the surface was asked for 3378 x 32711 pixels at scale 1.5.
        let g = geo(3378.0 / 1.5, 32711.0 / 1.5, 1.5, Some((1440.0, 2560.0)));
        let fixed = corrected_size(g, true).expect("must be corrected");
        assert!(fixed.x * 1.5 <= MAX_SURFACE_SIDE && fixed.y * 1.5 <= MAX_SURFACE_SIDE);
        assert!(fixed.max_elem() / fixed.min_elem() <= MAX_ASPECT);
    }

    #[test]
    fn a_huge_but_square_window_is_capped_to_the_surface_limit() {
        let g = geo(9000.0, 9000.0, 1.0, None);
        assert_eq!(
            corrected_size(g, false),
            Some(Vec2::splat(MAX_SURFACE_SIDE))
        );
    }

    #[test]
    fn a_normal_window_is_left_alone_even_after_a_monitor_change() {
        let g = geo(1280.0, 800.0, 1.5, Some((2560.0, 1440.0)));
        assert_eq!(corrected_size(g, true), None);
        assert_eq!(corrected_size(g, false), None);
    }

    #[test]
    fn a_window_wider_than_the_portrait_screen_fits_after_a_move() {
        // KC-12: 1280 points wide on a 1080-point wide portrait monitor.
        let g = geo(1280.0, 800.0, 2.0, Some((1080.0, 1920.0)));
        let fixed = corrected_size(g, true).unwrap();
        assert_eq!(fixed.x, 1080.0);
        assert_eq!(fixed.y, 800.0);
        // Without a monitor change the user's size is respected.
        assert_eq!(corrected_size(g, false), None);
    }

    #[test]
    fn nonsense_numbers_give_the_default_size() {
        for bad in [f32::NAN, f32::INFINITY, 0.0, -5.0] {
            let g = geo(bad, 800.0, 1.0, None);
            assert_eq!(corrected_size(g, false), Some(DEFAULT_SIZE), "{bad}");
        }
        let g = geo(1280.0, 800.0, f32::NAN, None);
        assert_eq!(corrected_size(g, false), None);
    }

    #[test]
    fn the_window_never_goes_below_the_minimum() {
        let g = geo(700.0, 330.0, 1.0, Some((600.0, 400.0)));
        assert_eq!(corrected_size(g, true), Some(Vec2::new(680.0, 320.0)));
    }

    #[test]
    fn a_window_taller_than_a_small_screen_shrinks_to_it() {
        let g = geo(1280.0, 800.0, 1.0, Some((1366.0, 768.0)));
        assert_eq!(corrected_size(g, true), Some(Vec2::new(1280.0, 688.0)));
    }

    #[test]
    fn a_1366_by_768_laptop_at_every_scale_keeps_the_window_on_the_screen() {
        for scale in [1.0_f32, 1.25, 1.5, 2.0] {
            let monitor = Vec2::new(1366.0, 768.0) / scale;
            let g = geo(1280.0, 800.0, scale, Some((monitor.x, monitor.y)));
            let got = corrected_size(g, true).unwrap();
            assert!(got.x <= monitor.x && got.y <= monitor.y, "{scale}: {got:?}");
        }
    }

    #[test]
    fn the_description_names_pixels_and_scale() {
        let text = describe(&geo(1280.0, 800.0, 1.5, Some((2560.0, 1440.0))));
        assert!(text.contains("1920 x 1200 pixels at scale 1.50"), "{text}");
    }
}
