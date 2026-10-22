//! What the user chooses about a simulation run: how far the quality ladder
//! climbs, the turbulence model and the openings (grilles, fans, filter fans).
//! The 3D view's Heat panel edits the first two, its Vents and fans panel the
//! openings; `HeatState::start` and `start_run` apply them.
//!
//! Openings are kept for the session: the project file has no place for them
//! yet (the bridge builds a sealed enclosure, and only WS-02 changes the
//! model), so the panel says so.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use novo_cfd::turb::TurbModel;
use novo_cfd::{Fidelity, Solver};
use novo_thermal::model::{Enclosure, Fan, FanMode, Opening, Placement, Scenario, Vent};

/// The wall an opening sits in. The door is the front of the cabinet, where
/// filter fans usually go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum Wall {
    Top,
    Bottom,
    Left,
    Right,
    Door,
}

impl Wall {
    pub(super) const ALL: [Wall; 5] =
        [Wall::Door, Wall::Left, Wall::Right, Wall::Top, Wall::Bottom];

    pub(super) fn label(self) -> &'static str {
        match self {
            Wall::Top => "Top",
            Wall::Bottom => "Bottom",
            Wall::Left => "Left wall",
            Wall::Right => "Right wall",
            Wall::Door => "Door",
        }
    }
}

/// What the opening is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum Kind {
    /// A passive grille or louvre: air goes where the pressure and the warm
    /// air push it.
    Grille,
    /// A fan in a cut-out.
    Fan,
    /// A fan with a filter mat in front; modelled as the fan, so enter the
    /// datasheet curve with the filter fitted.
    FilterFan,
}

impl Kind {
    pub(super) const ALL: [Kind; 3] = [Kind::Grille, Kind::Fan, Kind::FilterFan];

    pub(super) fn label(self) -> &'static str {
        match self {
            Kind::Grille => "Grille",
            Kind::Fan => "Fan",
            Kind::FilterFan => "Filter fan",
        }
    }

    pub(super) fn is_fan(self) -> bool {
        self != Kind::Grille
    }
}

/// Smallest opening, mm.
pub(super) const MIN_SIZE_MM: f64 = 20.0;
/// Planning size of a fan cut-out and its datasheet ends when the person has
/// not entered a maker's: stated in the panel as a starting point.
const FAN_SIZE_MM: f64 = 150.0;
const FAN_FLOW_M3H: f64 = 100.0;
const FAN_PRESSURE_PA: f64 = 60.0;
/// How far a preset sits from the floor or ceiling, mm (to the opening's
/// edge).
const EDGE_MARGIN_MM: f64 = 80.0;

/// A rectangle on a wall in the solver's frame (interior, mm, y down, z from
/// the plate to the door): `lo` and `hi` corners, flat on `wall`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Footprint {
    pub wall: Wall,
    pub lo: [f64; 3],
    pub hi: [f64; 3],
}

impl Footprint {
    /// True when the two faces lie on the same wall and share area.
    pub(super) fn overlaps(&self, other: &Footprint) -> bool {
        self.wall == other.wall
            && (0..3).all(|k| {
                let flat = self.hi[k] - self.lo[k] < 1e-6 || other.hi[k] - other.lo[k] < 1e-6;
                if flat {
                    self.lo[k] <= other.hi[k] && other.lo[k] <= self.hi[k]
                } else {
                    self.lo[k] < other.hi[k] && other.lo[k] < self.hi[k]
                }
            })
    }

    /// The direction air moves when it enters through this face, in the
    /// solver frame.
    pub(super) fn inward(&self) -> [f64; 3] {
        match self.wall {
            Wall::Top => [0.0, 1.0, 0.0],
            Wall::Bottom => [0.0, -1.0, 0.0],
            Wall::Left => [1.0, 0.0, 0.0],
            Wall::Right => [-1.0, 0.0, 0.0],
            Wall::Door => [0.0, 0.0, -1.0],
        }
    }

    pub(super) fn centre(&self) -> [f64; 3] {
        [0, 1, 2].map(|a| 0.5 * (self.lo[a] + self.hi[a]))
    }
}

