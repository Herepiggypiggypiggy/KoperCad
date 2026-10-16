//! Duplicate in the 3D view (Ctrl+D, right-click > Duplicate, View > 3D >
//! Duplicate): each selected device is copied as a new device of the same
//! kind with the same description, parts and properties, the next free tag
//! of its letter code, and put on the plate in the nearest free slot (no
//! overlap, the clearances kept, inside the plate, off the ducts). One undo
//! step takes all the copies back, and the copies end up selected so Ctrl+D
//! can go on.

use novo_core::{AnyId, Command, Device, DeviceId};
use novo_layout::state::CabinetLayout;
use novo_layout::{Placement, clearance_violations, collisions};
use novo_view3d::scene::{ItemId, ItemKind};

use super::NovoApp;

/// The most candidate spots `free_slot` lists.
const MAX_SPOTS: f64 = 40_000.0;

/// The top left of the free slot nearest to `src` for a copy of the same
/// size and turn, or `None` when the plate has no room. Ties go to the
/// same row, then to the right, so a copy lands next to its original.
pub(super) fn free_slot(layout: &CabinetLayout, src: &Placement) -> Option<(f64, f64)> {
    let mut step = if layout.grid > 0.0 { layout.grid } else { 5.0 };
    let usable = layout.plate.usable();
    // A huge plate on a fine grid would list millions of spots: coarsen the
    // search until it stays small (the copy then lands on a coarser grid).
    let cells = (usable.w / step).max(1.0) * (usable.h / step).max(1.0);
    if cells > MAX_SPOTS {
        step *= (cells / MAX_SPOTS).sqrt().ceil();
    }
    let mut copy = Placement {
        id: u64::MAX,
        rail: None,
        ..src.clone()
    };
    let (w, h) = {
        let f = copy.footprint();
        (f.w, f.h)
    };
    let snap = |v: f64| (v / step).ceil() * step;
    let mut spots: Vec<(f64, f64)> = Vec::new();
    let mut y = snap(usable.y);
    while y + h <= usable.bottom() + 1e-9 {
        let mut x = snap(usable.x);
        while x + w <= usable.right() + 1e-9 {
            spots.push((x, y));
            x += step;
        }
        y += step;
    }
    let dist = |s: &(f64, f64)| (s.0 - src.x).hypot(s.1 - src.y);
    spots.sort_by(|a, b| {
        let key = |s: &(f64, f64)| {
            (
                dist(s),
                (s.1 - src.y).abs(),
                // Right of the original before left of it.
                if s.0 >= src.x { 0.0_f64 } else { 1.0 },
            )
        };
        let (ka, kb) = (key(a), key(b));
        ka.0.total_cmp(&kb.0)
            .then(ka.1.total_cmp(&kb.1))
            .then(ka.2.total_cmp(&kb.2))
    });
    spots.into_iter().find(|&(x, y)| {
        (copy.x, copy.y) = (x, y);
        let rect = copy.footprint();
        usable.contains(&rect)
            && !layout.ducts.iter().any(|d| d.footprint().overlaps(&rect))
            && layout.placements.iter().all(|o| {
                let pair = [o.clone(), copy.clone()];
                collisions(&pair).is_empty() && clearance_violations(&pair).is_empty()
            })
    })
}

