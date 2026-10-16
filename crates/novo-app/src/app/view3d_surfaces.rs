//! The cabinet's surfaces in the 3D view: the door, walls, roof, floor and
//! extra plates as faint panes with an outline, the one picked in the
//! Cabinet header drawn stronger. Overlay data only (the scene, picking and
//! the camera fit ignore it). Shown with Layers > Surfaces, and whenever a
//! surface other than the plate is picked in the Cabinet header
//! (View > Cabinet > Cabinet surfaces).

use novo_layout::surfaces::MAIN_PLATE;
use novo_view3d::{Overlay, OverlayLines, OverlayMesh, PlateFrame};

use super::cabinet_surfaces::{SurfaceRow, SurfaceSummary};

/// Pane and outline colours (sRGB, straight alpha). Quiet blue-grey for the
/// surfaces, the copper accent for the picked one.
const PANE: [u8; 4] = [120, 150, 190, 34];
const PANE_PICKED: [u8; 4] = [216, 127, 60, 96];
const EDGE: [u8; 4] = [140, 165, 200, 150];
const EDGE_PICKED: [u8; 4] = [232, 150, 80, 255];

/// The overlays for `summary`: every surface but the mounting plate (the
/// scene draws that). `plate_origin` is the mounting plate's corner in the
/// enclosure frame; `frame` is the scene's plate frame.
pub(super) fn overlays(
    summary: &SurfaceSummary,
    picked: Option<&str>,
    frame: PlateFrame,
) -> Vec<Overlay> {
    let Some(plate) = summary.row(MAIN_PLATE) else {
        return Vec::new();
    };
    let origin = plate.origin;
    let world = |p: [f64; 3]| frame.world(p[0] - origin[0], p[1] - origin[1], p[2] - origin[2]);
    let mut panes = OverlayMesh {
        translucent: true,
        order: 10,
        ..OverlayMesh::default()
    };
    let mut edges = OverlayLines {
        width_px: 1.5,
        order: 11,
        ..OverlayLines::default()
    };
    for row in summary.rows.iter().filter(|r| r.name != MAIN_PLATE) {
        let is_picked = picked == Some(row.name.as_str());
        let corners = corners_of(row).map(world);
        let base = panes.positions.len() as u32;
        let (fill, edge) = if is_picked {
            (PANE_PICKED, EDGE_PICKED)
        } else {
            (PANE, EDGE)
        };
        for c in corners {
            panes.positions.push(c);
            panes.colors.push(fill);
        }
        panes.indices.extend([0, 1, 2, 0, 2, 3].map(|i| base + i));
        edges.starts.push(edges.positions.len() as u32);
        for c in corners.iter().chain(std::iter::once(&corners[0])) {
            edges.positions.push(*c);
            edges.colors.push(edge);
        }
    }
    let mut out = Vec::new();
    if !panes.indices.is_empty() {
        out.push(Overlay::Mesh(panes));
        out.push(Overlay::Lines(edges));
    }
    out
}

/// The four corners of a surface in the enclosure frame, in order around it.
fn corners_of(row: &SurfaceRow) -> [[f64; 3]; 4] {
    let at = |a: f64, b: f64| {
        [
            row.origin[0] + row.u[0] * a + row.v[0] * b,
            row.origin[1] + row.u[1] * a + row.v[1] * b,
            row.origin[2] + row.u[2] * a + row.v[2] * b,
        ]
    };
    let (w, h) = (row.plate.width, row.plate.height);
    [at(0.0, 0.0), at(w, 0.0), at(w, h), at(0.0, h)]
}

/// Whether the surfaces are drawn: the Layers toggle, or a surface other
/// than the plate picked in the Cabinet header.
pub(super) fn wanted(toggle: bool, picked: Option<&str>) -> bool {
    toggle || picked.is_some_and(|n| n != MAIN_PLATE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;
    use novo_layout::Plate;

    fn row(name: &str, origin: [f64; 3], u: [f64; 3], v: [f64; 3], w: f64, h: f64) -> SurfaceRow {
        SurfaceRow {
            name: name.into(),
            kind: "x",
            size: String::new(),
            devices: 0,
            assumed: false,
            plate: Plate {
                width: w,
                height: h,
                margin: 10.0,
            },
            origin,
            u,
            v,
        }
    }

    fn summary() -> SurfaceSummary {
        SurfaceSummary {
            rows: vec![
                row(
                    "plate",
                    [100.0, 50.0, 20.0],
                    [1., 0., 0.],
                    [0., 1., 0.],
                    600.,
                    800.,
                ),
                row(
                    "door",
                    [0., 0., 250.],
                    [1., 0., 0.],
                    [0., 1., 0.],
                    800.,
                    900.,
                ),
                row("roof", [0., 0., 0.], [1., 0., 0.], [0., 0., 1.], 800., 250.),
            ],
            ..SurfaceSummary::default()
        }
    }

    #[test]
    fn every_surface_but_the_plate_gets_a_pane_and_an_outline() {
        let frame = PlateFrame {
            width: 600.0,
            height: 800.0,
        };
        let all = overlays(&summary(), None, frame);
        assert_eq!(all.len(), 2);
        let Overlay::Mesh(m) = &all[0] else { panic!() };
        assert!(m.is_valid() && m.translucent);
        assert_eq!(m.positions.len(), 8, "two panes of four corners");
        let Overlay::Lines(l) = &all[1] else { panic!() };
        assert!(l.is_valid());
        assert_eq!(l.starts.len(), 2);
    }

    #[test]
    fn corners_are_in_the_plate_frame_with_y_up_and_z_out_of_the_plate() {
        let frame = PlateFrame {
            width: 600.0,
            height: 800.0,
        };
        let all = overlays(&summary(), Some("door"), frame);
        let Overlay::Mesh(m) = &all[0] else { panic!() };
        // Door corner (0, 0, 250) with the plate at (100, 50, 20): 100 mm
        // left of the plate, 50 mm above its top edge, 230 mm in front.
        assert_eq!(m.positions[0], Vec3::new(-100.0, 850.0, 230.0));
        // The picked surface is drawn stronger.
        assert_eq!(m.colors[0], PANE_PICKED);
        assert_eq!(m.colors[4], PANE);
    }

    #[test]
    fn a_cabinet_with_only_its_plate_has_nothing_to_draw() {
        let frame = PlateFrame {
            width: 600.0,
            height: 800.0,
        };
        let s = SurfaceSummary {
            rows: vec![row(
                "plate",
                [0.; 3],
                [1., 0., 0.],
                [0., 1., 0.],
                600.,
                800.,
            )],
            ..SurfaceSummary::default()
        };
        assert!(overlays(&s, None, frame).is_empty());
    }

    #[test]
    fn a_picked_surface_other_than_the_plate_shows_the_panes_without_the_toggle() {
        assert!(!wanted(false, None));
        assert!(!wanted(false, Some("plate")));
        assert!(wanted(false, Some("door")));
        assert!(wanted(true, None));
    }
}
