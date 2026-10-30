# Controller gamepad (lane C5): roadmap

Scope: a real USB gamepad or joystick holds the controls of the virtual controller (`plans/controller-live-view.md`, lane C5).
Offline, no drivers, no administrator rights. Design: `docs/design/controller-live.md`, section "Gamepad and joystick".

- Done (PR from `claude/controller-c5-gamepad`): pure module `novo_controller::gamepad` (input state, map, mapper, calibrator, validation, TOML), maps saved per
  instance in the project controllers block, generic host input service (`input_devices.rs`, trait plus fake device), Windows reader (`input_gilrs.rs`,
  `gilrs`), Gamepad section in the Controller card with live monitor, test view, map editor with "listen", calibration wizard, `pad` control command and QA step,
  lesson anchor `live.controller.gamepad`.
- Next: run the Windows reader against physical pads (Xbox, DualShock, a joystick) and fix what the field shows; devices without an SDL mapping
  (numbered `axis3`/`button7` are accepted by the map but the reader does not report them yet); a Linux/macOS reader if a system library is acceptable
  (gilrs needs libudev on Linux); wasm add-on request path for input devices (plan 5b: add-ons ask the host service).
- Open questions: should the map also be storable in the profile file (`*.controller.toml`) for sharing, or is the project enough; should the pad keep working
  while the card is closed (today it lets go)?
