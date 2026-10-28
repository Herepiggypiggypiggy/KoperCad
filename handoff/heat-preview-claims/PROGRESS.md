2026-10-28 12:50 UTC Thread started from Tadjiro's report: lesson says -T1 is hottest, preview shows T1 43 C and F1/F3 48 C.
2026-10-28 13:10 UTC Reproduced in a test (heat_preview_tests): preview T1 43.0 C, 2D solver T1 78.8 C (hottest). Cause: fixed +4..11 K stratification and -9 K inlet term dwarfed a 20 W source (peak +4.2 K).
2026-10-28 13:30 UTC Preview kernel rewritten and fitted to the 2D model; largest gap 6.4 K on the lesson cabinet at its own load (bound enforced in tests: 10 K). The error grows with load (R1: about 20 K at 2x, over 40 K at 4x, too hot).
2026-10-28 14:00 UTC Audit: claims tests for the cabinet-track lessons; stale numbers found and fixed in cut-lengths, panel-labels, wire-routing.
