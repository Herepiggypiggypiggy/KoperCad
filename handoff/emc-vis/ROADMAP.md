# EMC visualisation: roadmap

Scope: how EMC shows in KoperCAD (3D cabinet, plate, schematic). Reads findings only through the `Picture` interface; the rule engine thread owns `novo-emc`.

Done
- Concepts (corridors and layers chosen), `Picture` interface, overlays, exposure map, stub, 3D EMC view, panel, legend, EMC example, QA scenario 96-3d-emc.

Next, one PR each
1. DONE (PR 2D plate): EMC on the Cabinet plate (`emc_plate.rs`, Cabinet toolbar EMC, Ctrl+K `cabinet.emc`, QA 97).
2. DONE (PR schematic net classes): View > EMC net classes, `emc_schematic.rs`, QA 154.
3. Loops, bonding and shields from real data; per-cable EMC card.
4. Adapter from `novo-emc` (engine PR #1387) replacing the stub; stub findings carry no rule id until then; fix buttons wired to commands.

Open: hot spot badges overlap when two sit close; frame the camera on the cabinet when the view opens.

3. DONE (PR cable cards): cable card with findings and Apply (add shield, ends all round) in the schematic panel, QA 156/157.
