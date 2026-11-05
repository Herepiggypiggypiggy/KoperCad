//! Reading the user's own crane model (a STEP file) for the crane picture: every solid becomes a
//! part with the name the file gives it, a mesh of a few thousand triangles and a role guessed from
//! the name. Runs on a worker thread (a crane file can be tens of megabytes); the window polls the
//! channel each frame. Nothing is copied into the project: the project keeps only the path.

use novo_interop::part3d::crane_roles::{CraneRole, role_of};
use novo_interop::part3d::mesh::{MeshBudget, levels};
use novo_interop::part3d::{BuildError, StepOptions, build::placed_parts};
use novo_view3d::crane_model::{CraneModel, PartRole, RawPart};
use novo_view3d::math::Aabb;
use novo_view3d::mesh::MeshData;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

/// The triangles all solids of a model may have together once simplified. A crane with a hundred
/// solids stays smooth in software rendering at this size.
const TOTAL_TRIANGLES: usize = 180_000;
/// The fewest triangles a large solid is cut to.
const PART_FLOOR: usize = 400;
/// How long a file may take to read.
const TIME_LIMIT: Duration = Duration::from_secs(150);

/// The picture's role for the reader's.
pub(super) fn role_from(r: CraneRole) -> PartRole {
    match r {
        CraneRole::Wheel => PartRole::Wheel,
        CraneRole::Drum => PartRole::Drum,
        CraneRole::HookBlock => PartRole::HookBlock,
        CraneRole::Hook => PartRole::Hook,
        CraneRole::Girder => PartRole::Girder,
        CraneRole::Rail => PartRole::Rail,
        CraneRole::Motor => PartRole::Motor,
        CraneRole::Gearbox => PartRole::Gearbox,
        CraneRole::Other => PartRole::Other,
    }
}

/// The role each solid's name suggests.
pub(super) fn guess_roles(model: &CraneModel) -> Vec<PartRole> {
    model
        .parts
        .iter()
        .map(|p| role_from(role_of(&p.name)))
        .collect()
}

/// Reads the file into a model. `cancel` stops it early. The error is a sentence for the person.
pub(super) fn load(path: &Path, cancel: &AtomicBool) -> Result<CraneModel, String> {
    let name = path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let bytes = std::fs::read(path).map_err(|e| format!("{name}: {e}"))?;
    let text = String::from_utf8_lossy(&bytes);
    // A crane is metres long: the 0.05 mm chord tolerance made for terminals would give millions
    // of triangles. Start coarse-ish and go coarser when the file is still too detailed.
    let mut parts = None;
    let mut last = BuildError::NoGeometry;
    for tol in [1.0, 3.0, 8.0] {
        let opts = StepOptions {
            chord_tol_mm: tol,
            deadline: Some(Instant::now() + TIME_LIMIT),
            ..StepOptions::default()
        };
        match placed_parts(&text, &opts, cancel) {
            Ok((p, _)) => {
                parts = Some(p);
                break;
            }
            Err(BuildError::TooComplex) => last = BuildError::TooComplex,
            Err(e) => return Err(format!("{name}: {e}")),
        }
    }
    let Some(parts) = parts else {
        return Err(format!("{name}: {last}"));
    };
    let total: usize = parts.iter().map(|p| p.soup.tris.len()).sum();
    let mut out = Vec::with_capacity(parts.len());
    for p in parts {
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        let own = p.soup.tris.len();
        let share = (TOTAL_TRIANGLES as f64 * own as f64 / total.max(1) as f64) as usize;
        let budget = share.max(PART_FLOOR).min(own.max(12)) as u32;
        let lods = levels(
            p.soup.pos,
            p.soup.tris,
            MeshBudget {
                max_triangles: budget,
            },
        );
        let m = &lods[0];
        out.push(RawPart {
            name: p.name,
            mesh: MeshData {
                positions: m.positions.clone(),
                normals: m.normals.clone(),
                indices: m.indices.clone(),
                edges: m.edges.clone(),
                bounds: Aabb::EMPTY,
            },
        });
    }
    Ok(CraneModel::new(out))
}

