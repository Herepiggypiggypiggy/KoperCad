# Protection and cable engineering: roadmap

Scope: gap #2 of docs/AMBITION-REVIEW.md. IEC 60364-5-52 ampacity, Ik per node (IEC 60909), curves, selectivity
and back-up, SCCR, one-click starter sizing. Validation by worked examples. Design in docs/PROTECTION.md.

Done (branch claude/project-thread-utbmbn, stacked on #412 and #454):
- Versioned table set with provenance and licensed CSV override.
- Ik network, trip bands, selectivity/back-up, circuit report, JSON spec, `kopercad-cli calc`.
- SCCR (UL 508A, IEC 61439), starter sizing, Cable rating problem kind, transformer supply.

Next, in order:
1. Verify tables against the user's licensed IEC 60364-5-52; add A1/A2, D1/D2 buried, E/F/G free air, aluminium, > 120 mm², soil and ground factors, other grouping lines (tray, perforated).
2. Reproduce IEC 60909-4 worked examples as tests (needs the user's copy or a public worked example).
3. Maker data: trip curve and let-through import (via Parts library schema v2), selectivity tables per maker.
4. UI: Checks workspace "Calculations" tab, device inspector "Protection" section, Ctrl+K "Size this circuit", Project settings > Electrical > Tables (load licensed CSV). Needs UI integration thread.
5. Network from the drawing: build `Network` from the netlist (transformer, boards, cables) so Ik reduces along cables in the Issues list.
6. Generator/motor sources, meshes, IEC 60287 thermal check of the ampacity tables.

Open questions: which licensed documents Tadjiro has (IEC 60364-5-52, IEC 60909-0/-4, UL 508A); which maker tables to import first.
