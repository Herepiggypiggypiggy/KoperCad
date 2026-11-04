//! Enclosure and mounting-plate bodies from a STEP model (see
//! `novo_parts::body`).
//!
//! A mounting plate is a thin flat solid. Its front face is the plane at
//! the top of the thin direction; the inner loops of that face are the
//! openings the plate is delivered with. Each loop is told apart as a round
//! hole (all points on one circle), a rectangle (axis-aligned, rounded
//! corners allowed, the bounding size is reported) or something else (a
//! slot, a shaped opening), which is only counted.
//!
//! Plate coordinates: origin at the top left seen from the front, x to the
//! right, y downward, millimetres. The front is the side of the thin
//! direction with the larger model coordinate and "right" and "up" follow
//! the model's axes in cyclic order (thin axis k, then k+1 to the right,
//! k+2 up), so a plate modelled in the X-Y plane reads as X right, Y up. A
//! maker may model it the other way round: the import summary says so and
//! the user checks one hole before drilling.

use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use novo_parts::body::{BodyKind, BodyReading};

use super::brep::{Face, Surface};
use super::build::{StepOptions, placed_faces};
use super::math::{V3, dot};

/// A plate is at most this thick, mm.
const MAX_PLATE_THICKNESS_MM: f64 = 12.0;
/// A plate is at least this many times wider than thick.
const MIN_PLATE_RATIO: f64 = 8.0;
/// Openings smaller than this are not listed (hairline slits, mesh).
const MIN_OPENING_MM: f64 = 1.0;
/// Most openings listed; a perforated sheet beyond this is reported only.
const MAX_OPENINGS: usize = 2000;

/// Reads the body of STEP text. A model that is not a flat plate comes back
/// as [`BodyKind::Other`] with its box size.
pub fn read_body(text: &str) -> Result<BodyReading, String> {
    if text.len() > super::MAX_STEP_BYTES {
        return Err("the STEP file is too large to analyse".into());
    }
    let opts = StepOptions {
        chord_tol_mm: 0.05,
        deadline: Some(Instant::now() + Duration::from_secs(20)),
        ..StepOptions::default()
    };
    let built = std::panic::catch_unwind(|| placed_faces(text, &opts, &AtomicBool::new(false)))
        .map_err(|_| "the STEP file has geometry that cannot be analysed".to_owned())?;
    let (faces, unit_unknown) = built.map_err(|e| e.to_string())?;
    if unit_unknown {
        return Err("the STEP file uses a length unit KoperCAD does not know".into());
    }
    analyze(&faces)
}

