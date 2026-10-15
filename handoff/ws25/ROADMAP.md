# WS-25 symbols (packs and library growth): roadmap

## Scope

Standards packs and symbol library growth in `novo-symbols` (WS-05 crate, WS-25 work).
Design: `docs/design/symbol-packs.md`. Format: `crates/novo-symbols/FORMAT.md`.

## Done

| PR | What it delivered |
| --- | --- |
| #335 | Design doc for packs and the NFPA pack (merged) |
| #351 (merged) | `Pack`, `builtin_pack`, `load_dir_pack`, 135 NFPA symbols, `tests/packs.rs` |
| growth PR (merged) | 18 new IEC symbols plus NFPA counterparts (148 NFPA in total): mushroom, foot and pull-cord switches, flow switch, brake and solenoid coils, pressure, speed and flow meters, coloured lamps; letter codes for them |
| #380, #390, #407, #418, #424 | Audit of literal `iec/` keys: terminals, sim, plc, canvas, editor, workflow recognise NFPA symbols through `Pack::path_of`/`same_device`/`Catalog::in_pack`, each with `tests/nfpa_keys.rs` |
| audit 6/6 | xref and checks tests that NFPA contacts classify like IEC |

## Next, in order

0. ~~Audit of literal `iec/` keys~~ done (see Done).
1. ~~NFPA breadth~~ done: all 174 IEC symbols now have an NFPA twin (26 added: resistors, heaters, inductors, thermistors, varistor, potentiometer, horn/bell/buzzer, surge arrester, MCB with N, RCD/RCBO, Dahlander motor, EMC filter, 3-phase transformer; reactor, inductor and changeover switch share the IEC artwork). Render check in `docs/design/nfpa-breadth/`.
2. **Project standard setting**: `ProjectRules.standard` is on main (#369); WS-01 wires `Catalog::builtin_pack` in the app (#383).
3. **Switch standard**: `Command::SwitchStandard` is on main (#387); a caller that computes keys with `Pack::counterpart` and reports devices without counterpart is still to be written (app or workflow).
4. ~~More IEC growth~~ partly done: 3-wire sensors (inductive, capacitive, photoelectric, PNP and NPN), beacon, encoder, single-phase and DC fans, each with an NFPA twin. Left: illuminated push buttons and solid-state relay, which need a behavior kind with two circuits (contact plus lamp, input plus output) from WS-19.
5. **Hands-on check by Tadjiro** of the NFPA artwork: delay-contact arcs, pressure and temperature letters, breaker arc, fuse S-curve, pilot-light letters.

## Open questions

- Should NFPA-style designations (`L1/T1`, motor `T1 T2 T3`) be an instance option? Today both packs use EN 50005 ids so switching is lossless.
