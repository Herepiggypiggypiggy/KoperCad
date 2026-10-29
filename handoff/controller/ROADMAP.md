# Controller (virtual operator panel for the live view): roadmap

Crate `novo-controller` (pure engine). Lanes: C1 engine (profile format and validator, controls, signals, interlocks, link-loss and signal effects without any radio protocol, sequences, instances, bindings), C2 schematic device and live wiring, C3 panel UI, C4 import wizard, C5 gamepad adapter, C6 commissioning and checks, C7 validation.

- Done: C0 crate registration with skeleton and a two-step truth-table test.
- Done: C1 engine (#1558) and review follow-ups (PR open), see `docs/design/controller-profile.md`.
- Next: C4 and C7 in parallel, then C2 (needs C0b wasm host to run in-app; tests run natively until then), then C3, C5, C6.
- Open: host JSON API for the wasm module (C0b); profile structure agreed with C4 (import produces `Profile` values).
- C4 done: import wizard (paste, from device, table/EPLAN file, generic library) emitting Profile values. Next for C4: tuning the word lists on the user's own order pages (needs his files).
- C2 done in PR: the controller device in the live circuit (`docs/design/controller-live.md`): Controller card (Shift+L, Controller…, Simulate > Controller panel), receiver symbols with a shared common, bindings to pins, hand devices and PLC input bits, E-stop chain via `invert`, controllers stored in the project property `controllers.project`, fixture `qa/fixtures/radio-hoist.novo`, QA 1581, 1581b, 1582. Next: C3 panel polish (a real pendant picture, gamepad C5), checks in the Check menu, machine module paths (L5b), add-on packaging (C0b).

