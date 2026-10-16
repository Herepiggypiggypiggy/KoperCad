# CFD-F results and app: roadmap

1. (PR 1, this) CfdSnapshot helpers in novo-heatview (slice, as_2d, streamlines, iso_surface); Simulate in the Cabinet Heat view on a worker thread; fidelity badge, cancel, out-of-date mark, per-device margin on badges.
2. Progress bar with residual sparkline, ETA; cross-fade between fidelity levels once the scheduler (CFD-E) publishes several.
3. A/B compare: Δ mode done (PR 3); Wipe and Side modes, time slider for transient.
4. 3D face in novo-view3d (heat module): slice planes, iso-surface shell, streamline tubes, tracers.
5. Replace StubSolver in `heat_run::default_solver` with the scheduler.
