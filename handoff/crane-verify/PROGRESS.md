2026-11-25 15:20 UTC harness v1 on v70b: per-pin classes, pin-vs-file offsets, vector compare with EPLAN PDF and overlay PNGs; numbers sent to coordinator.

## Free side of a wired terminal (novo-checks)
The free side of a wired terminal or plug pin is now an info ("other side of a wired terminal"), not a warning. On v70b, 468 of the 1519 pins without a wire move from warning to info; touching and truly open pins keep the warning. `pdf_compare.py` render fixed for pins with extra fields.
