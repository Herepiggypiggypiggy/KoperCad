# WS-25 symbols (packs and library growth): roadmap

## Scope

Standards packs and symbol library growth in `novo-symbols` (WS-05 crate, WS-25 work).
Design: `docs/design/symbol-packs.md`. Format: `crates/novo-symbols/FORMAT.md`.

## Done

| PR | What it delivered |
| --- | --- |
| #335 | Design doc for packs and the NFPA pack (merged) |
| #351 (merged) | `Pack`, `builtin_pack`, `load_dir_pack`, 135 NFPA symbols, `tests/packs.rs` |
| growth PR | 18 new IEC symbols plus NFPA counterparts (148 NFPA in total): mushroom, foot and pull-cord switches, flow switch, brake and solenoid coils, pressure, speed and flow meters, coloured lamps; letter codes for them |

## Next, in order

0. **Audit of literal `iec/` keys** (reviewer ask on #335, listed in the design doc Risks): 20 non-test files match `iec/...` keys and would ignore `nfpa/...` symbols. Switch each to `function` or a pack-independent key and add one test per engine on `nfpa` keys. Owners are the engine threads; the pack is not usable from the app before this.
1. **NFPA breadth**, devices that exist in IEC but have no NFPA drawing yet (`gen_nfpa.py` has no entry): resistors and heaters (zigzag), inductors, potentiometers and variable passives, varistor, bell/buzzer/horn, 3-phase transformer, current transformer, emc filter, rcd/rcbo (GFCI), mcb with N, dahlander motor, changeover switch, surge arrester, sensors pt100 look.
2. **Project standard setting** that picks the pack and `Catalog::new(Library::builtin_pack(pack))`. This is WS-01/WS-02 work; ask them.
3. **Switch standard command** built on `Pack::counterpart`: rewrite symbol keys, report devices with no counterpart.
4. **More IEC growth**: 3-wire inductive/capacitive/photoelectric sensors, illuminated push buttons, solid-state relay, beacon, encoder, fans.
5. **Hands-on check by Tadjiro** of the NFPA artwork: delay-contact arcs, pressure and temperature letters, breaker arc, fuse S-curve, pilot-light letters.

## Open questions

- Should NFPA-style designations (`L1/T1`, motor `T1 T2 T3`) be an instance option? Today both packs use EN 50005 ids so switching is lossless.
