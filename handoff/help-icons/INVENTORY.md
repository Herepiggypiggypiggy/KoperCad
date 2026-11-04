# Explanation text inventory (baseline for the remaining areas)

Measured 2026-11-03 on main 5108a1e plus the first help-icon PR, at 1366x768, from the running app: every label of more than 8 words in the widget tree after each screenshot of 112 scenarios (a sample of every area, about one in six). Source: `KOPERCAD_QA_TEXT_AUDIT=<file> scripts/qa.sh ...` writes scenario, size, shot, words, text.

Static count for the rest (source literals of 8 or more words, hover texts included, tests excluded): 1,244 literals, 16,120 words, in 183 files of `crates/novo-app/src/app/` (biggest: bench.rs 773, cabinet.rs 742, preferences.rs 378, heat_flow.rs 345, cabinet_routes.rs 336, model_labels.rs 329, view3d_runs.rs 319, workshop.rs 314, emc_view.rs 299, live.rs 296).

Runtime sample (distinct labels, words) by scenario area, after the 3D Heat/Hoist/Drive run changes. Goal: close to zero outside the "?".

| Area | Distinct long labels | Words |
|---|---|---|
| Learning | 57 | 1251 |
| Parts | 84 | 1199 |
| Macros | 53 | 1076 |
| Learn | 41 | 895 |
| Cabinet | 42 | 827 |
| Reports | 27 | 615 |
| Assistant | 29 | 591 |
| Fixtures | 28 | 543 |
| Schematic | 28 | 500 |
| 3D | 22 | 390 |
| Check | 17 | 254 |
| Simulate | 19 | 228 |
| Add-ons | 16 | 221 |
| Terminals | 14 | 197 |
| Live | 14 | 180 |
| Heat | 10 | 154 |
| Hostile | 9 | 144 |
| Examples | 10 | 118 |
| Layout | 9 | 115 |
| Windows | 10 | 102 |
| Options | 6 | 96 |
| Journeys | 7 | 95 |
| Start | 8 | 87 |
| Editor | 4 | 84 |
| Commissioning | 7 | 82 |
| Workspaces | 5 | 60 |
| Cables | 4 | 46 |
| Regressions | 1 | 42 |
| Help | 3 | 39 |
| Command search | 2 | 24 |
| Revisions | 2 | 24 |
| Checks | 2 | 21 |
| Files | 1 | 11 |
| Appearance | 1 | 10 |
| Command field | 1 | 10 |
| Palette | 1 | 9 |
| **Total** | **594** | **10340** |

Not in the sample: hover tooltips (hidden until pointed at, not counted here), the painted canvas, and text shorter than 9 words. The status bar hints ("Q or Space to place · Ctrl+K to search", the X/Y readout with "Home zoom to fit") appear in 40 scenarios.

The full list (every long label with its scenario) is `/mnt/project-files/help-icons/INVENTORY-full.md`; regenerate it with `KOPERCAD_QA_TEXT_AUDIT=<file> scripts/qa.sh --size 1366x768 <scenarios>`.
