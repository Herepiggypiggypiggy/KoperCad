# Crane wires and junctions roadmap
Scope: wires and connection dots of the EPLAN/crane import come from the file's own runs and nodes.
Done: runs.rs (wires end on pins and real junctions, corners at nodes, stubs and loops dropped and counted); pin-to-pin coincident joins are runs too (nets.rs).
Next: after the symbol-artwork lane merges, re-run v70b/v68b and compare rendered pages to EPLAN's PDF (pin positions decide the end legs); look at the 335 "Wires overlap" findings (likely loosely matched symbols); decide what to do with "Unconnected point" (1362 on v70b, pins that face nothing in EPLAN too).
