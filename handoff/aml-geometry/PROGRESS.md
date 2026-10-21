2026-10-19 13:34 UTC Thread started from Tadjiro's report: AML import does not look like the page.
2026-10-19 13:50 UTC Found causes: library symbols placed by first pin only (8 vs 16 mm pitch), nets chained by x order (wires through terminals), boxes off-centre.
2026-10-19 14:10 UTC Switched to the exporter thread's `Wire` runs; importer joins runs into wires, fits symbols to pins.
2026-10-19 14:30 UTC Real project: 745 wires, 0 crossings, 1 open end (EPLAN's). Invented fixture + 6 geometry tests; pictures in qa/aml-geometry.
