//! The exposure index map: how strongly each point of the plate is leaned on
//! by the noisy parts and runs around it. A picture for ordering and for
//! feeling the layout, not a field strength: it is made from distances and
//! the loudness of each source, with no frequency, current or physics in it.
//! The quasi-static field of the EMC field lane replaces it through
//! `Picture::field` when that exists, and the legend says which one is shown.

use super::picture::{Class, Field, Picture, Role};

/// Cell edge of the map, mm.
pub const CELL_MM: f32 = 10.0;
/// Most cells along one side, so a very large plate stays cheap.
const MAX_SIDE: usize = 160;

/// How loud a run is as a source of noise, 0 to 1, before its shield.
pub fn run_loudness(class: Class) -> f32 {
    match class {
        Class::Motor => 0.55,
        Class::Mains => 0.30,
        Class::Control => 0.06,
        Class::Bus => 0.04,
        Class::Analog | Class::Safety | Class::Earth => 0.0,
    }
}

/// A screen keeps most of a run's noise in.
pub const SHIELD_FACTOR: f32 = 0.2;

/// Reach of a source of loudness `level`, mm: the distance at which its
/// share has fallen to half.
pub fn reach_mm(level: f32) -> f32 {
    40.0 + 90.0 * level.clamp(0.0, 1.0)
}

/// Share of a source at `d` mm: 1 at the source, a half at its reach, and a
/// steady fall after that (steep fall past the reach).
pub fn falloff(d: f32, reach: f32) -> f32 {
    let r = reach.max(1.0);
    1.0 / (1.0 + (d / r).powi(3))
}

fn dist_to_segment(p: [f32; 2], a: [f32; 3], b: [f32; 3]) -> f32 {
    let (ax, ay) = (a[0], a[1]);
    let (dx, dy) = (b[0] - ax, b[1] - ay);
    let len2 = dx * dx + dy * dy;
    let t = if len2 <= f32::EPSILON {
        0.0
    } else {
        (((p[0] - ax) * dx + (p[1] - ay) * dy) / len2).clamp(0.0, 1.0)
    };
    let (qx, qy) = (ax + t * dx, ay + t * dy);
    ((p[0] - qx).powi(2) + (p[1] - qy).powi(2)).sqrt()
}

/// The exposure at a plate point, 0 to 1: sources add up, soft-capped so two
/// drives read as worse than one without going off the scale.
pub fn exposure_at(pic: &Picture, p: [f32; 2]) -> f32 {
    let mut sum = 0.0f32;
    for n in pic.nodes.iter().filter(|n| n.role == Role::Source) {
        // Distance to the footprint, not its centre: a big drive is loud
        // along its whole body.
        let [x, y, w, h] = n.rect;
        let dx = (x - p[0]).max(0.0).max(p[0] - (x + w));
        let dy = (y - p[1]).max(0.0).max(p[1] - (y + h));
        sum += n.level * falloff((dx * dx + dy * dy).sqrt(), reach_mm(n.level));
    }
    for r in &pic.runs {
        let loud = run_loudness(r.class) * if r.shielded { SHIELD_FACTOR } else { 1.0 };
        if loud <= 0.0 {
            continue;
        }
        let mut best = f32::INFINITY;
        for w in r.path.windows(2) {
            best = best.min(dist_to_segment(p, w[0], w[1]));
        }
        if best.is_finite() {
            sum += loud * falloff(best, reach_mm(loud));
        }
    }
    1.0 - (-1.6 * sum).exp()
}

