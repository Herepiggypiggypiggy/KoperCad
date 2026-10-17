# CFD-A flow core: handoff

State: skeleton PR from branch claude/cfd-a (rebased on main after wave 0 #560).
Other lanes build against `novo_cfd::{Solver, Control, CfdSnapshot, Fidelity, Grid3}`; `StubSolver` (Coarse only) extrudes the 2D novo-thermal result so they can work before the real solver exists.
Contract types live in novo-cfd (not novo-thermal::cfd as the design sketched); they reuse novo_thermal::{DeviceResult, HotSpot}.
Where the user finds it: Cabinet view, Heat layer, Simulate (cabinet.thermal.simulate); app side is lane CFD-F.
Transient contract: `novo_cfd::{TransientSolver, TransientSpec, TransientFrame, TransientSummary}` (src/transient.rs). Implementations: `StubSolver` (2D engine, extruded) and, next, the 3D march in `cabinet/` on `FlowSolver::heat_scale`. Where the user finds it: Cabinet view, Heat layer, time slider and "Heat-up over time" (lane CFD-F).

Fine level: `LevelTuning::radiation` couples surface radiation (`cabinet/radiation.rs`, `FlowSolver::extra_heat_w_m3`). CFD-FAST owns `steady/assemble.rs`; the only change there is `+ self.extra_heat_k_s(id)` in the energy rhs. Where the user finds it: the Fine level of the Simulate dialog (lane CFD-F) gets radiation without a switch; `CabinetSolver::radiation` holds the settings. The 3D transient couples it too. Not yet: a per-device convection/conduction/radiation split from the coupled run (`rad::HeatPaths` exists).
