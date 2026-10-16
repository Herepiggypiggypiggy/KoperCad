//! What the user chooses about a simulation run: how far the quality ladder
//! climbs, the turbulence model and the ventilation openings. The 3D view's
//! Heat panel edits it; `HeatState::start` and `start_run` apply it.
//!
//! Vents are kept for the session: the project file has no place for them
//! yet (the bridge builds a sealed enclosure), so the panel says so.

use std::sync::Arc;

use novo_cfd::turb::TurbModel;
use novo_cfd::{Fidelity, Solver};
use novo_thermal::model::{Enclosure, Opening, Placement, Vent};

/// The wall a vent sits in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Wall {
    Top,
    Bottom,
    Left,
    Right,
}

impl Wall {
    pub(super) const ALL: [Wall; 4] = [Wall::Top, Wall::Bottom, Wall::Left, Wall::Right];

    pub(super) fn label(self) -> &'static str {
        match self {
            Wall::Top => "Top",
            Wall::Bottom => "Bottom",
            Wall::Left => "Left",
            Wall::Right => "Right",
        }
    }
}

/// One grille or louvre as the panel edits it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct VentSpec {
    pub wall: Wall,
    /// Where along the wall the centre is, 0 to 1 (bottom/left to top/right).
    pub at: f64,
    /// Along the wall, mm.
    pub width_mm: f64,
    /// The other way (for top and bottom: the depth), mm.
    pub height_mm: f64,
    /// Free share of the opening, 0 to 1.
    pub free_fraction: f64,
}

impl VentSpec {
    /// A louvred grille low on the left wall; the user edits it from there.
    pub(super) fn new(wall: Wall) -> VentSpec {
        VentSpec {
            wall,
            at: 0.5,
            width_mm: 150.0,
            height_mm: 100.0,
            free_fraction: 0.6,
        }
    }

    /// The vent in the solver's terms, placed in the interior of `e`.
    pub(super) fn to_vent(self, e: &Enclosure) -> Vent {
        let at = self.at.clamp(0.0, 1.0);
        let x = at * e.interior_width_mm();
        let y = at * e.interior_height_mm();
        let placement = match self.wall {
            Wall::Top => Placement::Top { x_mm: x },
            Wall::Bottom => Placement::Bottom { x_mm: x },
            Wall::Left => Placement::Left { y_mm: y },
            Wall::Right => Placement::Right { y_mm: y },
        };
        Vent {
            opening: Opening {
                placement,
                width_mm: self.width_mm.max(1.0),
                height_mm: self.height_mm.max(1.0),
            },
            free_area_fraction: self.free_fraction.clamp(0.05, 1.0),
        }
    }
}

/// The quality levels the panel offers (the Preview is the 2D estimate that
/// the Cabinet shows without a run).
pub(super) const LEVELS: [(Fidelity, &str, &str); 3] = [
    (
        Fidelity::Coarse,
        "Quick",
        "Coarse 3D, about 50 000 cells: seconds. Good for trying a layout.",
    ),
    (
        Fidelity::Medium,
        "Balanced",
        "Medium 3D, about 500 000 cells: a minute or so. Runs the quick level first.",
    ),
    (
        Fidelity::Fine,
        "Thorough",
        "Fine 3D, millions of cells: the longest run. Runs quick and balanced first and shows each result as it arrives.",
    ),
];

pub(super) const MODELS: [(TurbModel, &str, &str); 3] = [
    (
        TurbModel::Laminar,
        "Laminar",
        "No turbulence model. Matches the published natural-convection benchmarks; the default.",
    ),
    (
        TurbModel::KOmegaSst,
        "k-ω SST",
        "Menter's turbulence model with automatic wall treatment. Checked on a channel and a heated wall only.",
    ),
    (
        TurbModel::KEpsilon,
        "k-ε",
        "The classic model with wall functions, as a cross-check of k-ω SST.",
    ),
];

#[derive(Debug, Clone, PartialEq)]
pub(super) struct HeatSettings {
    /// The highest level the run climbs to.
    pub top: Fidelity,
    pub turbulence: TurbModel,
    pub vents: Vec<VentSpec>,
}

impl Default for HeatSettings {
    fn default() -> Self {
        HeatSettings {
            top: Fidelity::Fine,
            turbulence: TurbModel::Laminar,
            vents: Vec::new(),
        }
    }
}

impl HeatSettings {
    /// The solver for a run (tests use the synthetic demo ladder).
    pub(super) fn solver(&self) -> Arc<dyn Solver> {
        super::heat_run::default_solver(self.turbulence)
    }

    pub(super) fn vents_for(&self, e: &Enclosure) -> Vec<Vent> {
        self.vents.iter().map(|v| v.to_vent(e)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_layout::Rect;
    use novo_layout::sizing::{Installation, Material};

    fn enclosure() -> Enclosure {
        Enclosure {
            width_mm: 800.0,
            height_mm: 1000.0,
            depth_mm: 300.0,
            wall_mm: 2.0,
            material: Material::Steel,
            installation: Installation::Wall,
            plate: Rect {
                x: 0.0,
                y: 0.0,
                w: 700.0,
                h: 900.0,
            },
        }
    }

    #[test]
    fn a_top_vent_sits_across_the_interior_width() {
        let mut v = VentSpec::new(Wall::Top);
        v.at = 0.25;
        let e = enclosure();
        match v.to_vent(&e).opening.placement {
            Placement::Top { x_mm } => assert!((x_mm - 0.25 * 796.0).abs() < 1e-9),
            p => panic!("{p:?}"),
        }
    }

    #[test]
    fn a_side_vent_runs_along_the_interior_height() {
        let mut v = VentSpec::new(Wall::Left);
        v.at = 1.0;
        match v.to_vent(&enclosure()).opening.placement {
            Placement::Left { y_mm } => assert!((y_mm - 996.0).abs() < 1e-9),
            p => panic!("{p:?}"),
        }
    }

    #[test]
    fn the_free_share_stays_within_what_a_grille_can_be() {
        let mut v = VentSpec::new(Wall::Top);
        v.free_fraction = 7.0;
        assert_eq!(v.to_vent(&enclosure()).free_area_fraction, 1.0);
        v.free_fraction = 0.0;
        assert_eq!(v.to_vent(&enclosure()).free_area_fraction, 0.05);
    }

    #[test]
    fn the_defaults_are_the_sealed_laminar_thorough_run_of_before() {
        let s = HeatSettings::default();
        assert_eq!(s.top, Fidelity::Fine);
        assert_eq!(s.turbulence, TurbModel::Laminar);
        assert!(s.vents_for(&enclosure()).is_empty());
    }
}
