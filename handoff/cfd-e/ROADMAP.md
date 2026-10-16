# CFD-E performance: roadmap

1. Scheduler, cancel plumbing, event polling (PR 1, done in `novo-cfd/src/sched/`).
2. rayon kernels once the flow core (#575) is on main: parallel loops inside the pool installed by `sched::install`; single-thread equality tests.
3. Grid sequencing: coarse solution interpolated as the initial state of Medium (needs a warm-start hook on `Solver`, to agree with CFD-A).
4. Benchmarks example on the real solver; optional wgpu smoother behind a feature with CPU fallback.
5. (done) Threaded independent flow loops, bit-identical. Next: red-black sweeps, once CFD-A's hooks PR is on main.
