//! Loads part geometry in the background so the 3D view never waits.
//!
//! The view asks [`GeometryService::geometry`] every frame: it is a map
//! lookup. A request starts a job on a worker thread that reads the STEP
//! file (from the cache when it can), and calls `wake` when the state
//! changes so the view repaints once. A job that runs longer than the time
//! limit ends as `Failed`; it is not retried until someone calls
//! [`GeometryService::forget`].

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use super::cache::{MeshCache, cached_part_geometry};
use super::mesh::{Fallback, MeshOrigin, PartGeometry, Quality};
use super::{fallback_box, geometry_from_step, geometry_key};

/// Import time allowed per model by the 3D design.
pub const TIME_LIMIT: Duration = Duration::from_secs(10);

/// What the view knows about a part's geometry right now.
#[derive(Debug, Clone)]
pub enum GeometryState {
    /// Nothing requested.
    None,
    /// A job is queued or running; draw the box.
    Pending,
    Ready(Arc<PartGeometry>),
    /// The model could not be used; draw the box and say why once.
    Failed(String),
}

/// A model to load.
#[derive(Debug, Clone)]
pub struct GeometryJob {
    pub part_id: String,
    /// SHA-256 of the model file, as the parts library records it.
    pub content_id: String,
    pub step_path: PathBuf,
    /// Width, height, depth of the part, for matching axes.
    pub size_mm: Option<[f64; 3]>,
}

struct Shared {
    states: Mutex<HashMap<u64, GeometryState>>,
    cache: Option<MeshCache>,
    quality: Quality,
    time_limit: Duration,
    stop: AtomicBool,
    wake: Box<dyn Fn() + Send + Sync>,
}

pub struct GeometryService {
    shared: Arc<Shared>,
    tx: Sender<(u64, GeometryJob)>,
}

