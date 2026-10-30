# Controller panel kinds and signal filter: roadmap

Thread scope: two connected items of lane C3 (`plans/controller-live-view.md` sections 0, 2b, 5b, 6).

## Done in the pull request

- Add-on API 2: panel widget kinds `rocker`, `joystick`, `selector` (also rotary), `keymap`, `table`, `tabs`, `bar`
  (`crates/novo-addons/src/layout.rs`), drawn by `crates/novo-app/src/app/addons_widgets.rs` with the painters shared with
  the Controller card (`controller_widgets.rs`: `paint_zone`, `paint_knob`, `paint_pad`, `key_grid`).
- Version gate: `API_VERSION = 2`, `MIN_API_VERSION = 1`; a kind needs the API it came with (`parse_for_api`).
- The Controllers add-on panel (`addons/controllers/pack/data/radio.panel.toml`) uses the new kinds; add-on version 0.2.0.
- Signal filter (`novo_controller::monitor`): controller, text, only active, only unbound; Controller card UI; remembered
  for the session. Pop-out windows per controller instance.
- Docs `docs/design/addons.md`, `docs/design/controller-live.md`; QA scenarios 5630 to 5633.

## Next, in order

1. Gamepad adapter (C5) can drive the same joystick/rocker ids.
2. Engine replies with `bars` (a drive-speed meter) once the machine loop exists (L5b).
3. A pendant panel template as data (second panel file) using `tabs` for several hoists.
4. Persist the filter per project if the owner wants it (today it is session only, by design).

## Open questions

- The pop-out windows use the shared `panel::dialog`, so Escape closes the topmost one. If that is unwelcome while the
  keyboard drives a controller, give them their own id handling.