/// One grille, fan or filter fan as the panel edits it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct VentSpec {
    pub kind: Kind,
    pub wall: Wall,
    /// A fan blows out instead of in (grilles ignore it).
    pub exhaust: bool,
    /// Centre along the wall, mm from the interior's left edge (top, bottom
    /// and door) or its top edge (left and right wall).
    pub u_mm: f64,
    /// Door only: centre, mm from the interior's top edge.
    pub v_mm: f64,
    /// Along the wall, mm.
    pub width_mm: f64,
    /// The other way (for top and bottom: the depth), mm.
    pub height_mm: f64,
    /// Free share of a grille's opening, 0 to 1.
    pub free_fraction: f64,
    /// A fan's free-air flow and its shut-off pressure (the two ends of the
    /// datasheet curve).
    pub flow_m3h: f64,
    pub pressure_pa: f64,
}

/// A suggestion the Add menu offers: a name, what it is for, the opening.
pub(super) struct Preset {
    pub name: &'static str,
    pub hint: &'static str,
    pub spec: VentSpec,
}

impl Default for VentSpec {
    fn default() -> Self {
        VentSpec::new(Wall::Left)
    }
}

impl VentSpec {
    /// A louvred grille in `wall`, centred and sized for a first look; the
    /// person edits it from there.
    pub(super) fn new(wall: Wall) -> VentSpec {
        VentSpec {
            kind: Kind::Grille,
            wall,
            exhaust: false,
            u_mm: 0.0,
            v_mm: 0.0,
            width_mm: 150.0,
            height_mm: 100.0,
            free_fraction: 0.6,
            flow_m3h: FAN_FLOW_M3H,
            pressure_pa: FAN_PRESSURE_PA,
        }
    }

    /// Length of the wall along the `u` axis and the other way, mm.
    fn wall_len(wall: Wall, e: &Enclosure) -> (f64, f64) {
        let (w, h, d) = (
            e.interior_width_mm(),
            e.interior_height_mm(),
            e.interior_depth_mm(),
        );
        match wall {
            Wall::Top | Wall::Bottom => (w, d),
            Wall::Left | Wall::Right => (h, d),
            Wall::Door => (w, h),
        }
    }

    /// This opening moved and cut down until it lies inside its wall: nothing
    /// hangs over an edge, so the area the solver sees is the area drawn.
    pub(super) fn fitted(self, e: &Enclosure) -> VentSpec {
        self.finite().fitted_finite(e)
    }

    /// Every number finite: one that is not a number (typed, or from a damaged
    /// file) falls back to the starting value instead of reaching the
    /// geometry or the stored text (a NaN is stored as null and loses the list).
    pub(super) fn finite(mut self) -> VentSpec {
        let start = VentSpec::new(self.wall);
        for (v, d) in [
            (&mut self.u_mm, 0.0),
            (&mut self.v_mm, 0.0),
            (&mut self.width_mm, start.width_mm),
            (&mut self.height_mm, start.height_mm),
            (&mut self.free_fraction, start.free_fraction),
            (&mut self.flow_m3h, start.flow_m3h),
            (&mut self.pressure_pa, start.pressure_pa),
        ] {
            if !v.is_finite() {
                *v = d;
            }
        }
        self
    }

    fn fitted_finite(mut self, e: &Enclosure) -> VentSpec {
        let (along, across) = Self::wall_len(self.wall, e);
        self.width_mm = self.width_mm.clamp(MIN_SIZE_MM, along.max(MIN_SIZE_MM));
        self.height_mm = self.height_mm.clamp(MIN_SIZE_MM, across.max(MIN_SIZE_MM));
        let inside = |c: f64, size: f64, len: f64| {
            if len <= size {
                0.5 * len
            } else {
                c.clamp(0.5 * size, len - 0.5 * size)
            }
        };
        self.u_mm = inside(self.u_mm, self.width_mm, along);
        self.v_mm = if self.wall == Wall::Door {
            inside(self.v_mm, self.height_mm, across)
        } else {
            0.0
        };
        self.free_fraction = self.free_fraction.clamp(0.05, 1.0);
        self.flow_m3h = self.flow_m3h.max(1.0);
        self.pressure_pa = self.pressure_pa.max(1.0);
        self
    }