impl GeometryService {
    /// Starts the worker thread. `wake` is called from it after every state
    /// change (the view asks egui for a repaint there).
    pub fn new(
        cache: Option<MeshCache>,
        quality: Quality,
        time_limit: Duration,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> GeometryService {
        let shared = Arc::new(Shared {
            states: Mutex::new(HashMap::new()),
            cache,
            quality,
            time_limit,
            stop: AtomicBool::new(false),
            wake: Box::new(wake),
        });
        let (tx, rx) = channel();
        let worker = shared.clone();
        std::thread::Builder::new()
            .name("kopercad-geometry".into())
            .spawn(move || run(&worker, &rx))
            .ok();
        GeometryService { shared, tx }
    }

    /// Queues a job unless the part is already known; returns its key.
    pub fn request(&self, job: GeometryJob) -> u64 {
        let key = geometry_key(&job.part_id, &job.content_id);
        {
            let mut s = lock(&self.shared.states);
            if s.contains_key(&key) {
                return key;
            }
            s.insert(key, GeometryState::Pending);
        }
        if self.tx.send((key, job)).is_err() {
            lock(&self.shared.states).insert(
                key,
                GeometryState::Failed("the geometry worker is not running".into()),
            );
        }
        key
    }

    /// Cheap, never blocks.
    pub fn geometry(&self, key: u64) -> GeometryState {
        lock(&self.shared.states)
            .get(&key)
            .cloned()
            .unwrap_or(GeometryState::None)
    }

    /// Forgets a result so the next `request` tries again (a user's Retry).
    pub fn forget(&self, key: u64) {
        let mut s = lock(&self.shared.states);
        if !matches!(s.get(&key), Some(GeometryState::Pending)) {
            s.remove(&key);
        }
    }
}

impl Drop for GeometryService {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn run(s: &Shared, rx: &Receiver<(u64, GeometryJob)>) {
    while let Ok((key, job)) = rx.recv() {
        if s.stop.load(Ordering::Relaxed) {
            return;
        }
        let state = load(s, &job);
        lock(&s.states).insert(key, state);
        (s.wake)();
    }
}

fn load(s: &Shared, job: &GeometryJob) -> GeometryState {
    let deadline = Some(Instant::now() + s.time_limit);
    let read = || std::fs::read(&job.step_path);
    let g = match &s.cache {
        Some(c) => cached_part_geometry(
            c,
            &job.content_id,
            job.size_mm,
            s.quality,
            read,
            &s.stop,
            deadline,
        ),
        None => match read() {
            Ok(bytes) => geometry_from_step(&bytes, job.size_mm, s.quality, &s.stop, deadline)
                .unwrap_or_else(|e| fallback_box(job.size_mm, super::fallback_for(e))),
            Err(_) => fallback_box(job.size_mm, Fallback::Unreadable),
        },
    };
    match g.origin {
        MeshOrigin::Fallback(r) => GeometryState::Failed(reason_text(r, s.time_limit)),
        _ => GeometryState::Ready(Arc::new(g)),
    }
}

fn reason_text(r: Fallback, limit: Duration) -> String {
    match r {
        Fallback::NoModel => "the part has no 3D model".into(),
        Fallback::Unreadable => "the model file could not be read as STEP".into(),
        Fallback::NoGeometry => "the STEP file has no solid or surface geometry".into(),
        Fallback::TooComplex => "the STEP file is too large or detailed for a preview".into(),
        Fallback::TimedOut => format!(
            "reading the STEP file took longer than {} s",
            limit.as_secs()
        ),
        Fallback::Cancelled => "loading was cancelled".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    fn wait(svc: &GeometryService, key: u64) -> GeometryState {
        for _ in 0..400 {
            let s = svc.geometry(key);
            if !matches!(s, GeometryState::Pending) {
                return s;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("still pending");
    }

    fn job(dir: &std::path::Path, name: &str, body: &[u8]) -> GeometryJob {
        let p = dir.join(name);
        std::fs::write(&p, body).unwrap();
        GeometryJob {
            part_id: name.into(),
            content_id: format!("id{}", name.len()),
            step_path: p,
            size_mm: None,
        }
    }

    #[test]
    fn states_go_from_none_to_pending_to_ready_and_wake_the_view() {
        let dir = tempfile::tempdir().unwrap();
        let woken = Arc::new(AtomicUsize::new(0));
        let w = woken.clone();
        let svc = GeometryService::new(
            Some(MeshCache::at(dir.path().join("cache"))),
            Quality::Normal,
            TIME_LIMIT,
            move || {
                w.fetch_add(1, Ordering::SeqCst);
            },
        );
        let step = crate::part3d::build::test_support::block_step();
        let j = job(dir.path(), "a.step", step.as_bytes());
        assert!(matches!(
            svc.geometry(geometry_key(&j.part_id, &j.content_id)),
            GeometryState::None
        ));
        let key = svc.request(j.clone());
        assert!(matches!(
            svc.geometry(key),
            GeometryState::Pending | GeometryState::Ready(_)
        ));
        let GeometryState::Ready(g) = wait(&svc, key) else {
            panic!("not ready")
        };
        assert!(!g.is_box() && g.lods.last().unwrap().triangle_count() == 12);
        assert_eq!(woken.load(Ordering::SeqCst), 1);
        // Asking again does nothing new.
        assert_eq!(svc.request(j), key);
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(woken.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn an_unusable_model_fails_once_and_stays_failed_until_forgotten() {
        let dir = tempfile::tempdir().unwrap();
        let svc = GeometryService::new(None, Quality::Normal, TIME_LIMIT, || {});
        let key = svc.request(job(dir.path(), "junk.step", b"not step"));
        let GeometryState::Failed(why) = wait(&svc, key) else {
            panic!("should fail")
        };
        assert!(why.contains("STEP"), "{why}");
        // Not retried by asking again...
        let again = svc.request(job(dir.path(), "junk.step", b"not step"));
        assert_eq!(again, key);
        assert!(matches!(svc.geometry(key), GeometryState::Failed(_)));
        // ...until the user says retry.
        svc.forget(key);
        assert!(matches!(svc.geometry(key), GeometryState::None));
    }

    #[test]
    fn a_job_over_the_time_limit_fails_with_the_limit_in_the_message() {
        let dir = tempfile::tempdir().unwrap();
        let svc = GeometryService::new(None, Quality::Normal, Duration::ZERO, || {});
        let step = crate::part3d::build::test_support::block_step();
        let key = svc.request(job(dir.path(), "slow.step", step.as_bytes()));
        let GeometryState::Failed(why) = wait(&svc, key) else {
            panic!("should time out")
        };
        assert!(why.contains("longer than 0 s"), "{why}");
    }
}
