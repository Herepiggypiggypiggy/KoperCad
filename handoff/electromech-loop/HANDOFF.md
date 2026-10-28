# L5a handoff (2026-10-28)
Branch claude/project-thread-agxv7b. PR 1 (loop core): `novo_machine::port::{CircuitPort, FakeCircuit}`, `novo_machine::hoist_loop` (HoistLoop, HoistModule, LimitSwitch), and `TwoSpeedHoistUnit::step_energised` in novo-drive. 7 loop tests + design in docs/design/electromechanical-loop.md.
Next (PR 2, after PR 1 merges, from main): `SimCircuit` in novo-app implementing CircuitPort on novo-sim (settle_timed, device_states MotorRun speed 0 = fine, 1 = coarse; limit switch via manual PoleState), property sheet "Mechanical load", Simulate > Drive run..., live badges, QA scenarios 1366x768 and 1920x1080. Then PR 3: drive module (Drive::step with motor current feedback) once L2b gives motor at frequency.
L1 thread (cse_015gECXUkTwb2c16x4majyua) hands UI entries for Drive over to PR 2.
Customer facts (plans/crane-setup-facts.md) stay out of public files.