    /// The opening in the solver's terms, after [`VentSpec::fitted`].
    pub(super) fn opening(self, e: &Enclosure) -> Opening {
        let s = self.fitted(e);
        let placement = match s.wall {
            Wall::Top => Placement::Top { x_mm: s.u_mm },
            Wall::Bottom => Placement::Bottom { x_mm: s.u_mm },
            Wall::Left => Placement::Left { y_mm: s.u_mm },
            Wall::Right => Placement::Right { y_mm: s.u_mm },
            Wall::Door => Placement::Front {
                x_mm: s.u_mm,
                y_mm: s.v_mm,
            },
        };
        Opening {
            placement,
            width_mm: s.width_mm,
            height_mm: s.height_mm,
        }
    }

    /// The passive vent for a grille; `None` for a fan.
    pub(super) fn to_vent(self, e: &Enclosure) -> Option<Vent> {
        (!self.kind.is_fan()).then(|| Vent {
            opening: self.opening(e),
            free_area_fraction: self.free_fraction.clamp(0.05, 1.0),
        })
    }

    /// The fan for a fan or filter fan; `None` for a grille.
    pub(super) fn to_fan(self, e: &Enclosure) -> Option<Fan> {
        self.kind.is_fan().then(|| Fan {
            opening: self.opening(e),
            mode: if self.exhaust {
                FanMode::Exhaust
            } else {
                FanMode::Intake
            },
            free_flow_m3h: self.flow_m3h.max(1.0),
            max_pressure_pa: self.pressure_pa.max(1.0),
        })
    }

    /// Where the opening is on its wall, the way the solver cuts it (see
    /// `novo_cfd::openings`): the depth of a top, bottom or side opening is
    /// centred.
    pub(super) fn footprint(self, e: &Enclosure) -> Footprint {
        let s = self.fitted(e);
        let (w, h, d) = (
            e.interior_width_mm(),
            e.interior_height_mm(),
            e.interior_depth_mm(),
        );
        let span = |c: f64, size: f64| (c - 0.5 * size, c + 0.5 * size);
        let (x, y, z, face) = match s.wall {
            Wall::Top => (
                span(s.u_mm, s.width_mm),
                (0.0, 0.0),
                span(0.5 * d, s.height_mm),
                s.wall,
            ),
            Wall::Bottom => (
                span(s.u_mm, s.width_mm),
                (h, h),
                span(0.5 * d, s.height_mm),
                s.wall,
            ),
            Wall::Left => (
                (0.0, 0.0),
                span(s.u_mm, s.width_mm),
                span(0.5 * d, s.height_mm),
                s.wall,
            ),
            Wall::Right => (
                (w, w),
                span(s.u_mm, s.width_mm),
                span(0.5 * d, s.height_mm),
                s.wall,
            ),
            Wall::Door => (
                span(s.u_mm, s.width_mm),
                span(s.v_mm, s.height_mm),
                (d, d),
                s.wall,
            ),
        };
        Footprint {
            wall: face,
            lo: [x.0, y.0, z.0],
            hi: [x.1, y.1, z.1],
        }
    }

    /// Which way the air goes through this opening before the run says: a
    /// fan blows the way it is set, a grille is open to either.
    pub(super) fn set_direction(self) -> Option<bool> {
        self.kind.is_fan().then_some(!self.exhaust)
    }

    /// Height of the centre above the cabinet floor, mm (left, right wall and
    /// door; for the top and bottom, the wall's own height).
    pub(super) fn height_above_floor_mm(self, e: &Enclosure) -> f64 {
        let h = e.interior_height_mm();
        match self.wall {
            Wall::Top => h,
            Wall::Bottom => 0.0,
            Wall::Left | Wall::Right => h - self.u_mm,
            Wall::Door => h - self.v_mm,
        }
    }

    /// Sets the centre so it sits `height` mm above the floor.
    pub(super) fn set_height_above_floor(&mut self, height: f64, e: &Enclosure) {
        let h = e.interior_height_mm();
        match self.wall {
            Wall::Left | Wall::Right => self.u_mm = h - height,
            Wall::Door => self.v_mm = h - height,
            Wall::Top | Wall::Bottom => {}
        }
    }

