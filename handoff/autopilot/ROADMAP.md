# Autopilot roadmap

Scope: propose everything the drawing implies for the workshop pack, with a reason per row, reviewed before it is written, one undo step. Source: creative audit round 3, items 24 and 25 (`audit/creative/proposals-round3.md`).

## Done (PR 1)
- Engine `novo_reports::autopilot`: sizes (current from the protecting device, the rated devices on the circuit, or a stated load; ampacity table, derating, voltage drop), colours (IEC 60204-1), PE and neutral follow the line wires, `Basis` per row (rule, calculated, planning value, assumed).
- `novo_checks::calc_basis`: the facts per wire (protecting device, circuit kind, rated neighbours through unrated fuses, contacts and terminals).
- Wire-number rows from `novo-numbering` (project scheme, Fill mode).
- Window Autopilot (command `wires.autopilot`, Edit menu, command field) with per-row tick, Accept all, Accept ticked.
- Ready to build meter on Bench output and in the Autopilot window; "Before you print" list gets an Autopilot button.

## Next, in order
1. Cable types at location boundaries (novo-cables proposals) as review rows; cores take size from the cable.
2. Ferrule and strip length rows (today they follow the size on the Bench tabs; show them as rows so a person can override one).
3. Ducts and routes (novo-routing) as rows; this is what turns the "~" estimates into routed lengths and moves the meter.
4. Labels as rows (label stock and text), not only wire numbers.
5. Ready to build in the top bar; unlock "Workshop pack" at 100 % (decide with the owner: today the button is never locked).
6. Datasheet PDF to part with Claude: separate PR with the Claude integration thread (cmsg_01TV2QyH4AYw9X2aTmgUsuhC3b51sHqKoWryzmy63oxF2L). The user confirms every field.
7. Load licensed ampacity tables into the plan (it already reads `TableSet::fully_licensed`).

## Open questions
- Below 1.5 mm2 there is no table. Planning caps 0.75 mm2 = 4 A and 1.0 mm2 = 6 A (deliberately low) are used until IEC 60204-1 Table 6 is loaded.
- Efficiency and power factor 0.8 are used to estimate a motor current from kW when no nameplate current is stated (marked "estimated").
