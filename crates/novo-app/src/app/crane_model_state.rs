//! The crane model slot of the Hoist in 3D window: which STEP file the person asked for, the model
//! read from it, and the plan that stands it on the floor. The window asks for a file (or the
//! project does, when it opens), the slot reads it on a worker thread, and the picture shows the
//! model once it is there. Everything that follows from the file (the guessed roles, the plan) is
//! rebuilt from `want`, so undo, a reopened project and the window all agree.

use super::crane_model_load::{Loading, guess_roles};
use super::crane_setup::ModelRef;
use novo_view3d::crane_model::{
    CraneModel, ModelPlan, Orientation, PartRole, hook_bounds, plan, scene,
};
use novo_view3d::math::Aabb;
use novo_view3d::mesh::MeshLibrary;
use novo_view3d::scene::Scene;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A model read and placed.
struct Loaded {
    path: String,
    raw: Arc<CraneModel>,
    guess: Vec<PartRole>,
    /// What the plan was made with.
    orientation: Orientation,
    roles: Vec<PartRole>,
    plan: ModelPlan,
    library: Arc<MeshLibrary>,
    scene: Arc<Scene>,
    /// Counts the plans of the slot, so the window knows when to hand the renderer a new scene.
    generation: u64,
}

/// What the slot has to say to the window.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Status {
    /// No model asked for.
    None,
    /// Reading the file.
    Reading(String),
    /// The file could not be used: its name and the sentence.
    Failed(String, String),
    /// A model is shown.
    Ready {
        name: String,
        parts: usize,
        triangles: usize,
    },
}

/// One solid for the parts list.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PartRow {
    pub name: String,
    pub role: PartRole,
    pub guessed: PartRole,
    /// Size in the picture's axes (along the runway, up, across), mm.
    pub size_mm: [f32; 3],
}

#[derive(Default)]
pub(super) struct ModelSlot {
    want: Option<ModelRef>,
    loaded: Option<Loaded>,
    loading: Option<Loading>,
    failed: Option<(String, String)>,
    generation: u64,
    /// The solid the person points at in the parts list (index into the listed rows).
    highlight: Option<usize>,
    pub(super) parts_open: bool,
}

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map_or_else(|| path.to_owned(), |n| n.to_string_lossy().into_owned())
}

impl ModelSlot {
    /// What the picture is asked to show.
    pub(super) fn want(&self) -> Option<&ModelRef> {
        self.want.as_ref()
    }

    /// Takes what the project holds (a project opened, an undo).
    pub(super) fn set_want(&mut self, want: Option<ModelRef>) {
        if self.want.as_ref().map(|m| &m.path) != want.as_ref().map(|m| &m.path) {
            self.failed = None;
        }
        self.want = want;
    }

    /// The person picked a file.
    pub(super) fn pick(&mut self, path: &Path) {
        self.failed = None;
        self.want = Some(ModelRef::new(path.to_string_lossy().into_owned()));
    }

    /// Back to the generic crane.
    pub(super) fn clear(&mut self) {
        self.want = None;
    }

    /// Whether a model is on screen in place of the generic crane.
    pub(super) fn shown(&self) -> bool {
        match (&self.want, &self.loaded) {
            (Some(w), Some(l)) => w.path == l.path,
            _ => false,
        }
    }

    pub(super) fn status(&self) -> Status {
        let Some(want) = &self.want else {
            return Status::None;
        };
        if let Some(l) = self.loaded.as_ref().filter(|l| l.path == want.path) {
            return Status::Ready {
                name: file_name(&l.path),
                parts: l.plan.placed.len(),
                triangles: l.raw.triangles(),
            };
        }
        if let Some((path, why)) = &self.failed
            && *path == want.path
        {
            return Status::Failed(file_name(path), why.clone());
        }
        Status::Reading(file_name(&want.path))
    }

