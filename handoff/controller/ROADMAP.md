# Controller (virtual operator panel for the live view): roadmap

Crate `novo-controller` (pure engine). Lanes: C1 engine (profile format and validator, controls, signals, interlocks, link-loss and signal effects without any radio protocol, sequences, instances, bindings), C2 schematic device and live wiring, C3 panel UI, C4 import wizard, C5 gamepad adapter, C6 commissioning and checks, C7 validation.

- Done: C0 crate registration with skeleton and a two-step truth-table test.
- Done: C1 engine (#1558) and review follow-ups (PR open), see `docs/design/controller-profile.md`.
- Next: C4 and C7 in parallel, then C2 (needs C0b wasm host to run in-app; tests run natively until then), then C3, C5, C6.
- Open: host JSON API for the wasm module (C0b); profile structure agreed with C4 (import produces `Profile` values).
