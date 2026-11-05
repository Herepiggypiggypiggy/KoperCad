//! "Use my crane model": a STEP file of the user's own crane read in the background and swapped
//! into the Hoist in 3D picture (hook and hoist). The file is only read: it is not copied into the
//! project and not saved anywhere, so the choice is gone when the window's session ends.

use eframe::egui;
use glam::Vec3;
use novo_interop::part3d::build::{PlacedPart, StepOptions, placed_parts};
use novo_interop::part3d::crane_model::{CraneModel, Placement, UpAxis, crane_model};
use novo_view3d::crane_user::{Slot, UserCrane, UserPiece, key};
use novo_view3d::mesh::{MeshData, PartGeometry};
use novo_view3d::{Aabb, MeshLibrary};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// What a finished read gives back.
struct Done {
    parts: Result<Arc<Vec<PlacedPart>>, String>,
    model: Option<CraneModel>,
}

/// The state of the user's crane model in the window.
#[derive(Default)]
pub(super) struct UserModel {
    /// The chosen file.
    pub file: Option<PathBuf>,
    pub placement: Placement,
    /// Its parts, kept so that a change of placement does not read the file again.
    parts: Option<Arc<Vec<PlacedPart>>>,
    job: Option<Receiver<Done>>,
    /// Counts the models put in the library, so a new one gets new keys.
    token: u32,
    /// The pieces the picture is told to use.
    pub crane: Option<UserCrane>,
    /// One line about the result: what went in, or why nothing did.
    pub status: Option<String>,
}

impl UserModel {
    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    /// Reads `file` (or, when only the placement changed, reuses its parts).
    pub fn start(&mut self, file: PathBuf, ctx: &egui::Context) {
        if self.file.as_ref() != Some(&file) {
            self.parts = None;
        }
        self.file = Some(file.clone());
        self.status = None;
        let (tx, rx) = channel();
        self.job = Some(rx);
        let (known, placement) = (self.parts.clone(), self.placement);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let done = run(&file, known, placement);
            let _ = tx.send(done);
            ctx.request_repaint();
        });
    }

    /// Re-sorts the parts for a changed placement.
    pub fn restart(&mut self, ctx: &egui::Context) {
        if let Some(f) = self.file.clone() {
            self.start(f, ctx);
        }
    }

    /// Puts a finished read into the library. Returns whether the picture changed.
    pub fn poll(&mut self, library: &mut MeshLibrary) -> bool {
        let Some(rx) = &self.job else { return false };
        let Ok(done) = rx.try_recv() else {
            return false;
        };
        self.job = None;
        match done.parts {
            Err(why) => {
                self.crane = None;
                self.status = Some(why);
            }
            Ok(parts) => {
                self.parts = Some(parts);
                let model = done.model.unwrap_or_default();
                self.token += 1;
                self.crane = model.is_usable().then_some(UserCrane {
                    hook: model
                        .hook
                        .as_ref()
                        .map(|p| install(library, Slot::Hook, self.token, p)),
                    hoist: model
                        .hoist
                        .as_ref()
                        .map(|p| install(library, Slot::Hoist, self.token, p)),
                });
                self.status = Some(summary(&model));
            }
        }
        true
    }

    pub fn clear(&mut self) {
        *self = UserModel {
            token: self.token,
            ..UserModel::default()
        };
    }
}

fn run(file: &PathBuf, known: Option<Arc<Vec<PlacedPart>>>, placement: Placement) -> Done {
    let parts = match known {
        Some(p) => p,
        None => {
            let text = match std::fs::read_to_string(file) {
                Ok(t) => t,
                Err(_) => {
                    return Done {
                        parts: Err("The file could not be read".into()),
                        model: None,
                    };
                }
            };
            let opts = StepOptions {
                chord_tol_mm: 0.3,
                deadline: Some(Instant::now() + Duration::from_secs(120)),
                ..StepOptions::default()
            };
            match placed_parts(&text, &opts, &AtomicBool::new(false)) {
                Ok((p, _)) => Arc::new(p),
                Err(e) => {
                    return Done {
                        parts: Err(e.to_string()),
                        model: None,
                    };
                }
            }
        }
    };
    let model = crane_model(&parts, placement);
    Done {
        parts: Ok(parts),
        model: Some(model),
    }
}

fn summary(m: &CraneModel) -> String {
    if !m.is_usable() {
        return format!("No hook, drum, motor or gearbox found in {} parts", m.total);
    }
    let used =
        |p: &Option<novo_interop::part3d::crane_model::Piece>| p.as_ref().map_or(0, |p| p.parts);
    format!(
        "{} of {} parts used",
        used(&m.hook) + used(&m.hoist),
        m.total
    )
}

fn install(
    library: &mut MeshLibrary,
    slot: Slot,
    token: u32,
    p: &novo_interop::part3d::crane_model::Piece,
) -> UserPiece {
    let k = key(slot, token);
    let lods = p
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
        .collect();
    library.insert(k, PartGeometry { lods });
    UserPiece {
        key: k,
        size: Vec3::from_array(p.size_mm),
    }
}

/// The label of an up-axis choice.
pub(super) fn up_label(u: UpAxis) -> &'static str {
    match u {
        UpAxis::Z => "Z up",
        UpAxis::Y => "Y up",
    }
}
