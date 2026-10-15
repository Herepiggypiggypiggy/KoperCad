//! Look and camera data for the 3D cabinet view (`docs/design/3d-look.md`).
//!
//! Plain Rust with no GPU or egui types, so everything here is unit tested
//! without a window. It lives in `novo-theme` until the view crate exists;
//! the modules are self contained and move over as they are.

pub mod camera;
pub mod cube;
pub mod labels;
pub mod palette;

pub use camera::{Camera, Inertia, Projection, SavedView, StandardView, Transition};
pub use cube::{CubeHit, view_cube_hit};
pub use labels::{Fade, Highlight, LabelCandidate, highlight, visible_labels};
pub use palette::{
    LightRig, Material, QualityTier, ThemeColors, View3dTheme, WireColor, wire_radius_mm, wire_rgba,
};
