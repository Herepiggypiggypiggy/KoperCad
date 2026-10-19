# EPLAN-SYMBOLS roadmap
Goal: an EPLAN export loads into KoperCAD and looks as it did in EPLAN, with KoperCAD's own artwork (never EPLAN's).
1. DONE (PR "Own symbols for the crane import boxes"): 11 symbols in both packs, table rows, `SymbolMapper::map_fitting`, `fit_geometry`.
2. NEXT (needs the importer, PR #1031 `eplan/pxf/draw.rs`): terminals and 1-pin placements go through the mapper; rotate single points to EPLAN's facing; pin order by `fit_geometry`.
3. LATER: more families as real projects show boxes (PLC modules, drives, sensors with 4+ pins).