    /// Moves the slot towards what is wanted: starts a reading, takes a finished one, re-places the
    /// model when its orientation or roles changed. Returns whether the picture changed. `wake`
    /// repaints the window when a reading finishes.
    pub(super) fn update(&mut self, wake: impl Fn() + Send + 'static) -> bool {
        let Some(want) = self.want.clone() else {
            let had = self.loaded.take().is_some();
            self.loading = None;
            if had {
                self.generation += 1;
            }
            return had;
        };
        let mut changed = false;
        if let Some(l) = &self.loading
            && let Some(done) = l.poll()
        {
            let path = l.path.to_string_lossy().into_owned();
            self.loading = None;
            match done {
                Ok(model) if path == want.path => {
                    self.install(path, model, &want);
                    changed = true;
                }
                Ok(_) => {}
                Err(why) => self.failed = Some((path, why)),
            }
        }
        if self.loaded.as_ref().is_none_or(|l| l.path != want.path)
            && self
                .loading
                .as_ref()
                .is_none_or(|l| l.path.to_string_lossy() != want.path.as_str())
            && self.failed.as_ref().is_none_or(|(p, _)| *p != want.path)
        {
            self.loading = Some(Loading::start(PathBuf::from(&want.path), wake));
        }
        if let Some(l) = &mut self.loaded
            && l.path == want.path
        {
            let roles = effective_roles(&want, &l.guess);
            if l.orientation != want.orientation || l.roles != roles {
                self.generation += 1;
                l.replan(want.orientation, roles, self.highlight, self.generation);
                changed = true;
            }
        }
        changed
    }

    fn install(&mut self, path: String, model: CraneModel, want: &ModelRef) {
        let raw = Arc::new(model);
        let guess = guess_roles(&raw);
        let roles = effective_roles(want, &guess);
        self.generation += 1;
        let p = plan(&raw, want.orientation, &roles);
        self.loaded = Some(Loaded::new(
            path,
            raw,
            guess,
            want.orientation,
            roles,
            p,
            self.highlight,
            self.generation,
        ));
    }

    /// The scene and its library, with the number that changes when the scene does.
    pub(super) fn scene(&self) -> Option<(u64, Arc<Scene>, Arc<MeshLibrary>)> {
        let l = self.loaded.as_ref().filter(|_| self.shown())?;
        Some((l.generation, l.scene.clone(), l.library.clone()))
    }

    /// What a camera frames to see the hook of the model.
    pub(super) fn hook_bounds(&self) -> Option<Aabb> {
        let l = self.loaded.as_ref().filter(|_| self.shown())?;
        Some(hook_bounds(&l.plan))
    }

    /// The solids with their roles, in file order (solids without triangles are not listed).
    pub(super) fn rows(&self) -> Vec<PartRow> {
        let Some(l) = self.loaded.as_ref().filter(|_| self.shown()) else {
            return Vec::new();
        };
        let mut rows = Vec::new();
        for (i, part) in l.raw.parts.iter().enumerate() {
            if part.mesh.triangles() == 0 {
                continue;
            }
            let b = rotated_size(part.mesh.bounds, l.orientation);
            rows.push(PartRow {
                name: part.name.clone(),
                role: l.roles.get(i).copied().unwrap_or(PartRole::Other),
                guessed: l.guess.get(i).copied().unwrap_or(PartRole::Other),
                size_mm: b,
            });
        }
        rows
    }

    /// The index in file order of the `n`th listed solid.
    fn file_index(&self, n: usize) -> Option<usize> {
        let l = self.loaded.as_ref()?;
        l.raw
            .parts
            .iter()
            .enumerate()
            .filter(|(_, p)| p.mesh.triangles() > 0)
            .nth(n)
            .map(|(i, _)| i)
    }

    /// Sets the role of the `n`th listed solid.
    pub(super) fn set_role(&mut self, n: usize, role: PartRole) {
        let Some(i) = self.file_index(n) else { return };
        let Some(l) = &self.loaded else { return };
        let Some(want) = &mut self.want else { return };
        let mut roles = effective_roles(want, &l.guess);
        if let Some(r) = roles.get_mut(i) {
            *r = role;
        }
        want.roles = if roles == l.guess { Vec::new() } else { roles };
    }

    /// Gives every solid the role its name suggests.
    pub(super) fn reset_roles(&mut self) {
        if let Some(w) = &mut self.want {
            w.roles.clear();
        }
    }

