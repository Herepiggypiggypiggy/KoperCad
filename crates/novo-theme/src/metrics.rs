//! Spacing, radii and the other fixed sizes, on a 4 pt grid
//! (docs/design/design-system.md §2.3 and §2.4).

/// Spacing, in points.
pub mod space {
    /// Inside chips, icon to label nudge.
    pub const XS: f32 = 2.0;
    /// Between a label and its field, between icon buttons.
    pub const S: f32 = 4.0;
    /// Between controls in a row, panel inner padding.
    pub const M: f32 = 8.0;
    /// Between groups, dialog padding.
    pub const L: f32 = 12.0;
    /// Between sections, window margin.
    pub const XL: f32 = 16.0;
    /// Around an empty state, between dialog sections.
    pub const XXL: f32 = 24.0;
}

/// Corner radii, in points.
pub mod radius {
    /// Key caps, tags inside rows.
    pub const XS: u8 = 2;
    /// Buttons, fields, rows.
    pub const CONTROL: u8 = crate::tokens::RADIUS_CONTROL;
    /// Floating panels, menus, popups, cards.
    pub const SURFACE: u8 = crate::tokens::RADIUS_WINDOW;
    /// Modal dialogs.
    pub const DIALOG: u8 = 10;
    /// Status pills and chips: more than half any control's height.
    pub const PILL: u8 = 255;
}

/// Icon sizes, in points (docs/design/design-system.md §2.6).
pub mod icon_size {
    /// Inside a chip.
    pub const CHIP: f32 = 12.0;
    /// Inside a row.
    pub const ROW: f32 = 14.0;
    /// Buttons and the dock.
    pub const BUTTON: f32 = 16.0;
    /// Panel headers.
    pub const HEADER: f32 = 20.0;
    /// Empty-state icons.
    pub const EMPTY: f32 = 32.0;
}

/// Dialog widths: always one of these.
pub mod dialog_width {
    pub const SMALL: f32 = 360.0;
    pub const MEDIUM: f32 = 480.0;
    pub const LARGE: f32 = 640.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spacing_is_on_the_four_point_grid_apart_from_the_half_step() {
        for s in [space::S, space::M, space::L, space::XL, space::XXL] {
            assert_eq!(s % 4.0, 0.0);
        }
        assert_eq!(space::XS, 2.0);
    }

    #[test]
    fn spacing_and_radii_only_grow() {
        let s = [
            space::XS,
            space::S,
            space::M,
            space::L,
            space::XL,
            space::XXL,
        ];
        assert!(s.windows(2).all(|w| w[0] < w[1]));
        let r = [radius::XS, radius::CONTROL, radius::SURFACE, radius::DIALOG];
        assert!(r.windows(2).all(|w| w[0] < w[1]));
    }
}
