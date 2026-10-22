//! Lumping of cold neighbours for the 3D solver.
//!
//! The cost of a 3D mesh grows with the number of small bodies: the cells
//! across every body are the first claim on the budget (`novo-cfd` mesh
//! planner). A cabinet of a machine with 115 devices, 80 of them breakers,
//! relays and terminals of a few watts each, asks for about 1.5 million cells
//! at the Coarse level that is meant to have 6 000 to 50 000, and ends at its
//! time cap without converging.
//!
//! This module replaces runs of small, low-loss devices that stand next to
//! each other on the same row by one block with the sum of their losses.
//! What is kept: every device at or above [`Settings::keep_min_w`] stays
//! alone, the lump's loss is the sum, its depth the largest, its limit the
//! tightest of its members. What is lost: the temperature of the single
//! small device (the lump has one), and the air between the members, which
//! the block now fills. The result is a flow and heat picture of the
//! cabinet, not a per-device answer for the lumped devices.

use crate::model::*;
use novo_layout::Rect;

/// Choices of the reduction.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// Devices with at least this loss stay single, W.
    pub keep_min_w: f64,
    /// Widest lump, mm.
    pub max_width_mm: f64,
    /// Largest gap between two members of one lump, mm.
    pub max_gap_mm: f64,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            keep_min_w: 10.0,
            max_width_mm: 300.0,
            max_gap_mm: 30.0,
        }
    }
}

/// The reduced scenario and who went where.
#[derive(Debug, Clone, PartialEq)]
pub struct Lumped {
    pub scenario: Scenario,
    /// For every device of the reduced scenario, the ids of the original
    /// devices it stands for (one id for a device that stayed single).
    pub members: Vec<Vec<u64>>,
}

impl Lumped {
    pub fn lumps(&self) -> usize {
        self.members.iter().filter(|m| m.len() > 1).count()
    }
}

