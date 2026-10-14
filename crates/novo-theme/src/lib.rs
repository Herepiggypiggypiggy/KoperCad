//! NovoElectric look and feel.
//!
//! One place for the app's dark theme, its design tokens, the proposed dark
//! canvas palette and the brand assets. The app shell calls [`apply`] once at
//! start-up; screens use the [`tokens`] by name instead of raw colours.
//!
//! See `docs/design/ui-guidelines.md` for how screens should use them.

pub mod brand;
pub mod canvas;
pub mod fonts;
pub mod icons;
mod style;
pub mod tokens;

pub use canvas::{CANVAS_DARK, CanvasPalette};
pub use style::{apply, style, visuals};
