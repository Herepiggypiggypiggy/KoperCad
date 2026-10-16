# CFD-A flow core: handoff

State: skeleton PR from branch claude/cfd-a (rebased on main after wave 0 #560).
Other lanes build against `novo_cfd::{Solver, Control, CfdSnapshot, Fidelity, Grid3}`; `StubSolver` (Coarse only) extrudes the 2D novo-thermal result so they can work before the real solver exists.
Contract types live in novo-cfd (not novo-thermal::cfd as the design sketched); they reuse novo_thermal::{DeviceResult, HotSpot}.
Where the user finds it: Cabinet view, Heat layer, Simulate (cabinet.thermal.simulate); app side is lane CFD-F.
