//! Maker models in the 3D view: a device whose part has a STEP file in the
//! parts library is drawn with that model. The model is built off the UI
//! thread by `novo_interop::part3d::GeometryService` (the same service and
//! cache as the Parts window); until it is ready, and when it cannot be
//! used, the device keeps its typical shape or box, so the view never waits.

use std::collections::BTreeMap;
use std::sync::Arc;

use eframe::egui;
use glam::Vec3;
use novo_core::{CabinetPlacement, Device, Part, Project, StructureId};
use novo_interop::part3d::cache::MeshCache;
use novo_interop::part3d::service::{GeometryJob, GeometryService, GeometryState, TIME_LIMIT};
use novo_interop::part3d::{self, MeshOrigin, Quality};
use novo_parts::{FileKind, LocalLibrary, PartKey, SourceRecord};
use novo_view3d::scene::{ItemKind, Shape};
use novo_view3d::{Aabb, GeometryKey, MeshData, MeshLibrary, PartGeometry, Scene};

/// What became of one device's model request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    /// Asked for, not answered yet.
    Waiting,
    /// In the mesh library under this key.
    Drawn(GeometryKey),
    /// The part has a file but no usable model (see the Parts window).
    Unusable,
}

/// The models the open enclosure's devices ask for.
#[derive(Default)]
pub(super) struct Models {
    service: Option<GeometryService>,
    /// Service key per device (raw id) and what became of it.
    jobs: BTreeMap<u64, (u64, Outcome)>,
    /// Bumped whenever a model arrives, so the scene is built again.
    pub(super) revision: u64,
}

/// How many devices have a model drawn, wait for one, or have a file that
/// gave none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct Counts {
    pub drawn: usize,
    pub waiting: usize,
    pub unusable: usize,
}

impl Counts {
    /// The line for the Layers menu, `None` when no device has a file.
    pub(super) fn text(&self) -> Option<String> {
        let total = self.drawn + self.waiting + self.unusable;
        if total == 0 {
            return None;
        }
        let mut s = format!("Maker models: {} of {total} devices", self.drawn);
        if self.waiting > 0 {
            s.push_str(&format!(", {} still building", self.waiting));
        }
        if self.unusable > 0 {
            s.push_str(&format!(
                ", {} not usable (see the Parts window)",
                self.unusable
            ));
        }
        Some(s)
    }
}

/// The STEP record of each part, by the part's key, read once.
fn step_records(library: &LocalLibrary) -> BTreeMap<PartKey, SourceRecord> {
    let mut out = BTreeMap::new();
    for r in library.records().unwrap_or_default() {
        if FileKind::of_file_name(&r.file_name) == FileKind::Step {
            out.entry(r.key()).or_insert(r);
        }
    }
    out
}

/// The part of each device of the enclosure that is on the plate:
/// (device raw id, project part).
fn device_parts(project: &Project, id: StructureId) -> Vec<(u64, Part)> {
    project
        .all::<CabinetPlacement>()
        .filter(|p| p.structure == id)
        .filter_map(|p| {
            let device = project.get::<Device>(p.device)?;
            let part = device.parts.iter().find_map(|x| project.get::<Part>(*x))?;
            Some((p.device.raw(), part.clone()))
        })
        .collect()
}

/// A mesh of the interop crate as the view's own.
pub(super) fn convert(g: &part3d::PartGeometry) -> PartGeometry {
    PartGeometry {
        lods: g
            .lods
            .iter()
            .map(|m| MeshData {
                positions: m.positions.clone(),
                normals: m.normals.clone(),
                indices: m.indices.clone(),
                edges: m.edges.clone(),
                bounds: Aabb {
                    min: Vec3::from_array(m.bounds.min),
                    max: Vec3::from_array(m.bounds.max),
                },
            })
            .collect(),
    }
}

impl Models {
    /// Asks for the model of each device whose part has a STEP file. Cheap
    /// when nothing changed: a device already asked for is skipped.
    pub(super) fn plan(
        &mut self,
        ctx: &egui::Context,
        project: &Project,
        id: StructureId,
        scene: &Scene,
    ) {
        let Some(library) = LocalLibrary::open_default() else {
            return;
        };
        let records = step_records(&library);
        if records.is_empty() {
            self.jobs.clear();
            return;
        }
        let service = self.service.get_or_insert_with(|| {
            let wake = ctx.clone();
            GeometryService::new(
                MeshCache::open_default(),
                Quality::Normal,
                TIME_LIMIT,
                move || wake.request_repaint(),
            )
        });
        let mut wanted = BTreeMap::new();
        for (raw, part) in device_parts(project, id) {
            let Some(record) = records.get(&PartKey::new(&part.manufacturer, &part.order_number))
            else {
                continue;
            };
            let Some(item) = scene.get(novo_view3d::ItemId::new(ItemKind::Device, raw)) else {
                continue;
            };
            let job = GeometryJob {
                part_id: format!("{}|{}", part.manufacturer, part.order_number),
                content_id: record.sha256.clone(),
                step_path: library.path_of(record),
                size_mm: Some([
                    f64::from(item.size.x),
                    f64::from(item.size.y),
                    f64::from(item.size.z),
                ]),
            };
            let key = service.request(job);
            let outcome = match self.jobs.get(&raw) {
                Some((k, o)) if *k == key => *o,
                _ => Outcome::Waiting,
            };
            wanted.insert(raw, (key, outcome));
        }
        self.jobs = wanted;
    }

