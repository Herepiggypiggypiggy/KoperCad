//! Turntable camera (3d-look.md section 6): orbit, pan, zoom to cursor,
//! fit, standard and saved views, eased transitions and inertia.
//!
//! Z is up, millimetres. Yaw 0 looks at the plate from the front (eye on
//! the -Y side); positive yaw moves the eye towards +X. Pitch 0 is level,
//! positive looks down. The basis is built from yaw and pitch directly, so
//! the top and bottom views (pitch of plus or minus 90 degrees) have no
//! degenerate up vector.

pub type V3 = [f64; 3];

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn scale(a: V3, s: f64) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn len(a: V3) -> f64 {
    dot(a, a).sqrt()
}

/// Feel constants (section 6).
pub const ORBIT_DEG_PER_PX: f64 = 0.25;
pub const ZOOM_PER_NOTCH: f64 = 0.12;
pub const MIN_DISTANCE_MM: f64 = 20.0;
pub const MAX_ENCLOSURES: f64 = 10.0;
pub const PITCH_LIMIT_DEG: f64 = 89.0;
pub const TRANSITION_SECS: f64 = 0.22;
pub const INERTIA_HALF_LIFE_SECS: f64 = 0.09;
pub const FIT_MARGIN: f64 = 1.08;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Projection {
    /// Vertical field of view in degrees (default 30).
    Perspective {
        fov_deg: f64,
    },
    Ortho,
}

