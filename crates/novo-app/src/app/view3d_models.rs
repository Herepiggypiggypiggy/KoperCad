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
use novo_interop::part3d::{self, MeshOrigin, PartRotation, Quality};
use novo_parts::{FileKind, LocalLibrary, PartKey, SourceRecord};
use novo_view3d::scene::{ItemKind, Shape};
use novo_view3d::{Aabb, GeometryKey, MeshData, MeshLibrary, PartGeometry, Scene};

/// How many unused meshes the library may hold before it is pruned.
const PRUNE_SLACK: usize = 16;

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

/// A quarter turn the person asks for on a maker model that arrived lying on
/// its side or facing the wrong way. The turns are about the model's own
/// axes (x width, y height, z depth towards the door) and are kept with the
/// part in the library, so every device of the part and every project use them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Turn {
    /// About the width axis: tips the top towards or away from the door.
    Tip,
    /// About the height axis: turns the front round to the side or back.
    Spin,
    /// About the depth axis: lays the model on its side or stands it up.
    Roll,
    /// Back to the way the file came.
    Reset,
}

/// The quarter turns after `how`, each 0 to 3.
pub(super) fn turned(now: [u8; 3], how: Turn) -> [u8; 3] {
    let mut t = now.map(|q| q % 4);
    match how {
        Turn::Tip => t[0] = (t[0] + 1) % 4,
        Turn::Spin => t[1] = (t[1] + 1) % 4,
        Turn::Roll => t[2] = (t[2] + 1) % 4,
        Turn::Reset => t = [0; 3],
    }
    t
}

/// What the Position box shows about the selected device's maker model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ModelRow {
    /// Quarter turns kept for the part, about x, y, z.
    pub turns: [u8; 3],
    /// A line for the person: drawn, still building, or not usable.
    pub status: &'static str,
}

/// The library as read: the records file's modified time, the STEP record of
/// each part and the quarter turns kept for each part (by their library key).
type LibraryRead = (
    Option<std::time::SystemTime>,
    BTreeMap<PartKey, SourceRecord>,
    BTreeMap<String, [u8; 3]>,
);