/// Lumps cold neighbours; see the module text. Deterministic; the original
/// scenario is untouched.
pub fn lump_cold_neighbours(scn: &Scenario, settings: &Settings) -> Lumped {
    // Rows are devices with the same top edge, left to right.
    let mut order: Vec<usize> = (0..scn.devices.len()).collect();
    order.sort_by(|a, b| {
        let (fa, fb) = (scn.devices[*a].footprint, scn.devices[*b].footprint);
        fa.y.total_cmp(&fb.y).then(fa.x.total_cmp(&fb.x))
    });
    let cold = |i: usize| scn.devices[i].loss_w < settings.keep_min_w;
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for i in order {
        let f = scn.devices[i].footprint;
        let joins = cold(i)
            && groups.last().is_some_and(|g: &Vec<usize>| {
                let first = scn.devices[g[0]].footprint;
                let last = scn.devices[*g.last().expect("group is not empty")].footprint;
                cold(g[0])
                    && g.iter().all(|m| cold(*m))
                    && (first.y - f.y).abs() < 1.0
                    && f.x - last.right() <= settings.max_gap_mm
                    && f.x - last.right() >= 0.0
                    && f.right() - first.x <= settings.max_width_mm
            });
        if joins {
            groups.last_mut().expect("checked above").push(i);
        } else {
            groups.push(vec![i]);
        }
    }
    let mut devices = Vec::new();
    let mut members = Vec::new();
    for g in groups {
        members.push(g.iter().map(|i| scn.devices[*i].id).collect());
        if g.len() == 1 {
            devices.push(scn.devices[g[0]].clone());
            continue;
        }
        let rects: Vec<Rect> = g.iter().map(|i| scn.devices[*i].footprint).collect();
        let x = rects.iter().map(|r| r.x).fold(f64::INFINITY, f64::min);
        let y = rects.iter().map(|r| r.y).fold(f64::INFINITY, f64::min);
        let right = rects
            .iter()
            .map(|r| r.right())
            .fold(f64::NEG_INFINITY, f64::max);
        let bottom = rects
            .iter()
            .map(|r| r.bottom())
            .fold(f64::NEG_INFINITY, f64::max);
        let first = &scn.devices[g[0]];
        let min_of = |f: fn(&Device) -> Option<f64>| {
            g.iter()
                .filter_map(|i| f(&scn.devices[*i]))
                .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.min(v))))
        };
        devices.push(Device {
            id: first.id,
            footprint: Rect {
                x,
                y,
                w: right - x,
                h: bottom - y,
            },
            height_mm: g
                .iter()
                .map(|i| scn.devices[*i].height())
                .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v)))),
            loss_w: g.iter().map(|i| scn.devices[*i].loss_w).sum(),
            rated_max_ambient_c: min_of(|d| d.rated_max_ambient_c),
            rated_max_surface_c: min_of(|d| d.rated_max_surface_c),
            heat_capacity_j_per_k: None,
        });
    }
    Lumped {
        scenario: Scenario {
            devices,
            ..scn.clone()
        },
        members,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_layout::sizing::{Installation, Material};

    fn dev(id: u64, x: f64, y: f64, w: f64, loss: f64) -> Device {
        Device {
            id,
            footprint: Rect { x, y, w, h: 90.0 },
            height_mm: Some(70.0),
            loss_w: loss,
            rated_max_ambient_c: Some(60.0),
            rated_max_surface_c: None,
            heat_capacity_j_per_k: None,
        }
    }

    fn scn(devices: Vec<Device>) -> Scenario {
        Scenario {
            enclosure: Enclosure {
                width_mm: 904.0,
                height_mm: 604.0,
                depth_mm: 304.0,
                wall_mm: 2.0,
                material: Material::Steel,
                installation: Installation::Wall,
                plate: Rect {
                    x: 50.0,
                    y: 50.0,
                    w: 800.0,
                    h: 500.0,
                },
            },
            ambient_c: 35.0,
            devices,
            obstacles: Vec::new(),
            vents: Vec::new(),
            fans: Vec::new(),
            params: Params::default(),
        }
    }

    #[test]
    fn a_row_of_breakers_becomes_a_few_blocks_and_keeps_the_heat() {
        let mut v = Vec::new();
        for i in 0..20 {
            v.push(dev(i + 1, 60.0 + 28.0 * i as f64, 60.0, 18.0, 2.5));
        }
        let before = scn(v);
        let l = lump_cold_neighbours(&before, &Settings::default());
        assert!(
            l.scenario.devices.len() <= 4,
            "{}",
            l.scenario.devices.len()
        );
        let total = |s: &Scenario| s.devices.iter().map(|d| d.loss_w).sum::<f64>();
        assert!((total(&before) - total(&l.scenario)).abs() < 1e-9);
        let ids: usize = l.members.iter().map(Vec::len).sum();
        assert_eq!(ids, 20);
        for d in &l.scenario.devices {
            assert!(d.footprint.w <= 300.0 + 1e-9);
        }
        assert!(l.lumps() >= 1);
    }

    #[test]
    fn a_hot_device_stays_alone_and_splits_the_row() {
        let before = scn(vec![
            dev(1, 60.0, 60.0, 40.0, 2.0),
            dev(2, 105.0, 60.0, 40.0, 2.0),
            dev(3, 150.0, 60.0, 160.0, 165.0),
            dev(4, 315.0, 60.0, 40.0, 2.0),
        ]);
        let l = lump_cold_neighbours(&before, &Settings::default());
        assert_eq!(l.scenario.devices.len(), 3);
        assert_eq!(l.members[0], vec![1, 2]);
        assert_eq!(l.members[1], vec![3]);
        assert_eq!(l.scenario.devices[1].loss_w, 165.0);
    }

    #[test]
    fn devices_on_other_rows_or_far_apart_do_not_join() {
        let before = scn(vec![
            dev(1, 60.0, 60.0, 40.0, 2.0),
            dev(2, 400.0, 60.0, 40.0, 2.0),
            dev(3, 60.0, 200.0, 40.0, 2.0),
        ]);
        let l = lump_cold_neighbours(&before, &Settings::default());
        assert_eq!(l.scenario.devices.len(), 3);
        assert_eq!(l.lumps(), 0);
    }

    #[test]
    fn the_tightest_limit_and_the_deepest_member_win() {
        let mut a = dev(1, 60.0, 60.0, 40.0, 2.0);
        let mut b = dev(2, 105.0, 60.0, 40.0, 3.0);
        a.rated_max_ambient_c = Some(55.0);
        b.height_mm = Some(120.0);
        let l = lump_cold_neighbours(&scn(vec![a, b]), &Settings::default());
        let d = &l.scenario.devices[0];
        assert_eq!(d.rated_max_ambient_c, Some(55.0));
        assert_eq!(d.height_mm, Some(120.0));
        assert_eq!(d.loss_w, 5.0);
    }
}
