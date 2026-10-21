//! What the Run card says about the corners and the neighbours of a routed
//! wire: whether the wire can take the corners of its path, and whether it
//! passes close to a device that gives off heat.
//!
//! Both are **models** and the card says so. A route is a centre line; a
//! duct is a straight channel of a stated width; a wire needs a bend radius
//! of a stated multiple of its diameter (`wire.bend_factor`, a planning
//! value that differs per cable); a device gives off its stated power loss
//! and the gap is a straight distance to its outline. No stiffness, no
//! insulation temperature rating and no surface temperature is known.
//!
//! Where the user finds it: click a wire in the 3D view (or a row of the
//! Wire runs list); the lines are on the Run card, and a tight corner is
//! marked on the lit path.

use novo_core::{Project, Wire};
use novo_routing::checks::{Hot, near_hot, tight_bends};
use novo_routing::{Duct, Point, Route, WireRules};

use super::cabinet_routes::Tone;

/// A device that gives off heat, with the name the card shows.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct HotDevice {
    pub hot: Hot,
    pub tag: String,
}

/// A corner the wire cannot take in the ducts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct TightCorner {
    pub at: Point,
    pub needed_mm: f64,
    pub available_mm: f64,
}

/// A device the run passes close to.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Neighbour {
    pub tag: String,
    pub gap_mm: f64,
    pub loss_w: f64,
}

/// Where the diameter used for the bend check comes from.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Size {
    /// The outer diameter of the cable part, mm.
    Cable(String, f64),
    /// The conductor with its insulation, from the wire's cross-section, mm.
    Wire(f64),
    Unknown,
}

/// The two checks for one run.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct RunChecks {
    pub size: Size,
    pub factor: f64,
    pub corners: Vec<TightCorner>,
    pub neighbours: Vec<Neighbour>,
    pub hot_gap_mm: f64,
    pub hot_loss_w: f64,
    /// Devices on the plate that count as hot, so "none close" means
    /// something.
    pub hot_devices: usize,
}

/// The outer diameter of the cable the wire is a core of, from its part.
fn cable_size(project: &Project, wire: &Wire) -> Option<(String, f64)> {
    let (cable, _) = project.core_of_wire(wire.id)?;
    let cable = project.cable(cable)?;
    let part = project.part(cable.part?)?;
    let mm = match part.properties.get("outer_diameter_mm")? {
        novo_core::Value::Float(v) => *v,
        novo_core::Value::Int(v) => *v as f64,
        _ => return None,
    };
    (mm.is_finite() && mm > 0.0).then(|| (cable.tag.to_string(), mm))
}

/// Runs both checks on `route` of `wire`.
pub(super) fn run_checks(
    project: &Project,
    wire: &Wire,
    route: &Route,
    ducts: &[Duct],
    hot: &[HotDevice],
    rules: &WireRules,
) -> RunChecks {
    let size = match cable_size(project, wire) {
        Some((tag, mm)) => Size::Cable(tag, mm),
        None => match novo_routing::model::wire_diameter_mm(wire.cross_section_mm2) {
            Some(mm) => Size::Wire(mm),
            None => Size::Unknown,
        },
    };
    let diameter = match &size {
        Size::Cable(_, d) | Size::Wire(d) => Some(*d),
        Size::Unknown => None,
    };
    let corners = diameter
        .map(|d| tight_bends(route, ducts, d, rules.bend_factor))
        .unwrap_or_default()
        .into_iter()
        .map(|b| TightCorner {
            at: b.at,
            needed_mm: b.needed_mm,
            available_mm: b.available_mm,
        })
        .collect();
    let own: Vec<u64> = [&wire.from, &wire.to]
        .into_iter()
        .filter_map(|e| e.as_pin())
        .filter_map(|p| novo_routing::model::device_of_pin(project, p))
        .map(|d| d.raw())
        .collect();
    let all: Vec<Hot> = hot.iter().map(|h| h.hot.clone()).collect();
    let neighbours = near_hot(route, &all, &own, rules.hot_gap_mm, rules.hot_loss_w)
        .into_iter()
        .map(|n| Neighbour {
            tag: hot
                .iter()
                .find(|h| h.hot.id == n.device)
                .map(|h| h.tag.clone())
                .unwrap_or_default(),
            gap_mm: n.gap_mm,
            loss_w: n.loss_w,
        })
        .collect();
    RunChecks {
        size,
        factor: rules.bend_factor,
        corners,
        neighbours,
        hot_gap_mm: rules.hot_gap_mm,
        hot_loss_w: rules.hot_loss_w,
        hot_devices: hot
            .iter()
            .filter(|h| h.hot.loss_w >= rules.hot_loss_w)
            .count(),
    }
}