impl Default for Projection {
    fn default() -> Self {
        Self::Perspective { fov_deg: 30.0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub target: V3,
    /// Degrees.
    pub yaw: f64,
    /// Degrees, positive looks down.
    pub pitch: f64,
    /// Distance from the eye to the target, mm. In orthographic mode it
    /// sets the visible height the same way a perspective camera would.
    pub distance: f64,
    pub projection: Projection,
    /// Largest distance allowed, normally 10 times the enclosure diagonal.
    pub max_distance: f64,
}

impl Camera {
    /// Front view fitted to a plate of the given size, centred on it.
    pub fn home(center: V3, extent_mm: f64) -> Self {
        let mut c = Self {
            target: center,
            yaw: 0.0,
            pitch: 0.0,
            distance: 1.0,
            projection: Projection::default(),
            max_distance: extent_mm * MAX_ENCLOSURES,
        };
        c.fit_sphere(center, extent_mm / 2.0);
        c
    }

    fn eye_dir(&self) -> V3 {
        let (y, p) = (self.yaw.to_radians(), self.pitch.to_radians());
        [y.sin() * p.cos(), -y.cos() * p.cos(), p.sin()]
    }

    pub fn eye(&self) -> V3 {
        add(self.target, scale(self.eye_dir(), self.distance))
    }

    pub fn forward(&self) -> V3 {
        scale(self.eye_dir(), -1.0)
    }

    pub fn right(&self) -> V3 {
        let y = self.yaw.to_radians();
        [y.cos(), y.sin(), 0.0]
    }

    pub fn up(&self) -> V3 {
        cross(self.right(), self.forward())
    }

    fn fov_rad(&self) -> f64 {
        match self.projection {
            Projection::Perspective { fov_deg } => fov_deg.to_radians(),
            Projection::Ortho => 30f64.to_radians(),
        }
    }

    /// Visible height at the target depth, mm.
    pub fn visible_height(&self) -> f64 {
        2.0 * self.distance * (self.fov_rad() / 2.0).tan()
    }

    fn clamp(&mut self, limit_pitch: bool) {
        if limit_pitch {
            self.pitch = self.pitch.clamp(-PITCH_LIMIT_DEG, PITCH_LIMIT_DEG);
        }
        self.yaw = wrap_deg(self.yaw);
        self.distance = self
            .distance
            .clamp(MIN_DISTANCE_MM, self.max_distance.max(MIN_DISTANCE_MM));
    }

    /// Turntable orbit by a pointer delta in pixels. Dragging right moves
    /// the scene right (the eye goes left).
    pub fn orbit(&mut self, dx_px: f64, dy_px: f64) {
        self.yaw -= dx_px * ORBIT_DEG_PER_PX;
        self.pitch += dy_px * ORBIT_DEG_PER_PX;
        self.clamp(true);
    }

    /// Pan by a pointer delta, one screen pixel per pixel at the target depth.
    pub fn pan(&mut self, dx_px: f64, dy_px: f64, viewport_h_px: f64) {
        let mm_per_px = self.visible_height() / viewport_h_px.max(1.0);
        let d = add(
            scale(self.right(), -dx_px * mm_per_px),
            scale(self.up(), dy_px * mm_per_px),
        );
        self.target = add(self.target, d);
    }

    /// Ray through a point in normalised device coordinates (x right, y up,
    /// both -1 to 1). Returns origin and unit direction.
    pub fn ray(&self, ndc: [f64; 2], aspect: f64) -> (V3, V3) {
        let half_h = self.visible_height() / 2.0;
        let (rx, ry) = (ndc[0] * half_h * aspect, ndc[1] * half_h);
        let (r, u, f) = (self.right(), self.up(), self.forward());
        match self.projection {
            Projection::Ortho => (add(self.eye(), add(scale(r, rx), scale(u, ry))), f),
            Projection::Perspective { .. } => {
                let dir = add(
                    f,
                    add(scale(r, rx / self.distance), scale(u, ry / self.distance)),
                );
                (self.eye(), scale(dir, 1.0 / len(dir)))
            }
        }
    }

    /// Where a world point lands in normalised device coordinates, and its
    /// depth along the view direction. `None` behind a perspective eye.
    pub fn project(&self, p: V3, aspect: f64) -> Option<([f64; 2], f64)> {
        let rel = sub(p, self.eye());
        let depth = dot(rel, self.forward());
        let half_h = self.visible_height() / 2.0;
        let (x, y) = (dot(rel, self.right()), dot(rel, self.up()));
        match self.projection {
            Projection::Ortho => Some(([x / (half_h * aspect), y / half_h], depth)),
            Projection::Perspective { .. } => {
                if depth <= 1e-9 {
                    return None;
                }
                let s = self.distance / depth;
                Some(([x * s / (half_h * aspect), y * s / half_h], depth))
            }
        }
    }

    /// Zoom by `notches` (positive zooms in) keeping the world point under
    /// the cursor fixed. `pick` is the point from the last frame's depth
    /// sample; without one the plate plane `z = plane_z` is used, and failing
    /// that the target.
    pub fn zoom_at(
        &mut self,
        notches: f64,
        cursor_ndc: [f64; 2],
        aspect: f64,
        pick: Option<V3>,
        plane_z: f64,
    ) {
        let anchor = pick
            .or_else(|| {
                let (o, d) = self.ray(cursor_ndc, aspect);
                (d[2].abs() > 1e-9)
                    .then(|| (plane_z - o[2]) / d[2])
                    .filter(|t| *t > 0.0)
                    .map(|t| add(o, scale(d, t)))
            })
            .unwrap_or(self.target);
        let depth = dot(sub(anchor, self.eye()), self.forward());
        self.distance *= (1.0 - ZOOM_PER_NOTCH).powf(notches);
        self.clamp(false);
        // Slide the target so the anchor projects to the same pixel again.
        let (o, d) = self.ray(cursor_ndc, aspect);
        let t = match self.projection {
            Projection::Ortho => depth,
            Projection::Perspective { .. } => depth / dot(d, self.forward()),
        };
        let now = add(o, scale(d, t));
        self.target = add(self.target, sub(anchor, now));
    }

    /// Centre on a sphere and stand back far enough to see all of it, with
    /// the 8% margin. Direction is unchanged.
    pub fn fit_sphere(&mut self, center: V3, radius: f64) {
        self.target = center;
        let half = (self.fov_rad() / 2.0).sin().max(1e-3);
        self.distance = radius * FIT_MARGIN / half;
        self.clamp(false);
    }

    /// Look from a direction (from the target to the eye), keeping distance.
    pub fn look_from(&mut self, dir: V3) {
        let n = len(dir);
        if n < 1e-9 {
            return;
        }
        let d = scale(dir, 1.0 / n);
        self.pitch = d[2].clamp(-1.0, 1.0).asin().to_degrees();
        // Straight above or below: keep the current yaw.
        if d[0].hypot(d[1]) > 1e-6 {
            self.yaw = d[0].atan2(-d[1]).to_degrees();
        }
        self.clamp(false);
    }

    /// Enter a standard view; orthographic for the axis aligned ones.
    pub fn set_standard(&mut self, v: StandardView) {
        let (yaw, pitch, ortho) = v.angles();
        self.yaw = yaw;
        self.pitch = pitch;
        if ortho {
            self.projection = Projection::Ortho;
        } else if self.projection == Projection::Ortho {
            self.projection = Projection::default();
        }
    }

    /// First orbit after an automatic orthographic view returns to perspective.
    pub fn leave_auto_ortho(&mut self) {
        if self.projection == Projection::Ortho {
            self.projection = Projection::default();
        }
    }
}

fn wrap_deg(a: f64) -> f64 {
    let mut a = a % 360.0;
    if a > 180.0 {
        a -= 360.0;
    } else if a <= -180.0 {
        a += 360.0;
    }
    a
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StandardView {
    Front,
    Back,
    Left,
    Right,
    Top,
    Bottom,
    Isometric,
}

impl StandardView {
    /// Yaw, pitch and whether the view is orthographic.
    fn angles(self) -> (f64, f64, bool) {
        match self {
            Self::Front => (0.0, 0.0, true),
            Self::Back => (180.0, 0.0, true),
            Self::Right => (90.0, 0.0, true),
            Self::Left => (-90.0, 0.0, true),
            Self::Top => (0.0, 90.0, true),
            Self::Bottom => (0.0, -90.0, true),
            Self::Isometric => (35.0, 30.0, false),
        }
    }
}

/// A named camera stored with the project (eye, target, projection and the
/// cutaway flag).
#[derive(Clone, Debug, PartialEq)]
pub struct SavedView {
    pub name: String,
    pub camera: Camera,
    pub cutaway: bool,
}

/// An eased move between two cameras (220 ms, ease out cubic). With reduced
/// motion it completes at once.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition {
    from: Camera,
    to: Camera,
    t: f64,
    secs: f64,
}

pub fn ease_out_cubic(t: f64) -> f64 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

impl Transition {
    pub fn new(from: Camera, to: Camera, reduced_motion: bool) -> Self {
        Self {
            from,
            to,
            t: 0.0,
            secs: if reduced_motion { 0.0 } else { TRANSITION_SECS },
        }
    }

    pub fn done(&self) -> bool {
        self.t >= 1.0 || self.secs <= 0.0
    }

    /// Advance by `dt` seconds and return the camera to show.
    pub fn step(&mut self, dt: f64) -> Camera {
        if self.secs <= 0.0 {
            self.t = 1.0;
            return self.to;
        }
        self.t = (self.t + dt / self.secs).min(1.0);
        let k = ease_out_cubic(self.t);
        let lerp = |a: f64, b: f64| a + (b - a) * k;
        // Shortest arc for yaw.
        let dyaw = wrap_deg(self.to.yaw - self.from.yaw);
        let mut c = self.to;
        c.yaw = wrap_deg(self.from.yaw + dyaw * k);
        c.pitch = lerp(self.from.pitch, self.to.pitch);
        c.distance = lerp(self.from.distance, self.to.distance);
        c.target = [
            lerp(self.from.target[0], self.to.target[0]),
            lerp(self.from.target[1], self.to.target[1]),
            lerp(self.from.target[2], self.to.target[2]),
        ];
        // Projection switches at the end, so there is no mid-flight jump.
        c.projection = if self.t >= 1.0 {
            self.to.projection
        } else {
            self.from.projection
        };
        c
    }
}

/// Orbit momentum after a drag release: 90 ms half life, stopped by a click.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Inertia {
    /// Pixels per second.
    pub vx: f64,
    pub vy: f64,
}

impl Inertia {
    const STOP_PX_PER_SEC: f64 = 8.0;