/// The map of the whole plate. The engine's own field wins when there is one.
pub fn exposure(pic: &Picture) -> Field {
    if let Some(f) = &pic.field {
        return f.clone();
    }
    let cell = CELL_MM
        .max(pic.plate[0] / MAX_SIDE as f32)
        .max(pic.plate[1] / MAX_SIDE as f32);
    let nx = ((pic.plate[0] / cell).ceil() as usize).max(1);
    let ny = ((pic.plate[1] / cell).ceil() as usize).max(1);
    let mut values = Vec::with_capacity(nx * ny);
    for j in 0..ny {
        for i in 0..nx {
            let p = [(i as f32 + 0.5) * cell, (j as f32 + 0.5) * cell];
            values.push(exposure_at(pic, p));
        }
    }
    Field {
        origin: [0.0, 0.0],
        cell,
        nx,
        ny,
        values,
        label: "Exposure index (geometry only)".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::picture::{Node, Provenance, Run};
    use super::*;

    fn source(level: f32) -> Node {
        Node {
            device: 1,
            tag: "-U1".into(),
            role: Role::Source,
            rect: [100.0, 100.0, 50.0, 80.0],
            height: 100.0,
            level,
            what: "drive".into(),
        }
    }

    fn pic_with(nodes: Vec<Node>, runs: Vec<Run>) -> Picture {
        let mut p = Picture::empty([400.0, 400.0], Provenance::Synthetic);
        p.nodes = nodes;
        p.runs = runs;
        p
    }

    fn run(class: Class, shielded: bool) -> Run {
        Run {
            id: 7,
            label: "w".into(),
            class,
            shielded,
            path: vec![[200.0, 50.0, 5.0], [200.0, 350.0, 5.0]],
            halo_mm: 0.0,
            class_reason: String::new(),
            overridden: false,
        }
    }

    #[test]
    fn falls_off_with_distance_and_is_half_at_the_reach() {
        assert!((falloff(0.0, 80.0) - 1.0).abs() < 1e-6);
        assert!((falloff(80.0, 80.0) - 0.5).abs() < 1e-6);
        assert!(falloff(200.0, 80.0) < falloff(100.0, 80.0));
    }

    #[test]
    fn is_loudest_at_the_source_and_quiet_far_away() {
        let p = pic_with(vec![source(1.0)], vec![]);
        let near = exposure_at(&p, [125.0, 140.0]);
        let far = exposure_at(&p, [390.0, 390.0]);
        assert!(near > 0.7, "{near}");
        assert!(far < 0.2, "{far}");
        assert!(near > far);
    }

    #[test]
    fn a_louder_source_gives_more_everywhere() {
        let quiet = pic_with(vec![source(0.3)], vec![]);
        let loud = pic_with(vec![source(0.9)], vec![]);
        for p in [[120.0, 120.0], [200.0, 200.0], [300.0, 100.0]] {
            assert!(exposure_at(&loud, p) > exposure_at(&quiet, p));
        }
    }

    #[test]
    fn a_shield_keeps_most_of_a_run_in() {
        let open = pic_with(vec![], vec![run(Class::Motor, false)]);
        let screened = pic_with(vec![], vec![run(Class::Motor, true)]);
        let at = [230.0, 200.0];
        assert!(exposure_at(&screened, at) < 0.5 * exposure_at(&open, at));
    }

    #[test]
    fn quiet_classes_make_no_noise_and_the_value_stays_in_range() {
        let p = pic_with(vec![], vec![run(Class::Analog, false)]);
        assert_eq!(exposure_at(&p, [200.0, 200.0]), 0.0);
        let many = pic_with(
            (0..6).map(|_| source(1.0)).collect(),
            vec![run(Class::Motor, false), run(Class::Mains, false)],
        );
        let v = exposure_at(&many, [125.0, 140.0]);
        assert!((0.0..=1.0).contains(&v));
    }

    #[test]
    fn the_map_covers_the_plate_and_prefers_the_engines_field() {
        let p = pic_with(vec![source(1.0)], vec![]);
        let f = exposure(&p);
        assert_eq!(f.values.len(), f.nx * f.ny);
        assert!(f.nx as f32 * f.cell >= 400.0);
        assert!(f.at(125.0, 140.0) > f.at(380.0, 380.0));
        let mut q = p.clone();
        q.field = Some(Field {
            origin: [0.0, 0.0],
            cell: 100.0,
            nx: 1,
            ny: 1,
            values: vec![0.5],
            label: "engine".into(),
        });
        assert_eq!(exposure(&q).label, "engine");
    }
}
