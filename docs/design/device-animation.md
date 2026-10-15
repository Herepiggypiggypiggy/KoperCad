# Device models and animation (WS-20 / WS-21)

Status: design for PRs 1 to 3 below. Owner: Device models thread. The
Potentials thread keeps `coupling.rs` (coils, seal-in, timers) and the wire
tint; this work only reads a `Settled` and never feeds back into it.

## Goal

In the live view (Shift+L) the drawing behaves like the plant it describes:
a lamp lights when its two pins carry different potentials, a motor turns
when its terminals are fed, a contactor's armature moves when its coil
operates. Nothing is stored in the project.

## Layers

1. **Models** (`novo_sim::devices`, pure, no UI). `device_states(project,
   netlist, &Settled) -> DeviceStates` derives, per symbol:
   - `Lamp { lit }`: the two pins carry two different live potentials.
   - `Motor { run, speed }`: see below.
   - `Relay { operated }` on each coil: energized, and for each driven
     contact the armature position (open or closed, taken from the poles).
2. **Animation state** (`novo-app/src/app/live_anim.rs`). Per symbol it keeps
   an eased value (0..1 for glow and armature travel) and a rotation angle
   for motors, advanced by the frame delta. It lives in `LiveView`, never in
   the project, and asks for repaints only while something is moving.
3. **Painting.** Drawn by egui over the canvas, anchored on the symbol's
   own geometry (lamp circle, motor circle from the library artwork), so it
   follows rotation, mirroring and zoom.

## Motor model

Pins with the `PE` name are ignored. The rest are the windings.

| Symbol | Rule |
| --- | --- |
| `motor_3ph`, `_6t` | Three pins on three *different* line potentials: `Running`; direction from the order of the potential numbers (L1 L2 L3 and its rotations = forward, otherwise reverse). Two of three live: `PhaseLoss` (hums, does not turn). One or none: `Stopped`. Two pins sharing a potential count once. |
| `motor_3ph_dahlander` | Each group of three is a speed; one live group runs at that speed, both live is `Fault`. |
| `motor_1ph` | Two pins on two different potentials: `Running`. |
| `motor_dc` | Same; reverse when `A1` is the more negative pin (DC negative on `A1`, positive on `A2`). |

A shorted net (`Live::Shorted`) on any winding pin is `Fault`.

## Looks

- Lamp: filled disc and a soft halo in the device `color` property (default
  amber), fading in and out over about 150 ms.
- Motor: spokes rotate inside the circle at a rate set by speed, clockwise
  for forward; `PhaseLoss` shakes and is amber, `Fault` is red; ramp up and
  down so it spins down after power is lost.
- Contact: the moving blade is drawn on the pole between its pins, easing
  between open and closed in about 120 ms; a coil gets a ring that fills
  when it operates.
- Reduced motion (setting): states switch without easing and the motor shows
  a static direction arrow instead of spinning.

## PR plan

1. `novo-sim/src/devices.rs` + `tests/devices.rs`: the models.
2. `novo-app/src/app/live_anim.rs`: lamp, motor and contact animation, one
   hook in `live.rs`.
3. Polish: reduced motion, per-timer progress ring if `Settled` grows a
   per-device remaining time (Potentials thread).

## Out of scope

Overload trips, thermal models, faults injected by the user (WS-22),
latching and outputs wired back into potentials (Potentials thread).
