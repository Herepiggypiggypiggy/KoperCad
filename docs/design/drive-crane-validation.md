# Drive and crane: independent validation (lane L7)

Plan: `docs/design/drive-crane-simulation.md`, validation cases 1-21. Tests:
`crates/novo-drive/tests/hand_validation.rs` and `crates/novo-machine/tests/hand_validation.rs`.

## How this suite works

- The expected values are worked out by hand from the physics and are written in the test
  comments next to the arithmetic. They are never taken from the engine under test.
- A test without an ignore marker runs against the engine as merged. `oracle_*` tests check the
  hand arithmetic itself (a typo in a hand value cannot hide an engine error).
- A test marked `#[ignore = "Lx: ..."]` waits for lane Lx. Its body is a `todo!` that says which
  engine call to compare with which hand value. The lane that lands the engine part replaces
  the `todo!`, removes the ignore and keeps the hand value.
- All values are generic. No manufacturer data and no customer data are used.
- Conventions chosen here that a lane must follow or argue against in its PR: slip speed is
  constant in the constant-flux region (case 1, a model convention that neglects the stator drop at 5-10 Hz); the brake holding check does not credit
  gear efficiency (case 5); the brake-drop case includes the motor rotor inertia reflected
  through the gearbox (case 6); I2t is the integral of `(I/In)^2 - 1` with the entered
  150 % / 60 s point as the one exact value (case 10).

## Status

| Case | What | Runs now | Waits for |
| --- | --- | --- | --- |
| 1 | Synchronous speed table, pole-change pair | yes | slip speed: L2b |
| 2 | Linear ramp points, reversal through zero, S-ramp end points and jerk | yes (L1 landed) | |
| 3 | V/f law and flux ratio | yes (L1 landed) | |
| 4 | Breakdown torque against closed-form Thevenin, `(V/f)^2` scaling | hand values only | L2b |
| 5 | Hoisting torque 345.91 N m | yes (L2a landed) | |
| 6 | Brake opens before torque: 110.32 mm bound, 31.70 mm with inertia | yes (L2a landed) | |
| 7 | Braking energy 24.67 kJ, peak 12.34 kW, resistor limits | yes (L1 landed) | |
| 8 | DC bus energy balance (200 random runs) | yes (L1 landed) | |
| 9 | Current limit acceleration time 2.618 s | hand values only | L2b (torque from the motor model) |
| 10 | I2t trip times 60 s, 170.5 s, 25 s | yes (L1 landed) | |
| 11 | Pendulum period, 30 degree correction 4.0827 s | small angle yes | nonlinear option: L4b |
| 12 | Raise button stops at the entered limit within one sample | tolerance only | L5a |
| 13 | Two-speed run-up 0.2384 s + dead time + 0.5676 s | mechanics yes (L2a landed) | sequencer with motor curves: L5a |
| 14 | Brake apply delay, AC side -14.03 mm against DC side +0.42 mm | yes (L2a landed) | |
| 15 | End switch stops coarse only, ideal time 27.2 s | hand values only | L2a, L5a |
| 16 | Two cranes: coarse stop 27.0 m, fine to 29.5 m, 59 s | hand values only | L4a, L5b |
| 17 | Two crabs: reach 7.5 m, refusal at 1.5 m separation | hand values only | L4a |
| 18 | Two hoists on one crab, bindings swapped | hand values only | L5a |
| 19 | Tandem 6666.67 / 3333.33 kg, tilt 1.7191 degrees | hand values only | L4a |
| 20 | Transform composition (12.9, 2.9, 5.9) m, property test | hand values only | L4a |
| 21 | 3 cranes, 6 crabs, 10 hoists, deterministic twice, step budget | no | L5a |

## Findings so far

- **F-1 (L1), reversal through zero. Fixed by L1 (#1557).** `Ramp::step` used the accel rate for
  the whole move from +20 to -20 Hz (-12 Hz after 3.2 s). Hand value: decel 20 to 0 at 50/3 Hz/s
  = 1.2 s, then accel 0 to -20 at 10 Hz/s = 2.0 s. The test now passes against the fixed engine.
- **F-2 (plan wording, case 6).** The plan's drop `0.5 g t^2` is only an upper bound. With the
  motor inertia reflected through a 31.5:1 gearbox on a 0.2 m drum the same 150 ms gives 31.7 mm
  instead of 110.3 mm. The engine must include the inertia; the bound is kept as a sanity limit.
- **F-3 (edge, case 2). Fixed by L1 (#1557).** `Ramp::step` with an accel time of 0 and a step of 0 s gave `inf * 0 = NaN` and `clamp` panicked.

## Hand calculations, short form

Details sit in the test comments; the main derivations:

- Case 4: `T_max = 3 V_th^2 / (2 w_s (R_th + sqrt(R_th^2 + (X_th + X2)^2)))`, with
  `Z_th = jXm (R1 + jX1) / (R1 + j(X1 + Xm))` and `V_th = V jXm / (R1 + j(X1 + Xm))`. With R1 = 0
  every term scales with f at constant V/f, so the closed form (the unrestricted maximum) is 241.027 N m. It is reachable only where the peak slip is <= 1: 25 and 50 Hz yes (0.36, 0.18); at 5 Hz the peak slip is 1.78, so the motoring maximum is the standstill torque 205.980 N m.
- Case 6: reflected inertia as mass `J (i/r)^2 = 0.5 * 157.5^2 = 12403 kg`, so
  `a = g m / (m + J_ref) = 2.8175 m/s^2` and the drop is `0.5 a t^2`.
- Case 13: stage time `t = J (w2 - w1) / (T_motor - T_load)` with `J = 0.70156 kg m^2`
  (rotor 0.5 plus payload `m (r/i)^2 = 0.2016`).
- Case 14: free acceleration `a` as in case 6 for the apply delay, then the brake stop
  `v^2 / (2 a_b)` with `a_b = (T_brake -/+ T_load) / J * r / i`.
- Case 19: moment balance about the hoist points; tilt `asin(dh / L)`.
