//! Density and theme choice: the two per-user look settings
//! (docs/design/design-system.md §2.5 and §7).

use crate::tokens;

/// How tightly the interface is packed. The layout is the same at every
/// step; only metrics change.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Density {
    /// Laptops, many panels open.
    Compact,
    /// The default.
    #[default]
    Standard,
    /// Tablets, commissioning on site.
    Touch,
}

/// The metrics of a [`Density`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    /// Height of buttons, fields and rows.
    pub control_height: f32,
    /// Horizontal and vertical space between widgets.
    pub item_spacing: [f32; 2],
    /// Padding inside a button.
    pub button_padding: [f32; 2],
    /// Size of body text and buttons.
    pub font_body: f32,
    /// Size of captions and the HUD.
    pub font_small: f32,
    /// Size of headings.
    pub font_title: f32,
    /// Size of monospace text.
    pub font_mono: f32,
    /// How far from a widget the pointer still counts as on it.
    pub interact_radius: f32,
}

impl Density {
    pub const ALL: [Density; 3] = [Density::Compact, Density::Standard, Density::Touch];

    pub const fn metrics(self) -> Metrics {
        match self {
            Density::Compact => Metrics {
                control_height: 20.0,
                item_spacing: [6.0, 3.0],
                button_padding: [8.0, 2.0],
                font_body: 12.0,
                font_small: 10.5,
                font_title: 14.0,
                font_mono: 11.0,
                interact_radius: 0.0,
            },
            Density::Standard => Metrics {
                control_height: tokens::CONTROL_HEIGHT,
                item_spacing: [8.0, 5.0],
                button_padding: [9.0, 3.0],
                font_body: tokens::FONT_BODY,
                font_small: tokens::FONT_SMALL,
                font_title: tokens::FONT_HEADING,
                font_mono: tokens::FONT_MONO,
                interact_radius: 0.0,
            },
            Density::Touch => Metrics {
                control_height: 36.0,
                item_spacing: [12.0, 8.0],
                button_padding: [14.0, 8.0],
                font_body: 15.0,
                font_small: 13.0,
                font_title: 18.0,
                font_mono: 14.0,
                interact_radius: 6.0,
            },
        }
    }

    /// Name as shown in Preferences.
    pub const fn label(self) -> &'static str {
        match self {
            Density::Compact => "Compact",
            Density::Standard => "Standard",
            Density::Touch => "Touch",
        }
    }

    /// Stable id for the settings file.
    pub const fn id(self) -> &'static str {
        match self {
            Density::Compact => "compact",
            Density::Standard => "standard",
            Density::Touch => "touch",
        }
    }

    /// The density with this id; Standard for anything unknown.
    pub fn from_id(id: &str) -> Density {
        Self::ALL
            .into_iter()
            .find(|d| d.id() == id)
            .unwrap_or_default()
    }
}

/// Which theme the interface uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeChoice {
    /// Follow the operating system's light or dark setting.
    System,
    /// The graphite theme: the default, as before.
    #[default]
    Dark,
    Light,
}

impl ThemeChoice {
    pub const ALL: [ThemeChoice; 3] = [ThemeChoice::Dark, ThemeChoice::Light, ThemeChoice::System];

    pub const fn label(self) -> &'static str {
        match self {
            ThemeChoice::System => "Follow system",
            ThemeChoice::Dark => "Dark",
            ThemeChoice::Light => "Light",
        }
    }

    pub const fn id(self) -> &'static str {
        match self {
            ThemeChoice::System => "system",
            ThemeChoice::Dark => "dark",
            ThemeChoice::Light => "light",
        }
    }

    /// The choice with this id; Dark for anything unknown.
    pub fn from_id(id: &str) -> ThemeChoice {
        Self::ALL
            .into_iter()
            .find(|t| t.id() == id)
            .unwrap_or_default()
    }

    pub(crate) fn preference(self) -> egui::ThemePreference {
        match self {
            ThemeChoice::System => egui::ThemePreference::System,
            ThemeChoice::Dark => egui::ThemePreference::Dark,
            ThemeChoice::Light => egui::ThemePreference::Light,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_density_is_todays_look() {
        let m = Density::Standard.metrics();
        assert_eq!(m.control_height, tokens::CONTROL_HEIGHT);
        assert_eq!(m.font_body, tokens::FONT_BODY);
        assert_eq!(Density::default(), Density::Standard);
    }

    #[test]
    fn densities_grow_in_every_metric() {
        let m = Density::ALL.map(Density::metrics);
        for w in m.windows(2) {
            assert!(w[0].control_height < w[1].control_height);
            assert!(w[0].item_spacing[1] < w[1].item_spacing[1]);
            assert!(w[0].font_body < w[1].font_body);
        }
    }

    #[test]
    fn small_text_never_drops_below_ten_and_a_half_points() {
        for d in Density::ALL {
            assert!(d.metrics().font_small >= 10.5, "{d:?}");
        }
    }

    #[test]
    fn ids_round_trip_and_unknown_ids_fall_back() {
        for d in Density::ALL {
            assert_eq!(Density::from_id(d.id()), d);
        }
        for t in ThemeChoice::ALL {
            assert_eq!(ThemeChoice::from_id(t.id()), t);
        }
        assert_eq!(Density::from_id("huge"), Density::Standard);
        assert_eq!(ThemeChoice::from_id(""), ThemeChoice::Dark);
    }
}