/// A model being read on a worker thread.
pub(super) struct Loading {
    pub path: PathBuf,
    cancel: Arc<AtomicBool>,
    rx: mpsc::Receiver<Result<CraneModel, String>>,
}

impl Loading {
    /// Starts reading `path` in the background; `wake` is called when the work is done so the
    /// window repaints.
    pub fn start(path: PathBuf, wake: impl Fn() + Send + 'static) -> Self {
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let (p, c) = (path.clone(), cancel.clone());
        std::thread::spawn(move || {
            let result = load(&p, &c);
            let _ = tx.send(result);
            wake();
        });
        Self { path, cancel, rx }
    }

    /// The result, once the worker is done.
    pub fn poll(&self) -> Option<Result<CraneModel, String>> {
        match self.rx.try_recv() {
            Ok(r) => Some(r),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err("The reading stopped".into())),
        }
    }
}

impl Drop for Loading {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../qa/fixtures/crane-model/workshop-crane.stp")
    }

    #[test]
    fn the_workshop_crane_splits_into_named_parts_with_roles() {
        let m = load(&fixture(), &AtomicBool::new(false)).unwrap();
        let names: Vec<&str> = m.parts.iter().map(|p| p.name.as_str()).collect();
        assert!(
            names.contains(&"Rope drum") && names.contains(&"Hook block"),
            "{names:?}"
        );
        let roles = guess_roles(&m);
        let count = |r: PartRole| roles.iter().filter(|x| **x == r).count();
        assert_eq!(count(PartRole::Wheel), 8, "{names:?}");
        assert_eq!(count(PartRole::Drum), 1);
        assert_eq!(count(PartRole::HookBlock), 1);
        assert_eq!(count(PartRole::Hook), 1);
        assert_eq!(count(PartRole::Rail), 2, "{names:?}");
        assert_eq!(count(PartRole::Girder), 1);
        assert_eq!(count(PartRole::Motor), 1);
        assert_eq!(count(PartRole::Gearbox), 1);
        // Every part has triangles and the model is metres long.
        assert!(m.parts.iter().all(|p| p.mesh.triangles() > 0));
        assert!(m.bounds().size().x > 11_000.0, "{:?}", m.bounds());
    }

    #[test]
    fn a_missing_file_or_a_file_that_is_not_step_says_so() {
        let e = load(Path::new("/nonexistent/crane.stp"), &AtomicBool::new(false)).unwrap_err();
        assert!(e.contains("crane.stp"), "{e}");
        let dir = std::env::temp_dir().join("kopercad-crane-model-test");
        std::fs::create_dir_all(&dir).unwrap();
        let bad = dir.join("not-step.stp");
        std::fs::write(&bad, "hello").unwrap();
        let e = load(&bad, &AtomicBool::new(false)).unwrap_err();
        assert!(e.contains("not a STEP file"), "{e}");
    }

    #[test]
    fn a_cancelled_reading_stops() {
        let e = load(&fixture(), &AtomicBool::new(true)).unwrap_err();
        assert!(e.to_lowercase().contains("cancel"), "{e}");
    }

    /// Opt-in check on a real crane file: `KOPERCAD_CRANE_STEP=<file> cargo test -p novo-app
    /// --lib real_crane_file -- --ignored --nocapture`. Prints how long it took and what came out.
    #[test]
    #[ignore = "needs a real crane file"]
    fn real_crane_file() {
        let Ok(path) = std::env::var("KOPERCAD_CRANE_STEP") else {
            return;
        };
        let t = Instant::now();
        let m = load(Path::new(&path), &AtomicBool::new(false)).unwrap();
        let roles = guess_roles(&m);
        println!(
            "{} parts, {} triangles, {:.1} s, size {:?}",
            m.parts.len(),
            m.triangles(),
            t.elapsed().as_secs_f32(),
            m.bounds().size()
        );
        for r in PartRole::ALL {
            println!(
                "{}: {}",
                r.label(),
                roles.iter().filter(|x| **x == r).count()
            );
        }
    }
}