    pub fn release(vx: f64, vy: f64, reduced_motion: bool) -> Self {
        if reduced_motion {
            Self::default()
        } else {
            Self { vx, vy }
        }
    }

    pub fn active(&self) -> bool {
        self.vx.hypot(self.vy) > Self::STOP_PX_PER_SEC
    }

    pub fn stop(&mut self) {
        *self = Self::default();
    }

    /// Apply `dt` seconds of momentum to the camera.
    pub fn step(&mut self, camera: &mut Camera, dt: f64) {
        if !self.active() {
            self.stop();
            return;
        }
        camera.orbit(self.vx * dt, self.vy * dt);
        let k = 0.5f64.powf(dt / INERTIA_HALF_LIFE_SECS);
        self.vx *= k;
        self.vy *= k;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ASPECT: f64 = 16.0 / 9.0;

    fn cam() -> Camera {
        Camera::home([300.0, 0.0, 400.0], 800.0)
    }

    fn near(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn front_view_looks_along_plus_y_with_z_up() {
        let c = cam();
        assert!(near(c.forward()[1], 1.0, 1e-9));
        assert!(near(c.right()[0], 1.0, 1e-9));
        assert!(near(c.up()[2], 1.0, 1e-9));
        assert!(c.eye()[1] < c.target[1]);
    }

    #[test]
    fn right_view_puts_the_eye_on_plus_x() {
        let mut c = cam();
        c.set_standard(StandardView::Right);
        assert!(c.eye()[0] > c.target[0]);
        assert!(near(c.up()[2], 1.0, 1e-9));
    }

    #[test]
    fn top_view_has_a_valid_basis() {
        let mut c = cam();
        c.set_standard(StandardView::Top);
        assert!(near(c.forward()[2], -1.0, 1e-9));
        assert!(near(len(c.up()), 1.0, 1e-9));
        // Up on screen is the plate's +Y (away from the front).
        assert!(near(c.up()[1], 1.0, 1e-9));
    }

    #[test]
    fn standard_views_are_orthographic_and_iso_is_not() {
        let mut c = cam();
        c.set_standard(StandardView::Front);
        assert_eq!(c.projection, Projection::Ortho);
        c.set_standard(StandardView::Isometric);
        assert!(matches!(c.projection, Projection::Perspective { .. }));
    }

    #[test]
    fn orbit_clamps_pitch_and_wraps_yaw() {
        let mut c = cam();
        c.orbit(0.0, 100_000.0);
        assert!(near(c.pitch, 89.0, 1e-9));
        c.orbit(0.0, -1_000_000.0);
        assert!(near(c.pitch, -89.0, 1e-9));
        c.orbit(-4.0 * 360.0 / ORBIT_DEG_PER_PX + 4.0, 0.0);
        assert!(c.yaw > -180.0 && c.yaw <= 180.0);
    }

    #[test]
    fn orbit_keeps_the_target_and_distance() {
        let mut c = cam();
        let (t, d) = (c.target, c.distance);
        c.orbit(120.0, -40.0);
        assert_eq!(c.target, t);
        assert!(near(c.distance, d, 1e-9));
        assert!(near(len(sub(c.eye(), c.target)), d, 1e-6));
    }

    #[test]
    fn pan_moves_one_pixel_per_pixel_at_the_target_depth() {
        let mut c = cam();
        let before = c.project(c.target, ASPECT).unwrap().0;
        let target0 = c.target;
        c.pan(100.0, 0.0, 1080.0);
        // The old target point moved right on screen by 100 px of 1080.
        let after = c.project(target0, ASPECT).unwrap().0;
        let px = (after[1] - before[1]).abs();
        assert!(near(px, 0.0, 1e-9));
        let half_h_px = 540.0;
        let half_w = ASPECT;
        assert!(near(
            (after[0] - before[0]) * half_h_px * half_w,
            100.0,
            1e-6
        ));
    }

    fn zoom_keeps_point(projection: Projection) {
        let mut c = cam();
        c.projection = projection;
        c.orbit(60.0, 30.0);
        let cursor = [0.4, -0.25];
        let (o, d) = c.ray(cursor, ASPECT);
        let anchor = add(o, scale(d, c.distance * 0.9));
        let before = c.project(anchor, ASPECT).unwrap().0;
        c.zoom_at(3.0, cursor, ASPECT, Some(anchor), 0.0);
        let after = c.project(anchor, ASPECT).unwrap().0;
        assert!(near(before[0], after[0], 1e-9), "{before:?} {after:?}");
        assert!(near(before[1], after[1], 1e-9));
    }

    #[test]
    fn zoom_to_cursor_keeps_the_point_fixed_in_perspective() {
        zoom_keeps_point(Projection::default());
    }

    #[test]
    fn zoom_to_cursor_keeps_the_point_fixed_in_ortho() {
        zoom_keeps_point(Projection::Ortho);
    }

    #[test]
    fn zoom_falls_back_to_the_plate_plane() {
        let mut c = cam();
        c.orbit(40.0, 35.0);
        let cursor = [-0.3, 0.2];
        let (o, d) = c.ray(cursor, ASPECT);
        let t = (0.0 - o[2]) / d[2];
        let hit = add(o, scale(d, t));
        let before = c.project(hit, ASPECT).unwrap().0;
        c.zoom_at(2.0, cursor, ASPECT, None, 0.0);
        let after = c.project(hit, ASPECT).unwrap().0;
        assert!(near(before[0], after[0], 1e-9) && near(before[1], after[1], 1e-9));
    }

    #[test]
    fn zoom_clamps_distance() {
        let mut c = cam();
        c.zoom_at(500.0, [0.0, 0.0], ASPECT, None, 0.0);
        assert!(near(c.distance, MIN_DISTANCE_MM, 1e-9));
        c.zoom_at(-5000.0, [0.0, 0.0], ASPECT, None, 0.0);
        assert!(c.distance <= c.max_distance + 1e-6);
    }

    #[test]
    fn zoom_in_by_a_notch_is_twelve_percent_closer() {
        let mut c = cam();
        let d = c.distance;
        c.zoom_at(1.0, [0.0, 0.0], ASPECT, Some(c.target), 0.0);
        assert!(near(c.distance, d * 0.88, 1e-9));
    }

    #[test]
    fn fit_sphere_contains_the_sphere_with_margin() {
        let mut c = cam();
        c.orbit(30.0, 20.0);
        c.fit_sphere([100.0, 50.0, 200.0], 500.0);
        let top = add(c.target, scale(c.up(), 500.0));
        let ndc = c.project(top, ASPECT).unwrap().0;
        assert!(ndc[1] < 1.0 && ndc[1] > 0.85, "{ndc:?}");
    }

    #[test]
    fn look_from_sets_yaw_and_pitch_and_survives_straight_down() {
        let mut c = cam();
        c.look_from([1.0, 0.0, 0.0]);
        assert!(near(c.yaw, 90.0, 1e-9) && near(c.pitch, 0.0, 1e-9));
        c.look_from([0.0, 0.0, 1.0]);
        assert!(near(c.pitch, 90.0, 1e-9));
        assert!(
            near(c.yaw, 90.0, 1e-9),
            "keeps yaw when looking straight down"
        );
    }

    #[test]
    fn transition_takes_the_short_way_round_in_yaw() {
        let mut a = cam();
        a.yaw = 170.0;
        let mut b = a;
        b.yaw = -170.0;
        let mut t = Transition::new(a, b, false);
        let mid = t.step(TRANSITION_SECS / 2.0);
        // Halfway by time is past halfway by ease out, but still near 180.
        assert!(mid.yaw.abs() > 170.0, "{}", mid.yaw);
        let end = t.step(1.0);
        assert!(t.done());
        assert!(near(end.yaw, -170.0, 1e-9));
    }

    #[test]
    fn transition_is_monotonic_and_ends_exactly() {
        let a = cam();
        let mut b = a;
        b.set_standard(StandardView::Top);
        b.distance = 2500.0;
        let mut t = Transition::new(a, b, false);
        let mut last = a.distance;
        for _ in 0..30 {
            let c = t.step(0.01);
            assert!(c.distance >= last - 1e-9);
            last = c.distance;
        }
        assert_eq!(t.step(0.0), b);
    }

    #[test]
    fn reduced_motion_jumps() {
        let a = cam();
        let mut b = a;
        b.yaw = 90.0;
        let mut t = Transition::new(a, b, true);
        assert!(t.done());
        assert_eq!(t.step(0.0), b);
        let mut i = Inertia::release(500.0, 0.0, true);
        assert!(!i.active());
        i.stop();
    }

    #[test]
    fn inertia_halves_every_90_ms_and_stops() {
        let mut c = cam();
        let mut i = Inertia::release(800.0, 0.0, false);
        i.step(&mut c, INERTIA_HALF_LIFE_SECS);
        assert!(near(i.vx, 400.0, 1e-6));
        for _ in 0..200 {
            i.step(&mut c, 0.016);
        }
        assert!(!i.active());
    }
}
