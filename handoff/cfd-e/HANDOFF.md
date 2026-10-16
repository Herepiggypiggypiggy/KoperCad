# CFD-E performance: handoff

State: `sched` module on branch claude/cfd-e-performance-0u99co. `Scheduler::start(scenario, Plan)` returns a `Job`; the UI calls `Job::poll()` per frame and `Job::cancel()`. Levels run in order, unsupported ones are skipped, panics become `Outcome::Failed`, intermediate snapshots are throttled (`min_publish_interval`), the final one per level never is. A new start cancels the old job. Kernels run in a dedicated rayon pool (`Threads::Auto` = cores minus one).
Where the user finds it: Cabinet view, Heat layer, Simulate (`cabinet.thermal.simulate`); wired by CFD-F.
Next: kernel threading after #575, warm start between levels, real benchmarks (`examples/sched_bench.rs`).
