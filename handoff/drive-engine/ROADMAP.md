# Drive engine (lane L1): roadmap

Scope: ATV320-class travel drive behaviour with generic public parameters in `novo-drive` (`drive/`).

Done (PR 1):
- `Ramp` safe against NaN, zero or negative times and steps; reversal through zero decelerates then accelerates.
- `SRamp` jerk-limited ramp. `VfCurve` (constant torque, quadratic, boost, flux ratio).
- `DcBus` (rectifier, chopper, resistor, optional regen, energy booking), braking resistor check, sizing rule (2 x sum of motor currents).
- `I2tOverload`, `Fault` list with neutral codes.
- Terminals and macros (2-wire presets, two speeds, run and direction, motor pot), reference sources (panel, analog V, analog mA with loss, word, motor pot).
- `Drive` state machine: min/max, skip bands, jog, presets, fast stop, ramp stretch, current limit (V/f holds the ramp, vector gives a torque limit), faults with latch and reset, logic outputs.

Next:
- Un-ignore the L1 cases in `tests/hand_validation.rs` once PR 1555 is on main (patched copy in the PR description).
- UI entry with the L5a loop lane (property sheet "Drive" section, Simulate > Drive run, live badge).
- Generic profile format and alias mapping (L3 thread reads `drive::DriveConfig`).
- Motor torque and current from the motor model (L2b) replace the hand-fed `motor_current_a` and `motor_power_w` inputs.
- Open: drive thermal derating, 3-wire control, relay output assignment as data.