/// The analysis on faces already in model millimetres.
pub fn analyze(faces: &[Face]) -> Result<BodyReading, String> {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for p in faces.iter().flat_map(|f| f.loops.iter().flatten()) {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    if (0..3).any(|k| !(hi[k] - lo[k]).is_finite()) {
        return Err("the solids have no measurable extent".into());
    }
    let size = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
    let k = (0..3)
        .min_by(|&a, &b| size[a].total_cmp(&size[b]))
        .unwrap_or(2);
    let (a, b) = ((k + 1) % 3, (k + 2) % 3);
    let t = size[k];
    let flat = t <= MAX_PLATE_THICKNESS_MM && t * MIN_PLATE_RATIO <= size[a].min(size[b]);
    let mut reading = BodyReading {
        kind: BodyKind::Other,
        size_mm: size,
        holes: Vec::new(),
        cutouts: Vec::new(),
        other_openings: 0,
        notes: Vec::new(),
    };
    if !flat {
        return Ok(reading);
    }
    reading.kind = BodyKind::MountingPlate;
    reading.size_mm = [size[a], size[b], t];

    let mut axis = [0.0; 3];
    axis[k] = 1.0;
    let to_plate = |p: V3| (p[a] - lo[a], hi[b] - p[b]);
    let mut loops: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut outer_area = 0.0_f64;
    for face in faces {
        let Surface::Plane(f) = face.surface else {
            continue;
        };
        if dot(f.z, axis).abs() < 0.99 || (dot(f.o, axis) - hi[k]).abs() > 0.2 {
            continue;
        }
        let flat_loops: Vec<Vec<(f64, f64)>> = face
            .loops
            .iter()
            .map(|l| open(l.iter().map(|&p| to_plate(p)).collect()))
            .collect();
        let areas: Vec<f64> = flat_loops.iter().map(|l| area(l).abs()).collect();
        let Some(outer) = (0..flat_loops.len()).max_by(|&i, &j| areas[i].total_cmp(&areas[j]))
        else {
            continue;
        };
        outer_area = outer_area.max(areas[outer]);
        for (i, l) in flat_loops.into_iter().enumerate() {
            if i != outer && l.len() >= 3 {
                loops.push(l);
            }
        }
    }
    if outer_area == 0.0 {
        reading
            .notes
            .push("No flat front face was found, so no openings were read.".into());
        return Ok(reading);
    }
    if outer_area < 0.98 * size[a] * size[b] {
        reading.notes.push(
            "The plate outline is not a plain rectangle (corners or notches); the bounding size is given."
                .into(),
        );
    }
    if loops.len() > MAX_OPENINGS {
        reading.notes.push(format!(
            "{} openings: too many to list, none were read.",
            loops.len()
        ));
        return Ok(reading);
    }
    for l in &loops {
        match classify(l) {
            Opening::Round(x, y, d) => push_unique(&mut reading.holes, (x, y, d)),
            Opening::Rect(x, y, w, h) => {
                let c = (x, y, w, h);
                if !reading.cutouts.iter().any(|o| near4(*o, c)) {
                    reading.cutouts.push(c);
                }
            }
            Opening::Other => reading.other_openings += 1,
            Opening::Tiny => {}
        }
    }
    reading
        .holes
        .sort_by(|p, q| p.1.total_cmp(&q.1).then(p.0.total_cmp(&q.0)));
    reading
        .cutouts
        .sort_by(|p, q| p.1.total_cmp(&q.1).then(p.0.total_cmp(&q.0)));
    Ok(reading)
}

enum Opening {
    Round(f64, f64, f64),
    Rect(f64, f64, f64, f64),
    Other,
    Tiny,
}

fn open(mut l: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    if l.len() > 1 && dist(l[0], l[l.len() - 1]) < 1e-6 {
        l.pop();
    }
    l
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// Signed shoelace area.
fn area(l: &[(f64, f64)]) -> f64 {
    let n = l.len();
    (0..n)
        .map(|i| {
            let (p, q) = (l[i], l[(i + 1) % n]);
            p.0 * q.1 - q.0 * p.1
        })
        .sum::<f64>()
        / 2.0
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn near4(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
    (a.0 - b.0).abs() < 0.05
        && (a.1 - b.1).abs() < 0.05
        && (a.2 - b.2).abs() < 0.05
        && (a.3 - b.3).abs() < 0.05
}

fn push_unique(v: &mut Vec<(f64, f64, f64)>, h: (f64, f64, f64)) {
    if !v
        .iter()
        .any(|o| (o.0 - h.0).abs() < 0.05 && (o.1 - h.1).abs() < 0.05 && (o.2 - h.2).abs() < 0.05)
    {
        v.push(h);
    }
}

fn classify(l: &[(f64, f64)]) -> Opening {
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for p in l {
        x0 = x0.min(p.0);
        x1 = x1.max(p.0);
        y0 = y0.min(p.1);
        y1 = y1.max(p.1);
    }
    let (w, h) = (x1 - x0, y1 - y0);
    if w < MIN_OPENING_MM && h < MIN_OPENING_MM {
        return Opening::Tiny;
    }
    let a = area(l).abs();
    // Centroid of the polygon.
    let n = l.len();
    let (mut cx, mut cy) = (0.0, 0.0);
    for i in 0..n {
        let (p, q) = (l[i], l[(i + 1) % n]);
        let cr = p.0 * q.1 - q.0 * p.1;
        cx += (p.0 + q.0) * cr;
        cy += (p.1 + q.1) * cr;
    }
    let a6 = 6.0 * area(l);
    if a6.abs() < 1e-9 {
        return Opening::Tiny;
    }
    let (cx, cy) = (cx / a6, cy / a6);
    let radii: Vec<f64> = l.iter().map(|p| dist(*p, (cx, cy))).collect();
    let mean = radii.iter().sum::<f64>() / radii.len() as f64;
    let spread = radii.iter().map(|r| (r - mean).abs()).fold(0.0, f64::max);
    let circle_area = std::f64::consts::PI * mean * mean;
    if n >= 8 && spread <= 0.03 * mean && (a / circle_area) > 0.9 && (a / circle_area) < 1.03 {
        return Opening::Round(round2(cx), round2(cy), round2(2.0 * mean));
    }
    let rect_ratio = a / (w * h);
    if rect_ratio >= 0.93 && w.min(h) >= MIN_OPENING_MM {
        return Opening::Rect(
            round2(x0 + w / 2.0),
            round2(y0 + h / 2.0),
            round2(w),
            round2(h),
        );
    }
    Opening::Other
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::part3d::math::Frame;

    fn plane_at_z(z: f64, normal: f64) -> Surface {
        Surface::Plane(Frame {
            o: [0.0, 0.0, z],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            z: [0.0, 0.0, normal],
        })
    }

    fn rect_loop(x0: f64, y0: f64, x1: f64, y1: f64, z: f64) -> Vec<V3> {
        vec![[x0, y0, z], [x1, y0, z], [x1, y1, z], [x0, y1, z]]
    }

    fn circle_loop(cx: f64, cy: f64, d: f64, z: f64) -> Vec<V3> {
        (0..48)
            .map(|i| {
                let a = i as f64 / 48.0 * std::f64::consts::TAU;
                [cx + d / 2.0 * a.cos(), cy + d / 2.0 * a.sin(), z]
            })
            .collect()
    }

    fn face(loops: Vec<Vec<V3>>, surface: Surface) -> Face {
        Face {
            loops,
            surface,
            nurbs: None,
            same_sense: true,
        }
    }

    /// A 500 x 700 x 3 plate in the X-Y plane with two round holes, a
    /// rectangular opening, an L-shaped opening and a side face.
    fn plate_faces() -> Vec<Face> {
        let inner = |z| {
            vec![
                rect_loop(0.0, 0.0, 500.0, 700.0, z),
                circle_loop(25.0, 675.0, 9.0, z),
                circle_loop(475.0, 675.0, 9.0, z),
                rect_loop(230.0, 340.0, 270.0, 360.0, z),
                vec![
                    [100.0, 100.0, z],
                    [140.0, 100.0, z],
                    [140.0, 110.0, z],
                    [110.0, 110.0, z],
                    [110.0, 140.0, z],
                    [100.0, 140.0, z],
                ],
            ]
        };
        vec![
            face(inner(3.0), plane_at_z(3.0, 1.0)),
            face(inner(0.0), plane_at_z(0.0, -1.0)),
            face(
                vec![vec![
                    [0.0, 0.0, 0.0],
                    [500.0, 0.0, 0.0],
                    [500.0, 0.0, 3.0],
                    [0.0, 0.0, 3.0],
                ]],
                Surface::Plane(Frame {
                    o: [0.0; 3],
                    x: [1.0, 0.0, 0.0],
                    y: [0.0, 0.0, 1.0],
                    z: [0.0, -1.0, 0.0],
                }),
            ),
        ]
    }

    #[test]
    fn a_plate_gives_size_holes_and_openings_from_the_top_left() {
        let r = analyze(&plate_faces()).unwrap();
        assert_eq!(r.kind, BodyKind::MountingPlate);
        assert_eq!(r.size_mm, [500.0, 700.0, 3.0]);
        // Model y 675 is 25 mm from the top (y is flipped to run downward).
        assert_eq!(r.holes, vec![(25.0, 25.0, 9.0), (475.0, 25.0, 9.0)]);
        assert_eq!(r.cutouts, vec![(250.0, 350.0, 40.0, 20.0)]);
        assert_eq!(r.other_openings, 1);
    }

    #[test]
    fn a_thick_box_is_not_a_plate() {
        let faces = vec![
            face(
                vec![rect_loop(0.0, 0.0, 600.0, 760.0, 210.0)],
                plane_at_z(210.0, 1.0),
            ),
            face(
                vec![rect_loop(0.0, 0.0, 600.0, 760.0, 0.0)],
                plane_at_z(0.0, -1.0),
            ),
        ];
        let r = analyze(&faces).unwrap();
        assert_eq!(r.kind, BodyKind::Other);
        let mut s = r.size_mm;
        s.sort_by(f64::total_cmp);
        assert_eq!(s, [210.0, 600.0, 760.0]);
        assert!(r.holes.is_empty());
    }

    #[test]
    fn a_plate_modelled_on_its_side_reads_the_same_way() {
        // Thin in X: thin axis k = 0, right = Y, up = Z.
        let mut f = Face {
            loops: vec![
                vec![
                    [3.0, 0.0, 0.0],
                    [3.0, 500.0, 0.0],
                    [3.0, 500.0, 700.0],
                    [3.0, 0.0, 700.0],
                ],
                (0..48)
                    .map(|i| {
                        let a = i as f64 / 48.0 * std::f64::consts::TAU;
                        [3.0, 50.0 + 5.0 * a.cos(), 650.0 + 5.0 * a.sin()]
                    })
                    .collect(),
            ],
            surface: Surface::Plane(Frame {
                o: [3.0, 0.0, 0.0],
                x: [0.0, 1.0, 0.0],
                y: [0.0, 0.0, 1.0],
                z: [1.0, 0.0, 0.0],
            }),
            nurbs: None,
            same_sense: true,
        };
        let back = Face {
            loops: f
                .loops
                .iter()
                .map(|l| l.iter().map(|p| [0.0, p[1], p[2]]).collect())
                .collect(),
            ..f.clone()
        };
        f.same_sense = true;
        let r = analyze(&[f, back]).unwrap();
        assert_eq!(r.size_mm, [500.0, 700.0, 3.0]);
        assert_eq!(r.holes, vec![(50.0, 50.0, 10.0)]);
    }

    #[test]
    fn a_step_block_is_read_end_to_end_and_is_not_a_plate() {
        let r = read_body(&crate::part3d::build::test_support::block_step()).unwrap();
        assert_eq!(r.kind, BodyKind::Other);
        let mut s = r.size_mm;
        s.sort_by(f64::total_cmp);
        assert_eq!(s, [10.0, 20.0, 30.0]);
    }

    #[test]
    fn text_that_is_not_step_is_refused_in_plain_words() {
        assert!(read_body("hello").is_err());
    }
}
