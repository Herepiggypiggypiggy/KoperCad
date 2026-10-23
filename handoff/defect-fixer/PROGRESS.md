# Audit defect fixer: progress

2026-10-21 Acceptance round 2: B-08 #1271, B-07 #1272, B-11 #1273, B-12 #1276, B-09 #1274, B-10 #1277 merged; nits #1282 merged. B-10 first push was NOT READY (history clear also ran on the mid-session rules toggle); fixed to clear only when history was empty, test added.

2026-10-23 Creative round 30 S3s: Wire heat in the ducts fold stays with a "Route the wires first" line when no wires are routed; EMC toolbar button tooltip says Off/On (it is a switch, not disabled). Scenario 260.
2026-10-23 Heat colour bar: ticks above ~200 C smeared (27 labels on 150 px). Now round 20/50/100 C steps, 26 px apart (heat_legend.rs); scenario 250 with a 285 C fixture; PR open (from Lead A relay of R3 finding).

2026-10-23 Critic C-60: PLC address style (IEC, Siemens, Rockwell, Plain) is now chosen in the Add PLC card dialog and the I/O list header; Ctrl+K siemens/rockwell/address style finds both. Scenario 280.
