# Electromechanical loop (lane L5a): design

Status: design only, written 2026-10-28 while #1556 (L2a hoist engine) and #1557 (L1 drive
engine) are open. Code starts when both are merged (no stacked PRs).

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
    fn coil_energised(&self, binding: &Binding) -> bool          // read
    fn terminal_level(&self, binding: &Binding) -> Level         // read (drive DI, brake rectifier feed)
    fn set_sensor(&mut self, binding: &Binding, closed: bool)    // write (limit switch, photocell)
    fn settle(&mut self, dt: f64)                                 // advance logic by one tick
```

- `novo-machine` depends only on `novo-drive` (as today). It contains the loop (`Loop::step`),
  the per-instance binding table and a `FakeCircuit` used by its own tests.
- The real `CircuitPort` is implemented in `novo-app` (`app/machine_port.rs`) on top of
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

## Bindings

`Binding { device: DeviceTag, pin_or_group: .., kind }` stored in the project through the WS-02
model change (to be requested from WS-02, described in the PR). Until then the table lives in
`novo-machine` and in a sidecar settings blob.

## UI entries (an engine without UI is not done)

| Place | Content |
| --- | --- |
| Property sheet of a motor, "Mechanical load" | gear ratio, drum radius, falls, payload, brake delays; model vs exact labels |
| Property sheet of a drive, "Drive" | macro, presets, ramps, limits; `drive::FIDELITY` and `LEAVES_OUT` panel |
| Simulate > Drive run... | start the loop, scope of frequency, rpm, brake, hook height |
| Live view (Shift+L) badges | rpm, fine/coarse, brake open/closed, drive frequency and fault code |

## Tests (written first)

End-to-end with `FakeCircuit`: button -> brake release -> hook rise; coarse-only end switch stops
coarse but fine still runs; removing the NC contact makes the stop fail (plan case 12); swapped
bindings give swapped motion (case 18); 3 cranes / 6 crabs / 10 hoists within the frame budget and
identical twice (case 21). Then QA scenarios with screenshots at 1366x768 and 1920x1080.

## Honesty

Averaged models. Sensor positions are the user's entered values. Nothing here is a safety or
compliance verdict.