/// The models the open enclosure's devices ask for.
#[derive(Default)]
pub(super) struct Models {
    service: Option<GeometryService>,
    /// Service key per device (raw id) and what became of it.
    jobs: BTreeMap<u64, (u64, Outcome)>,
    /// The part of each device that has a model, and its stored turns.
    parts: BTreeMap<u64, (PartKey, [u8; 3])>,
    /// Bumped whenever a model arrives, so the scene is built again.
    pub(super) revision: u64,
    /// The STEP records as last read, and the records file's modified time
    /// they were read at, so a rebuild does not read the library again.
    records: Option<LibraryRead>,
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
        let stamp = std::fs::metadata(library.root().join("records.json"))
            .and_then(|m| m.modified())
            .ok();
        if !matches!(&self.records, Some((at, _, _)) if *at == stamp && stamp.is_some()) {
            self.records = Some((
                stamp,
                step_records(&library),
                library.turns().unwrap_or_default(),
            ));
        }
        let records = self
            .records
            .as_ref()
            .map(|r| &r.1)
            .cloned()
            .unwrap_or_default();
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
        let turns = self
            .records
            .as_ref()
            .map(|r| r.2.clone())
            .unwrap_or_default();
        let mut wanted = BTreeMap::new();
        let mut parts = BTreeMap::new();
        for (raw, part) in device_parts(project, id) {
            let part_key = PartKey::new(&part.manufacturer, &part.order_number);
            let Some(record) = records.get(&part_key) else {
                continue;
            };
            let kept = LocalLibrary::turns_in(&turns, &part_key);
            parts.insert(raw, (part_key, kept));
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
            let key = service.request_rotated(job, PartRotation::new(kept[0], kept[1], kept[2]));
            let outcome = match self.jobs.get(&raw) {
                Some((k, o)) if *k == key => *o,
                _ => Outcome::Waiting,
            };
            wanted.insert(raw, (key, outcome));
        }
        self.jobs = wanted;
        self.parts = parts;
    }

    /// The model of `device` for the Position box: `None` when its part has
    /// no STEP file.
    pub(super) fn row(&self, device: u64) -> Option<ModelRow> {
        let (_, turns) = self.parts.get(&device)?;
        let status = match self.jobs.get(&device)?.1 {
            Outcome::Waiting => "Building the maker model",
            Outcome::Drawn(_) => "Maker model",
            Outcome::Unusable => "Maker model not usable (see the Parts window)",
        };
        Some(ModelRow {
            turns: *turns,
            status,
        })
    }

    /// Turns the model of `device`'s part and keeps the turns in the library.
    /// The next scene build asks for the turned model. Returns the message
    /// to show when the turns could not be kept.
    pub(super) fn turn(&mut self, device: u64, how: Turn) -> Option<String> {
        let (key, now) = self.parts.get(&device)?.clone();
        let Some(library) = LocalLibrary::open_default() else {
            return Some("Could not open the parts library to keep the model's turns".into());
        };
        let to = turned(now, how);
        if let Err(e) = library.set_turns(&key, to) {
            return Some(format!("Could not keep the model's turns: {e}"));
        }
        // Read again, and build the scene again with the turned model asked for.
        self.records = None;
        self.revision += 1;
        None
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
                // `None` after a request means the service forgot the key (a
                // Retry elsewhere): not waiting for it forever.
                GeometryState::Ready(_) | GeometryState::Failed(_) | GeometryState::None => {
                    *outcome = Outcome::Unusable;
                }
                GeometryState::Pending => {}
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

    /// The meshes the models in use need, to keep when the library is pruned.
    pub(super) fn drawn_keys(&self) -> impl Iterator<Item = GeometryKey> + '_ {
        self.jobs.values().filter_map(|(_, o)| match o {
            Outcome::Drawn(k) => Some(*k),
            _ => None,
        })
    }

    /// Drops meshes no item of `scene` and no model in use needs, once the
    /// library holds `PRUNE_SLACK` more than that (so hiding and showing a
    /// part does not rebuild its mesh each time). Returns how many it dropped.
    pub(super) fn prune(&self, library: &mut Arc<MeshLibrary>, scene: &Scene) -> usize {
        let mut keep: std::collections::HashSet<GeometryKey> = self.drawn_keys().collect();
        keep.extend(scene.items.iter().filter_map(|i| match i.shape {
            Shape::Part(k) => Some(k),
            _ => None,
        }));
        if library.len() <= keep.len() + PRUNE_SLACK {
            return 0;
        }
        Arc::make_mut(library).retain(|k| keep.contains(&k))
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
    fn the_library_is_pruned_to_what_is_in_use_once_it_has_slack() {
        let part = || PartGeometry {
            lods: vec![novo_view3d::mesh::unit_box()],
        };
        let mut m = Models::default();
        m.jobs.insert(9, (900, Outcome::Drawn(GeometryKey(900))));
        let mut s = scene();
        s.items[0].shape = Shape::Part(GeometryKey(100));
        let mut lib = MeshLibrary::new();
        for k in [100, 900] {
            lib.insert(GeometryKey(k), part());
        }
        let mut lib = Arc::new(lib);
        for k in 0..PRUNE_SLACK as u64 {
            Arc::make_mut(&mut lib).insert(GeometryKey(1000 + k), part());
        }
        assert_eq!(m.prune(&mut lib, &s), 0, "within the slack");
        Arc::make_mut(&mut lib).insert(GeometryKey(5000), part());
        assert_eq!(m.prune(&mut lib, &s), PRUNE_SLACK + 1);
        assert!(lib.get(GeometryKey(100)).is_some(), "an item's mesh stays");
        assert!(lib.get(GeometryKey(900)).is_some(), "a drawn model stays");
        assert_eq!(lib.len(), 2);
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
    fn each_turn_adds_a_quarter_about_its_own_axis_and_reset_clears_them() {
        let mut t = [0; 3];
        t = turned(t, Turn::Tip);
        assert_eq!(t, [1, 0, 0]);
        t = turned(t, Turn::Roll);
        t = turned(t, Turn::Roll);
        assert_eq!(t, [1, 0, 2]);
        t = turned(t, Turn::Spin);
        assert_eq!(t, [1, 1, 2]);
        // Four quarters are a full turn: back to none about that axis.
        for _ in 0..3 {
            t = turned(t, Turn::Tip);
        }
        assert_eq!(t, [0, 1, 2]);
        assert_eq!(turned(t, Turn::Reset), [0; 3]);
        // Stored counts past 3 are taken modulo 4.
        assert_eq!(turned([5, 0, 0], Turn::Tip), [2, 0, 0]);
    }

    #[test]
    fn the_row_names_what_became_of_the_model_and_hides_for_a_part_without_one() {
        let mut m = Models::default();
        assert_eq!(m.row(1), None);
        m.parts.insert(1, (PartKey::new("A", "1"), [0, 1, 0]));
        // No job yet: nothing to say.
        assert_eq!(m.row(1), None);
        m.jobs.insert(1, (1, Outcome::Waiting));
        assert_eq!(m.row(1).unwrap().status, "Building the maker model");
        assert_eq!(m.row(1).unwrap().turns, [0, 1, 0]);
        m.jobs.insert(1, (1, Outcome::Drawn(GeometryKey(1))));
        assert_eq!(m.row(1).unwrap().status, "Maker model");
        m.jobs.insert(1, (1, Outcome::Unusable));
        assert!(m.row(1).unwrap().status.contains("not usable"));
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
