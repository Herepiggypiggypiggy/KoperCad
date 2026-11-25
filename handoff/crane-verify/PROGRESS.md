2026-11-25 15:20 UTC harness v1 on v70b: per-pin classes, pin-vs-file offsets, vector compare with EPLAN PDF and overlay PNGs; numbers sent to coordinator.

## Free side of a wired terminal (novo-checks)
The free side of a wired terminal or plug pin is now an info ("other side of a wired terminal"), not a warning. On v70b, 468 of the 1519 pins without a wire move from warning to info; touching and truly open pins keep the warning. `pdf_compare.py` render fixed for pins with extra fields.
2026-11-25 17:10 UTC re-run on main (#1994/#1995/#1997 in): EPLAN wire length covered 49% -> 89%, ours on EPLAN lines 99%, 0 of 8252 pins off the file, wire_overlap 2, no_main 4, dangling_junction 0. Found node kind 255 joining a drive's R/S/T/PE through four rays (EPLAN draws nothing there): legs now none; EPLAN nets 1889 -> 1925. Harness lists nets that join several EPLAN nets; pdf_compare ignores yellow table grids.
