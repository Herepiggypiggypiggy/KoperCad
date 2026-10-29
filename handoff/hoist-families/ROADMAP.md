# Hoist families roadmap

Scope: simulate contactor hoist families in the live circuit (conical-rotor and pole-change, fine-lift, rectifier brakes) and their failure cases. Generic public behaviour only, no manufacturer data.

Done (PR from branch claude/project-thread-8de6e0):
- supply.rs: voltage and lost phase, two-phase torque (double revolving field, zero at standstill).
- brake_system.rs: conical field-released brake, rectifier kinds (half-wave, bridge, fast excitation, controlled), rectifier faults, lost-phase effect on rectifier taps.
- brake.rs: wear (delay and torque scale), pull-in/drop-out hysteresis.
- hoist_unit.rs: Feeds, HoistFaults, Conditions, FineLift.
- spec.rs: mech_brake_kind, mech_rectifier*, mech_fl_* keys.
- Simulate > Hoist run: Faults to try, conditions.

Next: closed-loop hoist, other gear arrangements if real data is supplied, motor current and heating on stall or single-phasing, 3D hook binding.