impl NovoApp {
    /// Copies the devices selected in the 3D view; see the module text.
    pub(super) fn view3d_duplicate(&mut self) {
        let Some(document) = self.backend.document() else {
            return;
        };
        let project = document.project();
        let Some(structure) = super::view3d::enclosure(project, self.cabinet.location.as_ref())
        else {
            return;
        };
        let Some(mut layout) = novo_layout::model::load(project, structure) else {
            return;
        };
        let sources: Vec<u64> = self
            .view3d
            .view
            .state
            .selection
            .iter()
            .filter(|i| i.kind == ItemKind::Device)
            .map(|i| i.raw)
            .filter(|d| layout.placements.iter().any(|p| p.id == *d))
            .collect();
        if sources.is_empty() {
            self.message(
                super::Level::Info,
                "Select a device in the 3D view to duplicate",
            );
            return;
        }
        // Find every slot first, each copy counting as placed for the next.
        let mut plan: Vec<(u64, (f64, f64), bool)> = Vec::new();
        for (n, src) in sources.iter().enumerate() {
            let Some(p) = layout.placements.iter().find(|p| p.id == *src).cloned() else {
                continue;
            };
            let Some(at) = free_slot(&layout, &p) else {
                self.message(
                    super::Level::Warning,
                    "No free room on the plate for a copy of the selected device",
                );
                return;
            };
            let mut placed = p.clone();
            (placed.id, placed.x, placed.y, placed.rail) = (u64::MAX - n as u64, at.0, at.1, None);
            layout.placements.push(placed);
            plan.push((*src, at, p.rotated));
        }
        let mut made: Vec<u64> = Vec::new();
        let done = self.cabinet_edit("Duplicate", |tx| {
            for (src, at, rotated) in &plan {
                let Some(device) = tx.project().get::<Device>(DeviceId::from(*src)).cloned() else {
                    continue;
                };
                let number = tx
                    .project()
                    .next_tag_number(&device.tag, device.tag.letter_code());
                let created = tx.execute(Command::AddDevice {
                    tag: device.tag.with_number(number),
                    description: device.description.clone(),
                    kind: device.kind,
                })?;
                let Some(AnyId::Device(copy)) = created.into_iter().next() else {
                    continue;
                };
                for part in &device.parts {
                    tx.execute(Command::AssignPart {
                        device: copy,
                        part: *part,
                    })?;
                }
                for (key, value) in &device.properties {
                    tx.execute(Command::SetProperty {
                        target: AnyId::Device(copy),
                        key: key.clone(),
                        value: Some(value.clone()),
                    })?;
                }
                novo_layout::model::place_device(tx, structure, copy.raw(), at.0, at.1, *rotated)?;
                made.push(copy.raw());
            }
            Ok(())
        });
        if done && !made.is_empty() {
            self.view3d.view.state.selection = made
                .iter()
                .map(|d| ItemId::new(ItemKind::Device, *d))
                .collect();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_layout::{Margins, Plate, Size};

    fn placement(id: u64, x: f64, y: f64) -> Placement {
        Placement {
            id,
            x,
            y,
            rotated: false,
            size: Size {
                width: 45.0,
                height: 90.0,
                guessed: false,
            },
            clearance: None,
            margins: Margins::default(),
            rail: Some(3),
        }
    }

    fn layout() -> CabinetLayout {
        let mut l = CabinetLayout::new(Plate {
            width: 400.0,
            height: 300.0,
            margin: 10.0,
        });
        l.placements = vec![placement(1, 100.0, 100.0)];
        l
    }

    #[test]
    fn the_copy_lands_beside_the_original_on_the_same_row() {
        let l = layout();
        let (x, y) = free_slot(&l, &l.placements[0]).unwrap();
        assert_eq!(y, 100.0, "same row");
        assert!(x > 100.0 + 45.0, "clear of the original on its right: {x}");
        let copy = Placement {
            id: 2,
            x,
            y,
            ..l.placements[0].clone()
        };
        let both = [l.placements[0].clone(), copy];
        assert!(collisions(&both).is_empty() && clearance_violations(&both).is_empty());
    }

    #[test]
    fn a_full_row_sends_the_copy_to_the_nearest_free_place() {
        let mut l = layout();
        // Fill the original's row to the right edge.
        let mut x = 160.0;
        let mut id = 2;
        while x + 45.0 <= 390.0 {
            l.placements.push(placement(id, x, 100.0));
            x += 60.0;
            id += 1;
        }
        let (sx, sy) = free_slot(&l, &l.placements[0]).unwrap();
        let copy = Placement {
            id: 99,
            x: sx,
            y: sy,
            ..l.placements[0].clone()
        };
        for o in &l.placements {
            let pair = [o.clone(), copy.clone()];
            assert!(collisions(&pair).is_empty(), "overlaps {}", o.id);
            assert!(clearance_violations(&pair).is_empty(), "crowds {}", o.id);
        }
        assert!(l.plate.usable().contains(&copy.footprint()));
    }

    #[test]
    fn a_huge_plate_still_finds_a_slot_quickly() {
        let mut l = CabinetLayout::new(Plate {
            width: 100_000.0,
            height: 100_000.0,
            margin: 10.0,
        });
        l.placements = vec![placement(1, 1000.0, 1000.0)];
        let t = std::time::Instant::now();
        let (x, y) = free_slot(&l, &l.placements[0]).expect("room");
        assert!(t.elapsed().as_secs() < 5, "bounded search");
        let copy = Placement {
            id: 2,
            x,
            y,
            ..l.placements[0].clone()
        };
        assert!(collisions(&[l.placements[0].clone(), copy]).is_empty());
    }

    #[test]
    fn a_plate_without_room_gives_no_slot() {
        let mut l = CabinetLayout::new(Plate {
            width: 80.0,
            height: 120.0,
            margin: 10.0,
        });
        l.placements = vec![placement(1, 10.0, 10.0)];
        assert!(free_slot(&l, &l.placements[0]).is_none());
    }

    #[test]
    fn ducts_are_kept_clear() {
        let mut l = layout();
        l.ducts = vec![novo_layout::Duct {
            id: 5,
            x: 145.0,
            y: 60.0,
            length: 200.0,
            width: 120.0,
            vertical: false,
        }];
        let (x, y) = free_slot(&l, &l.placements[0]).unwrap();
        let rect = Placement {
            x,
            y,
            ..l.placements[0].clone()
        }
        .footprint();
        assert!(!l.ducts[0].footprint().overlaps(&rect));
    }
}
