# Closed-loop (encoder) hoists and synchronous winches (lane L2c)

Date 2026-10-29. Status: engine, scenarios and window built. Plan lane L2c in
`docs/design/drive-crane-simulation.md`.

## Why

Some hoists are not two-speed contactor hoists: the drive controls torque with an encoder on the
motor shaft, even at zero speed. The drive opens a holding brake through its own relay, only after
it has proven it can carry the load, and two winches can run on one load, alone or synchronous.
The failures that matter here are not in the steady state: a brake that opens before the torque
is there, torque removed before the brake holds, an encoder that dies, one drive of a pair that
trips, a drum that is one percent off.

## What is built

`novo-drive::closed_loop` (pure engine)

| Part | What it does | Exact or model |
| --- | --- | --- |
| `encoder` | Quadrature counts (4 ppr per revolution), speed over a window, position, failure injection (signal lost, direction channel lost) | counts and resolution exact; failures are scenarios |
| `sto` | Two channels; one low channel removes torque at once; a mismatch longer than the limit latches until both channels have been low | gating exact; the monitor is a model, no SIL or PL claim |
| `controller` | Speed PI around a first-order torque loop; torque limit from the current limit with constant power above rated speed; sequence Off, Magnetising, TorqueProof, Releasing, Running, Stopping, BrakeClosing; pre-torque (none, fixed, load cell, memory); jerk-limited ramp; monitoring (overspeed, speed deviation, rollback, encoder plausibility, brake feedback, motor I2t, DC bus); torque follower mode | ramp arithmetic exact; the rest model |
| `unit` | Drive + holding brake + hoist gravity on one shaft | gravity and reduction exact; brake delays are entered values |
| `findings` | Checks on the entered numbers (no run needed) | each states what it computed |

`novo-machine::sync_winch` (two winches on one beam)

Rigid beam of mass M and length L on two vertical ropes, centre of gravity e from the middle. Each
rope is a spring-damper that pulls only when taut. The beam is free to tilt or guided (tilt
locked). Exact statics of the stated model: with a free beam the rope forces are
`F_A = M g (1/2 + e/L)`, `F_B = M g (1/2 - e/L)` whatever the drums did and the tilt is
`(x_A - x_B) / L`; with a guided beam and equal rope stiffness the forces differ by `k (x_A - x_B)`.
Sync modes: same reference (no correction), position lock between encoders, torque follower.
Supervision: position error stop, partner stop on a drive fault (a link) or none.

`novo-machine::winch_scenarios`: six scripted scenarios with traces, events and findings, used by
the window.

## Where the user finds it

Simulate > Winch scenarios (`sim.winches`, Shift+Space search "winch"). The window runs a scenario
with a generic 30 kW hoist, draws hook, speed, torque and rope-force traces, lists the events and
the design findings, and shows which numbers are exact and which are a model.

## Hand-calculated cases (tests `crates/novo-drive/tests/closed_loop.rs`, `crates/novo-machine/tests/sync_winch.rs`)

1. 1024 ppr gives 4096 counts per revolution; one count per 5 ms window is 2.93 rpm.
2. Weight torque 2500 kg x 9.80665 x 0.2 m / 31.5 = 155.67 N m; the closed brake holds it.
3. Pre-torque equal to the weight: the load does not sag when the brake opens.
4. No pre-torque and no integral: the load creeps at `(T_s eta - T_min) / kp` (hand formula).
5. Torque removed 0.15 s before the brake applies: the drop is `a gap^2 / 2 + v^2 / (2 a_dec)` with
   `a = T_s eta / J`, `a_dec = (T_brake - T_s eta) / J`.
6. Static rope forces of a beam with the centre of gravity 0.4 m off: 29.42 kN and 19.61 kN.
7. Free beam, drum B 1 % larger: tilt equals the position difference over the length.
8. Guided beam, drum B 1 % larger: `F_B - F_A = k (x_B - x_A)`.
9. Torque follower: `F_B / F_A = mpr_A / mpr_B`, so no overload from the drum error.
10. Drive A trips, no link, guided beam: B carries `M g` = 49.0 kN against a 30 kN rope rating and
    needs 346 N m against a 320 N m limit, so it slows.

## Honest limits

See `novo_drive::closed_loop::LEAVES_OUT`. In short: averaged drive, perfect field orientation (a
lost encoder is detected, its effect on the field angle is not simulated), the encoder plausibility
check uses the true shaft speed above a threshold, generic parameters, fixed brake delays, linear
rope, vertical ropes (the snapshot flags a tilt beyond 5 degrees as outside the model). Not a
safety or compliance statement.

## Not yet

- A closed-loop hoist from the schematic: the property sheet section and the binding of the drive's
  brake relay and STO inputs to symbols, so the circuit drives these engines like the hoist run does.
  The engines take plain inputs (`ClInputs`, `PairCommand`), so the loop in `novo-machine` can feed them.
- Unconfirmed customer details: which drive maker's hoist and encoder arrangement applies. Everything
  here is generic public knowledge.