    /// The openings the Add menu offers for `e`, all in places that work: air
    /// in low, out high, opposite or far apart.
    pub(super) fn presets(e: &Enclosure) -> Vec<Preset> {
        let h = e.interior_height_mm();
        let w = e.interior_width_mm();
        let grille = |wall: Wall, u: f64| VentSpec {
            u_mm: u,
            ..VentSpec::new(wall)
        };
        let low = h - EDGE_MARGIN_MM - 50.0;
        let high = EDGE_MARGIN_MM + 50.0;
        let fan = |kind: Kind, wall: Wall, exhaust: bool, u: f64, v: f64| VentSpec {
            kind,
            exhaust,
            u_mm: u,
            v_mm: v,
            width_mm: FAN_SIZE_MM,
            height_mm: FAN_SIZE_MM,
            ..VentSpec::new(wall)
        };
        let half_fan = 0.5 * FAN_SIZE_MM + EDGE_MARGIN_MM;
        vec![
            Preset {
                name: "Grille, low on the left wall",
                hint: "Air comes in low and rises past the devices",
                spec: grille(Wall::Left, low).fitted(e),
            },
            Preset {
                name: "Grille, high on the right wall",
                hint: "Warm air leaves at the top, across from the low grille",
                spec: grille(Wall::Right, high).fitted(e),
            },
            Preset {
                name: "Filter fan, low on the door",
                hint: "Blows outside air in at the bottom; the usual pair with an exhaust grille",
                spec: fan(Kind::FilterFan, Wall::Door, false, 0.5 * w, h - half_fan).fitted(e),
            },
            Preset {
                name: "Exhaust grille, high on the door",
                hint: "Lets the warm air out at the top of the door",
                spec: grille(Wall::Door, 0.5 * w).fitted(e).with_v(high),
            },
            Preset {
                name: "Exhaust fan in the top",
                hint: "Draws the warm air out through the roof",
                spec: fan(Kind::Fan, Wall::Top, true, 0.5 * w, 0.0).fitted(e),
            },
        ]
    }

    fn with_v(mut self, v: f64) -> VentSpec {
        self.v_mm = v;
        self
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
    /// Grilles, fans and filter fans, in the order the person added them.
    pub vents: Vec<VentSpec>,
    /// The opening the panel has open and the 3D view highlights.
    pub selected: Option<usize>,
    /// A drag or a typed value is under way: it is one undo step.
    editing: bool,
    /// What the project holds for the shown enclosure, as last read or
    /// written (see `heat_openings_store`).
    pub(super) stored: Option<(novo_core::StructureId, String)>,
    /// A command (Ctrl+K) asked for this preset to be added; the panel does it.
    pub(super) requested: Option<usize>,
    undo: Vec<Vec<VentSpec>>,
    redo: Vec<Vec<VentSpec>>,
}

impl Default for HeatSettings {
    fn default() -> Self {
        HeatSettings {
            top: Fidelity::Fine,
            turbulence: TurbModel::Laminar,
            vents: Vec::new(),
            selected: None,
            editing: false,
            stored: None,
            requested: None,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }
}

/// Most steps of undo kept for the openings.
const UNDO_DEPTH: usize = 50;

impl HeatSettings {
    /// The solver for a run (tests use the synthetic demo ladder).
    pub(super) fn solver(&self) -> Arc<dyn Solver> {
        super::heat_run::default_solver(self.turbulence)
    }

    /// Adds the grilles and fans to the scenario of a run, fitted inside the
    /// walls of its enclosure.
    pub(super) fn apply(&self, s: &mut Scenario) {
        let e = s.enclosure.clone();
        s.vents
            .extend(self.vents.iter().filter_map(|v| v.to_vent(&e)));
        s.fans
            .extend(self.vents.iter().filter_map(|v| v.to_fan(&e)));
    }

