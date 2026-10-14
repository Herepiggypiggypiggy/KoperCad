//! GPU drawing canvas (owned by WS-03): pan, zoom, grid, snap, layers,
//! hit testing and selection, rendered through wgpu.
//!
//! The crate is independent of the data model. Anything that wants to be
//! drawn becomes an [`Item`]: a list of [`Primitive`]s (lines, polylines,
//! rectangles, circles, arcs) on a [`Layer`], with optional connection
//! points to snap to, keyed by a caller-chosen [`ItemId`]. The model adapter
//! keeps the [`Scene`] in sync; the canvas handles everything view-related.
//!
//! - [`Viewport`]: world (mm, y down) to screen mapping, pan, zoom at cursor, fit.
//! - [`Grid`]: snap grid and adaptive grid display.
//! - [`Scene`]: items, layers and a spatial index for picking and box selection.
//! - [`snap()`]: connection, geometry and grid snapping.
//! - [`Selection`]: the selection set.
//! - [`Tessellator`]: primitives to triangles; zoom-independent meshes.
//! - `render::CanvasRenderer`: wgpu pipeline that draws meshes (feature
//!   `gpu`, on by default through `egui`).
//! - [`widget`] (feature `egui`): an egui widget wiring all of it together.
//! - [`model`] (feature `model`): keeps a scene in sync with a `novo-core` page.

pub mod demo;
pub mod geom;
pub mod grid;
pub mod marks;
#[cfg(feature = "model")]
pub mod model;
#[cfg(feature = "gpu")]
pub mod render;
pub mod scene;
pub mod selection;
pub mod shape;
pub mod snap;
pub mod style;
pub mod tessellate;
pub mod text;
pub mod viewport;
#[cfg(feature = "egui")]
pub mod widget;

pub use geom::Aabb;
pub use glam::{Vec2, vec2};
pub use grid::{Grid, GridStyle};
pub use marks::{Mark, MarkKind, MarkTarget, Marks};
pub use scene::{BoxMode, Hit, Item, ItemId, Layer, LayerId, Scene, SnapPoint};
pub use selection::{SelectOp, Selection};
pub use shape::{Placement, Primitive, Shape};
pub use snap::{SnapKind, SnapSettings, Snapped, ortho, snap};
pub use style::{Color, Stroke, WidthUnit};
pub use tessellate::{Highlight, ItemMeshCache, Mesh, SceneMesh, Tessellator, Vertex};
pub use text::{Anchor, Label};
pub use viewport::Viewport;
