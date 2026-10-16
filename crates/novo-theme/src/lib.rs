//! KoperCAD look and feel.
//!
//! One place for the app's themes (dark and light), its design tokens and
//! colour roles, the canvas palettes and the brand assets. The app shell
//! calls [`apply`] once at start-up; screens use the [`Palette`] roles and
//! the [`space`] and [`radius`] sizes by name instead of raw values.
//!
//! See `docs/design/design-system.md` for the spec and how screens use it.

pub mod brand;
pub mod canvas;
mod density;
pub mod fonts;
pub mod heat;
pub mod icons;
mod metrics;
mod palette;
mod style;
pub mod table;
pub mod tokens;
pub mod widgets;

pub use canvas::{
    CANVAS_DARK, CANVAS_LIGHT, CanvasPalette, POTENTIALS_DARK, POTENTIALS_LIGHT, PotentialPalette,
    canvas_palette, potential_palette,
};
pub use density::{Density, Metrics, ThemeChoice};
pub use metrics::{dialog_width, icon_size, radius, space};
pub use palette::Palette;
pub use style::{
    apply, apply_with, set_density, set_theme, style, style_for, visuals, visuals_for,
};
