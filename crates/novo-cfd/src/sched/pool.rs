//! Thread pool for the kernels.

/// How many threads the solver kernels may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Threads {
    /// All cores but one, so the UI stays smooth.
    #[default]
    Auto,
    /// Exactly this many (at least one).
    Fixed(usize),
}

/// Threads `Threads::Auto` resolves to on this machine.
pub fn default_threads() -> usize {
    let n = std::thread::available_parallelism().map_or(1, |n| n.get());
    n.saturating_sub(1).max(1)
}

impl Threads {
    pub fn count(self) -> usize {
        match self {
            Threads::Auto => default_threads(),
            Threads::Fixed(n) => n.max(1),
        }
    }
}

/// Run `f` inside a dedicated pool, so rayon loops in the kernels use
/// exactly `threads` workers and not the global pool.
pub fn install<R: Send>(threads: Threads, f: impl FnOnce() -> R + Send) -> R {
    match rayon::ThreadPoolBuilder::new()
        .num_threads(threads.count())
        .thread_name(|i| format!("cfd-worker-{i}"))
        .build()
    {
        Ok(pool) => pool.install(f),
        // No pool (resource limits): run on the current thread.
        Err(_) => f(),
    }
}
