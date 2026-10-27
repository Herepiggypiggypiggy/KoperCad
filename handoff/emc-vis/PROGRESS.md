2026-10-22 14:30 UTC first EMC view built: interface, overlays, stub, panel, example, QA at 1366 and 1920 dark and light, tests green
2026-10-22 14:50 UTC review fixes pushed, head e8451324162f6be624f617778289134a5582ebf6
2026-10-22 16:05 UTC #1388 and #1391 merged; 2D plate mirror built (emc_plate.rs), QA 97 at both sizes, tests green
2026-10-22 16:20 UTC #1396 R1 item: plate view says how many wires with no duct route are left out (EmcState.left_out, legend line, test); exposure map cached per picture; 2D badges step down when they overlap; QA 97 expects 'Indicative model' and scrolls the card to Close.
2026-10-22 17:10 UTC #1396 merged; follow-up PR: legend says 'Route all', left_out reset when the layout cannot load, QA 97 @sizes, QA 100 picture with 4 wires left out
2026-10-22 17:20 UTC #1396 merged; follow-up PR: legend says 'Route all', left_out reset when the layout cannot load, QA 97 @sizes, QA 100 picture with 4 wires left out
2026-10-24 UTC schematic net-class overlay built on main 975b1d7: wire looks by class, tags, panel, wire card with set class (one undo step), command view.emc_nets, QA 154 at both sizes dark and light, evidence /mnt/project-files/emc/evidence-schematic/
2026-10-24 UTC #1453 merged. Cable card and one-click shield fixes in the schematic net-class panel; EMC example gets motor cable =A1+CAB-W2; QA 156 (1366) and 157 (1920).
2026-10-24 UTC cable cards (#1459, #1462) merged; plate now draws bond outlines and shield ends (QA 163, 164), evidence /mnt/project-files/emc/evidence-plate-bonding/
2026-10-24 UTC R5 review on #1464: added shields fixture emc-cabinet-shields.novo (all three screen ends, good bond), tests, legend lines and ring hover for the shield rings, dark backing stroke; QA 165/166.
2026-10-27 UTC findings without an engine position (EMC-04, EMC-05, EMC-12) get a hot spot at their device or run on the plate and in 3D (adapter place_of); QA 300/301.
