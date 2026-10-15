//! 3D cabinet view (WS-29): scene, camera, picking, culling and a light wgpu
//! renderer. Design and the scene contract: `docs/design/3d-view.md`.
//!
//! The scene, camera, picking and culling are plain Rust with no GPU and no
//! project model, so they are tested headless. The renderer is behind the
//! `gpu` feature and the egui widget behind `egui`.

#[cfg(feature = "model")]
pub mod build;
pub mod camera;
pub mod cube;
pub mod cull;
pub mod demo;
pub mod feel;
pub mod governor;
pub mod labels;
pub mod look;
pub mod math;
pub mod mesh;
pub mod palette;
pub mod pick;
pub mod scene;
pub mod view_state;
pub mod wire;

#[cfg(feature = "gpu")]
pub mod render;

#[cfg(feature = "egui")]
pub mod widget;

pub use camera::{Camera, Projection, ViewPreset};
pub use look::Look;
pub use math::{Aabb, Ray};
pub use mesh::{GeometryKey, MeshData, MeshLibrary, PartGeometry};
pub use scene::{
    CONTRACT_VERSION, Group, GroupId, Item, ItemId, ItemKind, Mark, PlateFrame, Pose, Profile,
    Scene, Shape, Wire,
};
pub use view_state::{Display, ViewState};
