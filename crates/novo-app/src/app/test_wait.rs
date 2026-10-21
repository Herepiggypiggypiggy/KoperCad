//! Waiting for work on another thread in tests: a deadline, not a count of
//! frames, so a loaded machine gets time and a quick one does not wait.

use std::time::{Duration, Instant};

/// How long a test waits for a background result before giving up. Far above
/// what any of them needs on an idle machine; the loop leaves as soon as the
/// result is in.
pub(super) const PATIENCE: Duration = Duration::from_secs(30);

/// Counts up from 0 until [`PATIENCE`] has passed: `for _ in patient()`
/// replaces `for _ in 0..400` around a poll and a short sleep.
pub(super) fn patient() -> impl Iterator<Item = usize> {
    within(PATIENCE)
}

fn within(limit: Duration) -> impl Iterator<Item = usize> {
    let start = Instant::now();
    (0..).take_while(move |_| start.elapsed() < limit)
}

#[test]
fn the_wait_runs_until_its_deadline_and_then_stops() {
    let started = Instant::now();
    let n = within(Duration::from_millis(40)).count();
    assert!(n > 0);
    assert!(started.elapsed() >= Duration::from_millis(40));
    assert!(started.elapsed() < Duration::from_secs(5));
}