    /// Points at the `n`th listed solid (or at none): it shows in the highlight colour.
    pub(super) fn set_highlight(&mut self, n: Option<usize>) {
        if self.highlight == n {
            return;
        }
        self.highlight = n;
        if let Some(l) = &mut self.loaded {
            self.generation += 1;
            l.scene = Arc::new(scene(&l.plan, n));
            l.generation = self.generation;
        }
    }

    pub(super) fn set_orientation(&mut self, o: Orientation) {
        if let Some(w) = &mut self.want {
            w.orientation = o;
        }
    }
}

fn effective_roles(want: &ModelRef, guess: &[PartRole]) -> Vec<PartRole> {
    if want.roles.len() == guess.len() {
        want.roles.clone()
    } else {
        guess.to_vec()
    }
}

/// The size of a box turned to the picture's axes.
fn rotated_size(b: Aabb, o: Orientation) -> [f32; 3] {
    let q = o.rotation();
    let mut out = Aabb::EMPTY;
    for c in b.corners() {
        out.extend(q * c);
    }
    out.size().to_array()
}

impl Loaded {
    #[allow(clippy::too_many_arguments)]
    fn new(
        path: String,
        raw: Arc<CraneModel>,
        guess: Vec<PartRole>,
        orientation: Orientation,
        roles: Vec<PartRole>,
        plan: ModelPlan,
        highlight: Option<usize>,
        generation: u64,
    ) -> Self {
        let scene = Arc::new(scene(&plan, highlight));
        let library = Arc::new(plan.library.clone());
        Self {
            path,
            raw,
            guess,
            orientation,
            roles,
            plan,
            library,
            scene,
            generation,
        }
    }

    fn replan(
        &mut self,
        orientation: Orientation,
        roles: Vec<PartRole>,
        highlight: Option<usize>,
        generation: u64,
    ) {
        let p = plan(&self.raw, orientation, &roles);
        self.scene = Arc::new(scene(&p, highlight));
        self.library = Arc::new(p.library.clone());
        self.plan = p;
        self.orientation = orientation;
        self.roles = roles;
        self.generation = generation;
    }
}

/// Loads a model right away for tests.
#[cfg(test)]
pub(super) fn load_now(slot: &mut ModelSlot, path: &Path) {
    use super::crane_model_load::load;
    use std::sync::atomic::AtomicBool;
    slot.pick(path);
    let model = load(path, &AtomicBool::new(false)).unwrap();
    let want = slot.want.clone().unwrap();
    slot.install(path.to_string_lossy().into_owned(), model, &want);
}

#[cfg(test)]
mod tests {
    use super::super::crane_model_load::load;
    use super::*;
    use novo_view3d::crane_model::UpAxis;

    fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../qa/fixtures/crane-model/workshop-crane.stp")
    }

    #[test]
    fn a_picked_file_is_read_in_the_background_and_then_shown() {
        let mut slot = ModelSlot::default();
        assert_eq!(slot.status(), Status::None);
        slot.pick(&fixture());
        assert!(matches!(slot.status(), Status::Reading(_)));
        assert!(!slot.shown());
        let mut tries = 0;
        while !slot.shown() && tries < 600 {
            slot.update(|| {});
            std::thread::sleep(std::time::Duration::from_millis(50));
            tries += 1;
        }
        assert!(slot.shown(), "{:?}", slot.status());
        let Status::Ready { name, parts, .. } = slot.status() else {
            panic!("{:?}", slot.status())
        };
        assert_eq!(name, "workshop-crane.stp");
        assert_eq!(parts, 23);
        let (_, scene, library) = slot.scene().unwrap();
        assert!(scene.items.len() > parts);
        assert!(library.len() >= parts);
    }

    #[test]
    fn a_file_that_cannot_be_read_is_reported_once_and_not_retried() {
        let mut slot = ModelSlot::default();
        slot.pick(Path::new("/nonexistent/crane.stp"));
        for _ in 0..200 {
            slot.update(|| {});
            if matches!(slot.status(), Status::Failed(..)) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(
            matches!(slot.status(), Status::Failed(..)),
            "{:?}",
            slot.status()
        );
        assert!(!slot.shown() && slot.scene().is_none());
        // Another frame does not start a second reading.
        slot.update(|| {});
        assert!(slot.loading.is_none());
        // Picking something else forgets the failure.
        slot.pick(&fixture());
        assert!(matches!(slot.status(), Status::Reading(_)));
    }

    #[test]
    fn changing_a_role_or_the_orientation_places_the_model_again() {
        let mut slot = ModelSlot::default();
        load_now(&mut slot, &fixture());
        assert!(!slot.update(|| {}));
        let (g0, ..) = slot.scene().unwrap();
        let rows = slot.rows();
        let i = rows.iter().position(|r| r.name == "Rope drum").unwrap();
        assert_eq!(rows[i].role, PartRole::Drum);
        slot.set_role(i, PartRole::Other);
        assert!(slot.update(|| {}));
        assert_eq!(slot.rows()[i].role, PartRole::Other);
        assert_eq!(slot.rows()[i].guessed, PartRole::Drum);
        let (g1, ..) = slot.scene().unwrap();
        assert!(g1 > g0);
        assert_eq!(slot.want().unwrap().roles.len(), 23);
        // Back to the guess: nothing is stored.
        slot.set_role(i, PartRole::Drum);
        assert!(slot.want().unwrap().roles.is_empty());
        slot.set_role(i, PartRole::Motor);
        slot.reset_roles();
        assert!(slot.want().unwrap().roles.is_empty());
        slot.update(|| {});
        assert_eq!(slot.rows()[i].role, PartRole::Drum);
        // Y up lays the model on its side: the long size is no longer along x only.
        let before = slot.rows()[0].size_mm;
        slot.set_orientation(Orientation {
            up: UpAxis::Y,
            turns: 0,
        });
        assert!(slot.update(|| {}));
        assert_ne!(slot.rows()[0].size_mm, before);
    }

    #[test]
    fn pointing_at_a_row_paints_that_solid_and_pointing_away_paints_it_back() {
        use novo_view3d::crane_model::HIGHLIGHT;
        let mut slot = ModelSlot::default();
        load_now(&mut slot, &fixture());
        let (g0, ..) = slot.scene().unwrap();
        let painted = |slot: &ModelSlot| {
            slot.scene()
                .unwrap()
                .1
                .items
                .iter()
                .filter(|i| i.tint == Some(HIGHLIGHT))
                .count()
        };
        assert_eq!(painted(&slot), 0);
        slot.set_highlight(Some(3));
        assert_eq!(painted(&slot), 1);
        assert!(slot.scene().unwrap().0 > g0);
        // The same row again changes nothing; moving away paints it back.
        let g1 = slot.scene().unwrap().0;
        slot.set_highlight(Some(3));
        assert_eq!(slot.scene().unwrap().0, g1);
        slot.set_highlight(None);
        assert_eq!(painted(&slot), 0);
        // It stays through a re-placement.
        slot.set_highlight(Some(5));
        slot.set_role(0, PartRole::Girder);
        slot.update(|| {});
        assert_eq!(painted(&slot), 1);
    }

    #[test]
    fn clearing_goes_back_to_the_generic_crane() {
        let mut slot = ModelSlot::default();
        load_now(&mut slot, &fixture());
        assert!(slot.shown());
        slot.clear();
        assert!(!slot.shown());
        assert!(slot.update(|| {}));
        assert!(slot.scene().is_none() && slot.status() == Status::None);
    }

    #[test]
    fn a_role_list_of_the_wrong_length_falls_back_to_the_guess() {
        let mut slot = ModelSlot::default();
        let mut want = ModelRef::new(fixture().to_string_lossy().into_owned());
        want.roles = vec![PartRole::Wheel, PartRole::Wheel];
        slot.set_want(Some(want));
        let model = load(&fixture(), &std::sync::atomic::AtomicBool::new(false)).unwrap();
        let want = slot.want.clone().unwrap();
        slot.install(want.path.clone(), model, &want);
        let rows = slot.rows();
        assert!(rows.iter().all(|r| r.role == r.guessed));
    }
}
