# Device models and animation (WS-20 / WS-21): roadmap

Scope: what lamps, motors and contactors do in the live view (Shift+L). Design: `docs/design/device-animation.md`.
Split with Potentials (cse_01LfvbEvxsdpj175azTV38EX): they own `coupling.rs`, latching, outputs into potentials, wire tint; this thread owns `novo-sim/src/devices.rs` and `novo-app/src/app/live_anim.rs`.

Done
- Design doc.
- `novo_sim::device_states`: lamp lit, motor Stopped/Running(direction, speed)/PhaseLoss/Fault.
- Animation: lamp glow, motor spokes with spin-up and spin-down, coil pull-in ring.

Next
1. Hands-on test by Tadjiro; tune sizes and speeds.
2. Contact blade drawn moving between open and closed (needs symbol artwork per contact).
3. Per-timer progress ring once `Settled` has per-device remaining time (ask Potentials).
4. Reduced-motion setting (needs a Preferences entry from the App shell thread).
5. Overload relay trip and fault scenarios (WS-22).
