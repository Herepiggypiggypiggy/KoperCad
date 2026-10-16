# Bulk parts import (`import-parts.cmd`, `kopercad-cli parts`)

Gets real maker data into the parts catalog with one command. Design limits
from `PARTS-LIBRARY-DESIGN.md` hold: **the tool never contacts a maker site**.
You download the catalog files (your own login, the maker's terms); the tool
reads, checks and merges them. No admin rights; it writes only under
`%APPDATA%\KoperCAD` (the parts catalog, its `.bak`, `import-report.txt`, `inbox`).

## Run it

1. Double-click `import-parts.cmd` in the `KoperCAD-latest` folder. First run
   creates `%APPDATA%\KoperCAD\inbox\` with a folder per maker and opens it.
2. Save downloads into the maker's folder (BMEcat `.xml`, `.csv`, or a `.zip`
   holding BMEcat). A maker without a folder: make a folder named after it.
   Wholesaler files that name the maker in every row go in `_wholesaler`.
   Spreadsheets must be saved as CSV first.
3. Double-click `import-parts.cmd` again. Re-run after every new download.

`kopercad-cli parts guide` lists, per maker, the page to use, whether a login
is needed and where to save. `--dry-run` reads and reports without writing.
`--inbox DIR` and `--catalog FILE` override the locations (used by tests).

## What it does

Same readers and maker recipes as the one-click Import in the app
(`novo-parts::recipes`), over a whole folder:
BMEcat/CSV/ZIP -> maker recipe (SI values, connection, wire range, dimensions,
ETIM/ECLASS, price, lifecycle) -> checks -> merge by (maker, order number).
Same file twice changes nothing; a newer file updates; a `.bak` of the
catalog is kept. Checks (warnings, nothing is dropped): maker present,
description present, ETIM class `EC000000`, dimensions positive and under
3000 mm, order number looks like the maker's, terminals vs poles.
The report lists per maker: parts, new, updated, same, without size, without
ETIM, completeness (Good/Fair/Poor), unmapped features and parts to check.

## Limits

- Mappings were written from public wording and tested on synthetic samples
  (`crates/novo-parts/tests/golden/`); the first real file will show features the
  report lists under "did not map". Send that list to fix the recipe.
- Duplicate parts across files: the later file name (alphabetical) wins.
- Geometry (STEP/DXF) is not bulk imported; it stays one-click per part.
