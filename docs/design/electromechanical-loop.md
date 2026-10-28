# Electromechanical loop (lane L5a): design

Status: part 1 (loop core, PR #1561) and part 2 (hoist run in the live view) are built. The
drive module in the loop (travel drive with motor current feedback) is part 3 and waits for the
motor-at-frequency lane (L2b).

## Goal

Connect the schematic's live view to the physical engines: a pressed button settles the
circuit, coils and drive terminals respond, motors and brakes move mechanics, and the
resulting positions feed limit switches back into the circuit. One deterministic fixed-step loop.

## The dependency cycle, and how it is cut

`novo-sim` (logic settle) must not know about `novo-drive` or `novo-machine`, and the engines must
not depend on `novo-sim` or `novo-core`. The loop needs both sides, so it talks to the circuit through a
small trait owned by the loop crate:

```
novo-machine::port::CircuitPort
    fn motor_feed(&self, motor: &str) -> MotorFeed       // read: which windings and brake feed are live
    fn is_energised(&self, device: &str) -> bool          // read: a coil or contact device is on
    fn knows_device(&self, device: &str) -> bool          // read: the tag exists in the project
    fn set_contact(&mut self, device: &str, closed: bool) // write: limit switch contact at its height
    fn settle(&mut self, dt_s: f64)                       // advance the logic by one tick
```

- `novo-machine` depends only on `novo-drive` (as today). It contains the loop (`Loop::step`),
  the per-instance binding table and a `FakeCircuit` used by its own tests.
- The real `CircuitPort` is `LiveCircuit` in `novo-app` (`app/hoist_run.rs`) on top of
  `novo_sim::{settle_timed, PoleState, Clock}`: reading coil and pole state is already public;
  writing a sensor means forcing a pole (the limit switch's contact group) in the `PoleState`
  before the next settle. No new public API in `novo-sim` is expected. If a hook is missing, it
  is added in a separate small PR from the sim owner.
- Result: no cycle, `novo-machine` stays pure and testable without a project.

## Loop order (per tick, dt fixed, default 5 ms, deterministic)

1. Sensors from the previous mechanical positions (limit, photocell) -> `set_sensor`.
2. `circuit.settle(dt)`.
3. Read bindings: contactor coils -> `TwoSpeedHoistUnit` step/direction inputs; mains and
   brake feed levels; drive digital inputs in macro order, reference, mains_ok.
4. Step engines: `TwoSpeedHoistUnit::step`, `Drive::step` with the motor's measured current
   and power from the previous tick.
5. Mechanics advance (hook height, travel positions) and outputs (rpm, fine/coarse, brake state, drive
   frequency and fault) are written to a `Snapshot` for the UI and the scope.

Every module instance has its own bindings, so identical hoists on one crab bind to different
contactor groups. Cost is linear in module count.

## Bindings and mechanical data

The motor's data lives in its device properties (`novo_machine::spec`, keys `mech_*`; setting
`mech_mass_kg` turns a motor into a hoist). The limit switch is bound by the tag of its drawn device
(`mech_upper_limit`, `mech_lower_limit`) and the height it acts at. `HoistLoop::check_bindings`
reports a tag the project does not have and a separate brake feed that is not bound, before the run
starts. A dedicated binding table in the project model needs a WS-02 change and is not part of this
work.

## UI entries

| Place | Content |
| --- | --- |
| Property sheet of a motor, "Mechanical load" | hook load, drum, gearbox, windings, brake delays, limit tags and heights |
| Simulate > Hoist run | window with Start/Stop, readings per hoist, notes from the binding check, the model-versus-exact list |
| Live view (Shift+L) badges | next to the motor: rpm, winding, brake state, hook height |

Not yet built: Property sheet of a drive ("Drive") and Simulate > Drive run (part 3).

## Tests (written first)

End-to-end with `FakeCircuit`: button -> brake release -> hook rise; coarse-only end switch stops
coarse but fine still runs; removing the NC contact makes the stop fail (plan case 12); swapped
bindings give swapped motion (case 18); 3 cranes / 6 crabs / 10 hoists within the frame budget and
identical twice (case 21). Then QA scenarios with screenshots at 1366x768 and 1920x1080.

## Honesty

Averaged models. Sensor positions are the user's entered values. Nothing here is a safety or
compliance verdict.