impl RunChecks {
    /// What these checks add to the card, as toned lines. The last line says
    /// what is still not checked.
    pub(super) fn lines(&self) -> Vec<(Tone, String)> {
        let mut out = Vec::new();
        let (what, d) = match &self.size {
            Size::Cable(tag, d) => (format!("cable {tag} {d:.0} mm"), *d),
            Size::Wire(d) => (format!("wire {d:.1} mm"), *d),
            Size::Unknown => {
                out.push((
                    Tone::Busy,
                    "Bend radius not checked: no wire size or cable diameter is set".to_owned(),
                ));
                return self.with_heat(out);
            }
        };
        let need = d * self.factor;
        if self.corners.is_empty() {
            out.push((
                Tone::Idle,
                format!(
                    "Bend radius (model): the {what} needs {need:.0} mm ({:.0} x its diameter); \
                     the duct corners leave enough",
                    self.factor
                ),
            ));
        }
        for c in self.corners.iter().take(3) {
            out.push((
                Tone::Over,
                format!(
                    "Bend radius (model): at x {:.0}, y {:.0} mm the ducts leave about {:.0} mm, \
                     the {what} needs {:.0} mm ({:.0} x its diameter): widen the ducts or lay it \
                     round the corner by hand",
                    c.at.x, c.at.y, c.available_mm, c.needed_mm, self.factor
                ),
            ));
        }
        if self.corners.len() > 3 {
            out.push((
                Tone::Over,
                format!("... and {} more tight corners", self.corners.len() - 3),
            ));
        }
        self.with_heat(out)
    }

    fn with_heat(&self, mut out: Vec<(Tone, String)>) -> Vec<(Tone, String)> {
        for n in self.neighbours.iter().take(3) {
            out.push((
                Tone::Busy,
                format!(
                    "Heat (model): passes {:.0} mm from {} which loses {:.0} W (rule: keep {:.0} mm \
                     from devices losing {:.0} W or more)",
                    n.gap_mm,
                    if n.tag.is_empty() { "a device" } else { &n.tag },
                    n.loss_w,
                    self.hot_gap_mm,
                    self.hot_loss_w
                ),
            ));
        }
        if self.neighbours.is_empty() && self.hot_devices > 0 {
            out.push((
                Tone::Idle,
                format!(
                    "Heat (model): {:.0} mm or more from every device losing {:.0} W or more",
                    self.hot_gap_mm, self.hot_loss_w
                ),
            ));
        }
        out.push((
            Tone::Idle,
            "Not checked: how stiff this cable is, its temperature rating, the real surface \
             temperature of a device, corners at terminals and in free air"
                .to_owned(),
        ));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checks(size: Size) -> RunChecks {
        RunChecks {
            size,
            factor: 6.0,
            corners: Vec::new(),
            neighbours: Vec::new(),
            hot_gap_mm: 20.0,
            hot_loss_w: 5.0,
            hot_devices: 0,
        }
    }

    #[test]
    fn a_wire_that_fits_says_so_and_what_is_not_checked() {
        let lines = checks(Size::Wire(3.0)).lines();
        assert!(
            lines[0]
                .1
                .starts_with("Bend radius (model): the wire 3.0 mm needs 18 mm")
        );
        assert_eq!(lines[0].0, Tone::Idle);
        assert!(
            lines
                .last()
                .unwrap()
                .1
                .starts_with("Not checked: how stiff")
        );
    }

    #[test]
    fn a_tight_corner_is_red_and_gives_both_radii() {
        let mut c = checks(Size::Cable("-W1".into(), 20.0));
        c.corners.push(TightCorner {
            at: Point::new(300.0, 100.0),
            needed_mm: 120.0,
            available_mm: 40.0,
        });
        let lines = c.lines();
        assert_eq!(lines[0].0, Tone::Over);
        assert!(lines[0].1.contains("about 40 mm") && lines[0].1.contains("needs 120 mm"));
        assert!(lines[0].1.contains("cable -W1 20 mm"));
    }

    #[test]
    fn an_unknown_size_is_not_checked_and_says_why() {
        let lines = checks(Size::Unknown).lines();
        assert!(
            lines[0]
                .1
                .starts_with("Bend radius not checked: no wire size")
        );
    }

    #[test]
    fn a_close_hot_device_is_named_with_its_loss_and_the_rule() {
        let mut c = checks(Size::Wire(3.0));
        c.hot_devices = 1;
        c.neighbours.push(Neighbour {
            tag: "-K1".into(),
            gap_mm: 8.0,
            loss_w: 30.0,
        });
        let text = c
            .lines()
            .iter()
            .map(|l| l.1.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            text.contains("passes 8 mm from -K1 which loses 30 W"),
            "{text}"
        );
        assert!(text.contains("keep 20 mm"));
    }

    #[test]
    fn nothing_close_is_only_claimed_when_there_are_hot_devices() {
        let mut c = checks(Size::Wire(3.0));
        assert!(!c.lines().iter().any(|l| l.1.starts_with("Heat")));
        c.hot_devices = 2;
        assert!(
            c.lines()
                .iter()
                .any(|l| l.1.starts_with("Heat (model): 20 mm or more"))
        );
    }
}