    /// Remembers the openings as they are, to come back to with `undo`. Call
    /// before a change; a run of drags on one control is one step when the
    /// caller only calls it on the first frame of the drag.
    pub(super) fn checkpoint(&mut self) {
        if self.undo.last() != Some(&self.vents) {
            self.undo.push(self.vents.clone());
            if self.undo.len() > UNDO_DEPTH {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
    }

    /// Like `checkpoint`, but only on the first change of a drag or a typed
    /// value, so the whole gesture undoes at once.
    pub(super) fn checkpoint_edit(&mut self) {
        if !self.editing {
            self.checkpoint();
            self.editing = true;
        }
    }

    /// The gesture is over (the pointer is up and no field has focus).
    pub(super) fn end_edit(&mut self) {
        self.editing = false;
    }

    /// Pairs of openings that cover the same part of a wall: their flow would
    /// be counted twice.
    pub(super) fn overlapping(&self, e: &Enclosure) -> Vec<(usize, usize)> {
        let feet: Vec<Footprint> = self.vents.iter().map(|v| v.footprint(e)).collect();
        let mut pairs = Vec::new();
        for i in 0..feet.len() {
            for j in i + 1..feet.len() {
                if feet[i].overlaps(&feet[j]) {
                    pairs.push((i, j));
                }
            }
        }
        pairs
    }

    pub(super) fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub(super) fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub(super) fn undo(&mut self) {
        if let Some(before) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.vents, before));
            self.fix_selection();
        }
    }

