2026-10-22 11:10 UTC thread started from the heat/layout findings (CV-200, 1.48 M cells, not converged).
2026-10-22 11:25 UTC reproduced the grid with the CV-200 file (297x172x29), found that planes of small bodies and slivers beside hot devices set the cell count.
2026-10-22 12:00 UTC reduction built (lump, anchored lattice, smearing, expand), wired into CabinetSolver; CV-200 Coarse 9.7 k cells converges in 35 s.
2026-10-22 12:40 UTC found the ordinary example cabinet needs 199 k cells at Coarse; hot-device clustering and a volume-growth limit added; CV-200 grid study Coarse/Medium/Fine converges.
2026-10-22 13:20 UTC tests (reduced vs full on a small cabinet), Heat legend lines, QA scenario 40 at 1366x768 and 1920x1080.
