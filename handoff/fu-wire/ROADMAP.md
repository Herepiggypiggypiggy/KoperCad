# FU-WIRE: wire production follow-ups

Lane brief: /mnt/project-files/plans/spawn-table.md (FU-WIRE). Branch `claude/project-thread-pda7tq`, repo kopercad.

## Done (PR 1)
- Wire production rules as project properties `wire.*` (`novo_routing::project_rules`): service loop, longest stub, fill limit, bend, ferrule and lug end allowances, lug threshold, estimate minimum and estimate across pages. No core change. Bad values read as the default and are listed.
- One rule set feeds the wire list, cut list, wire production report, job CSV, schedule lengths, design checks and the Cabinet overlay.
- Routing fixes: one wire diameter rule (`wire_diameter_mm`, was 1.4 mm insulation in the reports and 1.6 in routing and checks), cable-core cross-section used for duct fill, checks reuse `wire_connections`.
- Duct tone and label on the Cabinet routes overlay: idle, ok, busy, over, no size; labels with fill and wire count, falling back to shorter forms when the duct is small.
- `kopercad-cli export wirecut` matches File > Export wire cut: `;` and byte order mark into a file, nothing written when there is nothing to cut.
- Wire rules editor in Cabinet > Routes (Wire rules, collapsed).

## Next
- Machine formats only once Tadjiro names the machine; the generic CSV stays the default.
- Rotated devices: anchor facing is `Any`; rotate the facing with the placement.
- Terminal positions from part geometry (PINS-UI lane) in `anchor_of`.
- Menu row for the wire rules (palette command) once the Cabinet menus land.
