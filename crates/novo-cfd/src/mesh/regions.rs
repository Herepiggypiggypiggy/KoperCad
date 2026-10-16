//! Scenario to solid boxes in the grid frame.
//!
//! Frame of the mesh: millimetres, `x` right, `y` down, `z` from the
//! mounting plate surface at the rear wall (`z = 0`) towards the door. The
//! plate is a slab `0 ..= plate_thickness`; devices and obstacles stand on
//! it. An obstacle has no height in the scenario: it is taken as its
//! `blockage` times the air depth in front of the plate (the meaning the 2D
//! model gives that number).

use novo_thermal::{Obstacle, ObstacleKind, Opening, Placement, Scenario};

use crate::result::{MaterialInfo, MaterialKind};

/// A solid axis-aligned box with its material id.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Region {
    pub material: u16,
    pub lo: [f64; 3],
    pub hi: [f64; 3],
}

impl Region {
    pub fn volume_mm3(&self) -> f64 {
        (0..3).map(|a| self.hi[a] - self.lo[a]).product()
    }
}

/// Interior size `[w, h, d]`, mm.
pub fn domain_mm(scn: &Scenario) -> [f64; 3] {
    let e = &scn.enclosure;
    [
        e.interior_width_mm(),
        e.interior_height_mm(),
        e.interior_depth_mm(),
    ]
}

/// Planning values for the solid materials; CFD-C2 replaces them with the
/// real conduction data. A partition (`ObstacleKind::Solid`) is a `Wall`.
const K_DEVICE: f64 = 1.0;
const K_DUCT: f64 = 0.2;
const K_RAIL: f64 = 50.0;
const K_PARTITION: f64 = 50.0;

/// Solid regions in priority order (earlier wins where boxes overlap:
/// devices, then obstacles, then the plate) and the material table. Boxes
/// are clipped to the domain; empty ones are dropped. Ids: 0 air, 1 plate,
/// then devices and obstacles in scenario order (ids of dropped ones are
/// skipped so ids stay stable when an item moves out of the domain).
pub fn regions(scn: &Scenario) -> (Vec<Region>, Vec<MaterialInfo>) {
    regions_with(scn, true)
}

/// [`regions`], with the mounting plate left out when `plate_slab` is false
/// (the caller models it as a sheet on the rear face; the material id 1
/// stays reserved so ids do not move).
pub fn regions_with(scn: &Scenario, plate_slab: bool) -> (Vec<Region>, Vec<MaterialInfo>) {
    let dom = domain_mm(scn);
    let t = scn.params.plate_thickness_mm.clamp(0.0, dom[2]);
    let mut mats = vec![
        MaterialInfo {
            id: 0,
            kind: MaterialKind::Air,
            name: "Air".into(),
            conductivity_w_mk: 0.026,
        },
        MaterialInfo {
            id: 1,
            kind: MaterialKind::Plate,
            name: "Mounting plate".into(),
            conductivity_w_mk: scn.params.plate_k,
        },
    ];
    let mut dev = Vec::new();
    let mut obs = Vec::new();
    let mut next = 2u16;
    let mut push = |list: &mut Vec<Region>, lo: [f64; 3], hi: [f64; 3], info: MaterialInfo| {
        let id = info.id;
        let r = clip(
            Region {
                material: id,
                lo,
                hi,
            },
            dom,
        );
        mats.push(info);
        if let Some(r) = r {
            list.push(r);
        }
    };
    for d in &scn.devices {
        let f = &d.footprint;
        let info = MaterialInfo {
            id: next,
            kind: MaterialKind::Device(d.id),
            name: format!("Device {}", d.id),
            conductivity_w_mk: K_DEVICE,
        };
        next += 1;
        push(
            &mut dev,
            [f.x, f.y, t],
            [f.right(), f.bottom(), t + d.height()],
            info,
        );
    }
    for (i, o) in scn.obstacles.iter().enumerate() {
        let (kind, name, k) = obstacle_material(o);
        let info = MaterialInfo {
            id: next,
            kind,
            name: format!("{name} {}", i + 1),
            conductivity_w_mk: k,
        };
        next += 1;
        let h = o.blockage().clamp(0.0, 1.0) * (dom[2] - t);
        let r = &o.rect;
        push(
            &mut obs,
            [r.x, r.y, t],
            [r.right(), r.bottom(), t + h],
            info,
        );
    }
    let p = &scn.enclosure.plate;
    let mut plate = Vec::new();
    if let Some(r) = clip(
        Region {
            material: 1,
            lo: [p.x, p.y, 0.0],
            hi: [p.right(), p.bottom(), t],
        },
        dom,
    ) && plate_slab
    {
        plate.push(r);
    }
    let mut all = dev;
    all.extend(obs);
    all.extend(plate);
    (all, mats)
}

fn obstacle_material(o: &Obstacle) -> (MaterialKind, &'static str, f64) {
    match o.kind {
        ObstacleKind::Duct => (MaterialKind::Duct, "Duct", K_DUCT),
        ObstacleKind::Rail => (MaterialKind::Rail, "Rail", K_RAIL),
        ObstacleKind::Solid => (MaterialKind::Wall, "Partition", K_PARTITION),
    }
}

fn clip(mut r: Region, dom: [f64; 3]) -> Option<Region> {
    for (a, d) in dom.iter().enumerate() {
        r.lo[a] = r.lo[a].max(0.0);
        r.hi[a] = r.hi[a].min(*d);
        if r.hi[a].partial_cmp(&r.lo[a]) != Some(std::cmp::Ordering::Greater) {
            return None;
        }
    }
    Some(r)
}

/// Interval an opening covers on each axis along its wall (`None` for the
/// axis across the wall, where its depth is not given).
pub fn opening_spans(o: &Opening) -> [Option<(f64, f64)>; 3] {
    let span = |c: f64, w: f64| Some((c - 0.5 * w, c + 0.5 * w));
    match o.placement {
        Placement::Top { x_mm } | Placement::Bottom { x_mm } => {
            [span(x_mm, o.width_mm), None, None]
        }
        Placement::Left { y_mm } | Placement::Right { y_mm } => {
            [None, span(y_mm, o.width_mm), None]
        }
        Placement::Front { x_mm, y_mm } => [span(x_mm, o.width_mm), span(y_mm, o.height_mm), None],
    }
}