    /// Moves finished requests into the mesh library. Returns whether a model
    /// arrived (the scene must then be built again).
    pub(super) fn collect(&mut self, library: &mut Arc<MeshLibrary>) -> bool {
        let Some(service) = &self.service else {
            return false;
        };
        let mut arrived = false;
        for (key, outcome) in self.jobs.values_mut() {
            if *outcome != Outcome::Waiting {
                continue;
            }
            match service.geometry(*key) {
                GeometryState::Ready(g) if matches!(g.origin, MeshOrigin::Step { .. }) => {
                    let k = GeometryKey(*key);
                    Arc::make_mut(library).insert(k, convert(&g));
                    *outcome = Outcome::Drawn(k);
                    arrived = true;
                }
                GeometryState::Ready(_) | GeometryState::Failed(_) => *outcome = Outcome::Unusable,
                GeometryState::None | GeometryState::Pending => {}
            }
        }
        if arrived {
            self.revision += 1;
        }
        arrived
    }

    /// Gives each device its model, where it has one.
    pub(super) fn apply(&self, scene: &mut Scene) {
        let mut changed = false;
        for item in &mut scene.items {
            if item.id.kind != ItemKind::Device {
                continue;
            }
            if let Some((_, Outcome::Drawn(k))) = self.jobs.get(&item.id.raw) {
                item.shape = Shape::Part(*k);
                changed = true;
            }
        }
        if changed {
            scene.revision += 1;
        }
    }

    pub(super) fn counts(&self) -> Counts {
        let mut c = Counts::default();
        for (_, o) in self.jobs.values() {
            match o {
                Outcome::Waiting => c.waiting += 1,
                Outcome::Drawn(_) => c.drawn += 1,
                Outcome::Unusable => c.unusable += 1,
            }
        }
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_view3d::scene::{Item, ItemId, PlateFrame, Pose};

    fn scene() -> Scene {
        let mut s = Scene::new(PlateFrame {
            width: 100.0,
            height: 100.0,
        });
        for raw in [1, 2] {
            s.push(Item::new(
                ItemId::new(ItemKind::Device, raw),
                Shape::Box,
                Pose::IDENTITY,
                Vec3::new(10.0, 20.0, 30.0),
            ));
        }
        s
    }

    #[test]
    fn only_a_drawn_device_takes_its_model() {
        let mut m = Models::default();
        m.jobs.insert(1, (7, Outcome::Drawn(GeometryKey(7))));
        m.jobs.insert(2, (8, Outcome::Unusable));
        let mut s = scene();
        let before = s.revision;
        m.apply(&mut s);
        assert_eq!(s.items[0].shape, Shape::Part(GeometryKey(7)));
        assert_eq!(s.items[1].shape, Shape::Box);
        assert!(s.revision > before);
    }

    #[test]
    fn counts_and_text_tell_what_is_missing() {
        let mut m = Models::default();
        assert_eq!(m.counts().text(), None);
        m.jobs.insert(1, (1, Outcome::Drawn(GeometryKey(1))));
        m.jobs.insert(2, (2, Outcome::Waiting));
        m.jobs.insert(3, (3, Outcome::Unusable));
        assert_eq!(
            m.counts().text().as_deref(),
            Some(
                "Maker models: 1 of 3 devices, 1 still building, 1 not usable (see the Parts window)"
            )
        );
        assert_eq!(m.counts().waiting, 1);
    }

    #[test]
    fn a_converted_mesh_keeps_its_bounds_and_levels() {
        let g = part3d::box_mesh([10.0, 20.0, 30.0]);
        let v = convert(&g);
        assert_eq!(v.lods.len(), g.lods.len());
        assert!(
            (v.lods[0].bounds.size() - Vec3::new(10.0, 20.0, 30.0))
                .abs()
                .max_element()
                < 1e-3
        );
        assert_eq!(v.lods[0].triangles(), g.lods[0].triangle_count());
    }
}
