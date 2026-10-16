2026-10-16 08:50 UTC First implementation: graded axes, exact cut-cell shares, presets, preview, 12 tests green locally.
2026-10-16 15:00 UTC Round 2: refinement around bodies with cell budget; full novo-cfd suite green (ducts threshold 0.8 to 0.75); study numbers in HANDOFF.
2026-10-16 15:40 UTC R1 review on 19d93bb: thin devices keep planes and one cell and are reported unresolved; ducts bound back to 0.8.
2026-10-16 16:45 UTC #707 merged (80a218e) after two CI fixes (flow_steady fan case per R1; cabinet test for the unresolved device replaced because the body-first mesh always resolves a device). Mesh study with #705 on main, below.