    pub(super) fn redo(&mut self) {
        if let Some(after) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.vents, after));
            self.fix_selection();
        }
    }

    /// Replaces the openings with what the project holds (opened, or undone
    /// there); the panel's own undo starts over.
    pub(super) fn load(&mut self, vents: Vec<VentSpec>) {
        self.vents = vents;
        self.undo.clear();
        self.redo.clear();
        self.editing = false;
        self.fix_selection();
    }

    pub(super) fn is_editing(&self) -> bool {
        self.editing
    }

    fn fix_selection(&mut self) {
        if self.selected.is_some_and(|i| i >= self.vents.len()) {
            self.selected = self.vents.len().checked_sub(1);
        }
    }

    /// Adds an opening and selects it.
    pub(super) fn add(&mut self, spec: VentSpec) {
        self.checkpoint();
        self.vents.push(spec);
        self.selected = Some(self.vents.len() - 1);
    }

    pub(super) fn remove(&mut self, i: usize) {
        if i < self.vents.len() {
            self.checkpoint();
            self.vents.remove(i);
            self.selected = None;
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_cooling_suggestion_places_its_fans_side_by_side_inside_the_wall() {
        let e = enclosure();
        let sg = super::super::view3d_vents::Suggestion::for_test(Kind::FilterFan, 3, 180.0);
        let specs = sg.specs_for_test(&e);
        assert_eq!(specs.len(), 3);
        assert!(
            specs
                .iter()
                .all(|v| v.kind == Kind::FilterFan && v.flow_m3h == 180.0)
        );
        assert!(!specs[0].footprint(&e).overlaps(&specs[1].footprint(&e)));
        assert!(!specs[1].footprint(&e).overlaps(&specs[2].footprint(&e)));
    }

    #[test]
    fn identical_openings_are_reported_as_overlapping_and_apart_ones_are_not() {
        let e = enclosure();
        let a = VentSpec::presets(&e)[2].spec;
        let mut set = HeatSettings::default();
        set.add(a);
        set.add(a);
        assert_eq!(set.overlapping(&e), vec![(0, 1)]);
        let mut b = a;
        b.u_mm = if a.u_mm > 300.0 {
            60.0
        } else {
            e.interior_width_mm() - 60.0
        };
        set.vents[1] = b;
        let far = set.vents[1]
            .footprint(&e)
            .overlaps(&set.vents[0].footprint(&e));
        assert!(!far || b.width_mm > 200.0, "{b:?}");
    }

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

    fn scenario() -> Scenario {
        Scenario {
            enclosure: enclosure(),
            ambient_c: 35.0,
            devices: Vec::new(),
            obstacles: Vec::new(),
            vents: Vec::new(),
            fans: Vec::new(),
            params: novo_thermal::model::Params::default(),
        }
    }

    #[test]
    fn a_top_vent_keeps_its_centre_when_it_fits() {
        let mut v = VentSpec::new(Wall::Top);
        v.u_mm = 200.0;
        match v.to_vent(&enclosure()).unwrap().opening.placement {
            Placement::Top { x_mm } => assert_eq!(x_mm, 200.0),
            p => panic!("{p:?}"),
        }
    }

    #[test]
    fn an_opening_pushed_over_an_edge_is_pulled_back_inside_the_wall() {
        // Interior is 796 wide, 996 high; a 150 wide grille centred at 5 mm
        // would hang 70 mm over the edge, which the solver used to cut off.
        let mut v = VentSpec::new(Wall::Left);
        v.u_mm = 5.0;
        let f = v.footprint(&enclosure());
        assert_eq!(f.lo[1], 0.0);
        assert_eq!(f.hi[1], 150.0);
        v.u_mm = 5000.0;
        let f = v.footprint(&enclosure());
        assert_eq!(f.hi[1], 996.0);
        assert_eq!(f.lo[1], 846.0);
    }

    #[test]
    fn an_opening_bigger_than_its_wall_is_cut_to_the_wall() {
        let mut v = VentSpec::new(Wall::Top);
        v.width_mm = 5000.0;
        v.height_mm = 5000.0;
        let f = v.footprint(&enclosure());
        assert_eq!((f.lo[0], f.hi[0]), (0.0, 796.0));
        assert_eq!((f.lo[2], f.hi[2]), (0.0, 296.0));
    }

    #[test]
    fn the_footprint_is_flat_on_its_wall_and_matches_the_solvers_cut() {
        let e = enclosure();
        let mut v = VentSpec::new(Wall::Right);
        v.u_mm = 500.0;
        let f = v.footprint(&e);
        assert_eq!((f.lo[0], f.hi[0]), (796.0, 796.0));
        assert_eq!((f.lo[1], f.hi[1]), (425.0, 575.0));
        // The depth is centred, like `novo_cfd::openings`.
        assert_eq!((f.lo[2], f.hi[2]), (98.0, 198.0));
        let mut d = VentSpec::new(Wall::Door);
        d.u_mm = 400.0;
        d.v_mm = 100.0;
        let f = d.footprint(&e);
        assert_eq!((f.lo[2], f.hi[2]), (296.0, 296.0));
        assert_eq!((f.lo[1], f.hi[1]), (50.0, 150.0));
    }

    #[test]
    fn a_fan_goes_to_the_fans_and_a_grille_to_the_vents() {
        let mut s = scenario();
        let mut settings = HeatSettings::default();
        let mut fan = VentSpec::new(Wall::Door);
        fan.kind = Kind::FilterFan;
        fan.u_mm = 400.0;
        fan.v_mm = 800.0;
        let mut out = VentSpec::new(Wall::Top);
        out.u_mm = 400.0;
        settings.vents = vec![fan, out];
        settings.apply(&mut s);
        assert_eq!(s.vents.len(), 1);
        assert_eq!(s.fans.len(), 1);
        assert_eq!(s.fans[0].mode, FanMode::Intake);
        assert_eq!(s.fans[0].free_flow_m3h, FAN_FLOW_M3H);
        assert!(matches!(
            s.fans[0].opening.placement,
            Placement::Front { x_mm, y_mm } if x_mm == 400.0 && y_mm == 800.0
        ));
    }

    #[test]
    fn an_exhaust_fan_blows_out() {
        let mut v = VentSpec::new(Wall::Top);
        v.kind = Kind::Fan;
        v.exhaust = true;
        assert_eq!(v.to_fan(&enclosure()).unwrap().mode, FanMode::Exhaust);
        assert_eq!(v.set_direction(), Some(false));
        assert!(v.to_vent(&enclosure()).is_none());
    }

    #[test]
    fn a_number_that_is_not_a_number_falls_back_instead_of_reaching_the_geometry() {
        let e = enclosure();
        let mut v = VentSpec::new(Wall::Door);
        v.width_mm = f64::NAN;
        v.height_mm = f64::INFINITY;
        v.u_mm = f64::NAN;
        v.v_mm = f64::NEG_INFINITY;
        v.flow_m3h = f64::NAN;
        let f = v.footprint(&e);
        assert!(f.lo.iter().chain(&f.hi).all(|x| x.is_finite()), "{f:?}");
        assert!(f.hi[0] > f.lo[0] && f.hi[1] > f.lo[1]);
        let fan = VentSpec {
            kind: Kind::Fan,
            ..v
        };
        assert!(fan.to_fan(&e).unwrap().free_flow_m3h.is_finite());
    }

    #[test]
    fn the_free_share_stays_within_what_a_grille_can_be() {
        let mut v = VentSpec::new(Wall::Top);
        v.free_fraction = 7.0;
        assert_eq!(v.to_vent(&enclosure()).unwrap().free_area_fraction, 1.0);
        v.free_fraction = 0.0;
        assert_eq!(v.to_vent(&enclosure()).unwrap().free_area_fraction, 0.05);
    }

    #[test]
    fn the_defaults_are_the_sealed_laminar_thorough_run_of_before() {
        let s = HeatSettings::default();
        assert_eq!(s.top, Fidelity::Fine);
        assert_eq!(s.turbulence, TurbModel::Laminar);
        let mut sc = scenario();
        s.apply(&mut sc);
        assert!(sc.vents.is_empty() && sc.fans.is_empty());
    }

    #[test]
    fn height_above_the_floor_round_trips_on_a_side_wall_and_the_door() {
        let e = enclosure();
        for wall in [Wall::Left, Wall::Right, Wall::Door] {
            let mut v = VentSpec::new(wall);
            v.set_height_above_floor(300.0, &e);
            assert!(
                (v.height_above_floor_mm(&e) - 300.0).abs() < 1e-9,
                "{wall:?}"
            );
        }
    }

    #[test]
    fn every_preset_lies_inside_its_wall_and_low_is_below_high() {
        let e = enclosure();
        let presets = VentSpec::presets(&e);
        assert_eq!(presets.len(), 5);
        for p in &presets {
            assert_eq!(p.spec, p.spec.fitted(&e), "{}", p.name);
            let f = p.spec.footprint(&e);
            for a in 0..3 {
                assert!(f.lo[a] >= -1e-9 && f.hi[a] <= [796.0, 996.0, 296.0][a] + 1e-9);
            }
        }
        assert!(
            presets[0].spec.height_above_floor_mm(&e) < presets[1].spec.height_above_floor_mm(&e)
        );
        assert!(
            presets[2].spec.height_above_floor_mm(&e) < presets[3].spec.height_above_floor_mm(&e)
        );
        assert_eq!(presets[2].spec.set_direction(), Some(true));
    }

    #[test]
    fn undo_and_redo_walk_back_and_forth_and_a_new_change_drops_the_redo() {
        let e = enclosure();
        let mut s = HeatSettings::default();
        s.add(VentSpec::presets(&e)[0].spec);
        s.add(VentSpec::presets(&e)[1].spec);
        assert_eq!(s.vents.len(), 2);
        s.undo();
        assert_eq!(s.vents.len(), 1);
        s.undo();
        assert!(s.vents.is_empty());
        assert!(!s.can_undo());
        s.redo();
        assert_eq!(s.vents.len(), 1);
        s.add(VentSpec::presets(&e)[2].spec);
        assert!(!s.can_redo());
        s.remove(0);
        assert_eq!(s.vents.len(), 1);
        s.undo();
        assert_eq!(s.vents.len(), 2);
    }

    #[test]
    fn a_drag_is_one_undo_step_however_many_frames_it_changes_the_value() {
        let e = enclosure();
        let mut s = HeatSettings::default();
        s.add(VentSpec::presets(&e)[0].spec);
        s.end_edit();
        let start = s.vents[0].u_mm;
        for step in 1..=5 {
            s.checkpoint_edit();
            s.vents[0].u_mm = start + 10.0 * f64::from(step);
        }
        s.end_edit();
        s.undo();
        assert_eq!(s.vents[0].u_mm, start);
        s.undo();
        assert!(s.vents.is_empty());
    }

    #[test]
    fn a_selection_past_the_end_after_undo_moves_to_the_last_opening() {
        let e = enclosure();
        let mut s = HeatSettings::default();
        s.add(VentSpec::presets(&e)[0].spec);
        s.add(VentSpec::presets(&e)[1].spec);
        assert_eq!(s.selected, Some(1));
        s.undo();
        assert_eq!(s.selected, Some(0));
        s.undo();
        assert_eq!(s.selected, None);
    }
}
