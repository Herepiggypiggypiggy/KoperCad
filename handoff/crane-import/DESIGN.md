# Crane configurator project import: design note (2026-10-19)

Goal: File > Import > Crane configurator project reads what Tadjiro's crane configurator produced (the first real project, output v95) and creates a real KoperCAD project: pages with drawn symbols,
devices, terminals, cables with cores, wires, parts. The import report says what is exact, what is inferred and what is
left out. Knowledge only from the private repo: no EPLAN macros, symbol library, manual text or motor data enters the
public repo, and the real v95 file stays on the private mount.

## 1. What a configurator project is (checked on the real v95 files, 2026-10-19)

The configurator is Python that assembles an EPLAN project and lets EPLAN export it. One order per version folder,
`output/custom/<name>/<vNN>/`:

| File | What it is | Used as |
| --- | --- | --- |
| `<order>.epj` (206 MB for v95) | EPLAN 2024 PXF project export, PxfVersion 1.23, Build 21667 | the drawing truth: pages, symbols, nodes, texts, cores, interruption points |
| `<order>_Projectcontrole.md / .xlsx` | the configurator's own 72 automatic checks, verdict and open points | shown in the report as "the source says" (not recomputed) |
| `hernoemen.json / .md` | tag renames the configurator applied (was, becomes, sheet, column) | provenance only: the epj already holds the new names |
| `<order>_ontwerp.txt` | design report with open points (S-numbers) | open points go into the report notes |
| `images/<order>/*.png`, `layout_*.png` | page pictures EPLAN drew | not imported (KoperCAD draws its own); optional page background later |
| `custom/<name>/project.json` | the choices: brand, order, customer, drives, motors, hoist models, radio, features | project info and a "configurator choices" note |
| `*.edb/` | EPLAN's binary database | never read (the epj is the same data) |

So "configurator project" = the version folder (or just the `.epj` inside it). A folder given instead of a file picks the
newest `vNN` and the epj in it.

### 1.1 Inside the v95 epj (counts from the real file)

- 65 pages (`O4`): 23 schematic pages with function code and location (=10D10 ... =19N14), 3 cover pages, 36 generated
  report pages (=DOC: contents, lists, cable and terminal plans; made of `O52` report cells) and 3 layout pages.
- 467 symbol placements (`O17`) of 203-odd repeated tags -> 98 devices after the configurator's own count; 417 connection
  nodes (`O42`); 198 interruption points (`O70`); 122 cable cores (`O59`); 11336 texts but most belong to the
  embedded symbol library, not to pages.
- Wires are not objects. Drawn connections are implicit: nodes and pins that face each other join (rules already in
  `eplan::nets`, from the crane repo's own checker). `O31` lines on schematic pages are only frames and height-index marks.
- The file embeds EPLAN's symbol library (2711 `O20` definitions, 192 000 lines, 27 000 boxes, 12 MB to 199 MB). Placements
  point into it with `A1381/2/3` (number / variant / set). A definition carries a multi-language name (`P16011`) and its
  pin designations (`A1046`, e.g. `L1 T1 L2 T2 L3 T3`). We read these transiently to map symbols and learn pins; they are
  never stored or shipped (the user's own file, user-run import).
- Pin designations of a placement are in `A185` (`1;2`), orientation in `S39x761`, position in `S40x1201 A762` (mm, y up).
- Structure names (function codes) are in `O6` with titles.

## 2. Mapping

| Configurator / PXF | KoperCAD | Quality |
| --- | --- | --- |
| project data (`O14`, `project.json`) | project info: order number, customer, title, revision, voltage | exact |
| `O6` structure rows + page codes | sections `=10D41`, locations `+K1BG1` with titles | exact |
| `O4` schematic page | page named `=fn+loc/nn title`, A3, page kind schematic | exact |
| `O4` report (=DOC) page | not imported: KoperCAD generates its own reports | left out, counted |
| `O17` tag (`P150`) | device `=fn+loc-X` ; repeated placements = one device with several drawings | exact |
| `O17` part number `P20100_1` | part (manufacturer, order number, type) assigned to the device | exact (names as the file writes them) |
| `O17` terminal (`P20030`) | terminal strip + terminal | exact |
| `O17` symbol key -> `O20` name + pins | KoperCAD library symbol via `eplan::symbols` mapper; generic box that keeps every pin when nothing fits | inferred (basis shown per device) |
| placement position / orientation | symbol position and rotation on the page, mm | exact for position, orientation table checked on the real file |
| `O42` nodes | junctions | exact |
| facing pins / nodes | wires, via `eplan::nets` | inferred by EPLAN's drawing rules, same rules as the crane repo's checker; counted per page |
| `O70` interruption points | cross-reference pairs by full name, odd ones reported | exact pairing rule |
| `O59` cable core + `O17` cable | cable with cores, core numbers | exact |
| `O30` texts on a page | text on the page, first language that exists | exact |
| `O52` report cells, `O31/O32/O89` on pages | not carried (frames, height index and report layout are KoperCAD's) | left out, counted |
| `O20/O5/O144/O45...` symbol library | not carried | left out |

## 3. Risks

1. Size: 206 MB, 28 s in a debug build for the current reader. Streaming stays; the library part is skipped fast.
2. Reader counts library texts and lines as page objects (they come after the last page). First fix, with a test on the real file.
3. Symbol mapping is by name: many EPLAN names have no KoperCAD twin yet. The generic box keeps pins so nets still hold.
4. Net rebuild quality on real symbols depends on pin offsets from `O20` (`S48x823` rows); verify against the checker's own
   result (122 cores, 198 interruption points, "C03 blocked run" warnings listed in `_Projectcontrole.md`).
5. Customer data: fixtures in the repo are synthetic or anonymised and tiny; the real v95 test is opt-in with
   `KOPERCAD_CRANE_EPJ=/path/to.epj` and skips silently otherwise (same rule as `KOPERCAD_PRIVATE_SAMPLES`).
6. Other versions: v96 scripts exist; the reader must not depend on v95 (version folders, missing optional files).

## 4. Slices (one draft PR each, no stacking)

1. Source layer: folder or epj, `project.json`, project info, provenance and the "source says" notes; reader scoping fix;
   File > Import > Crane configurator project; `kopercad-cli import crane`; report with exact / inferred / left out.
2. Drawn pages: symbols with mapping, junctions, texts, wires, interruption points, with real-file counts as tests.
3. Polish from real use: symbol table suggestions saved to the user's table, optional page pictures, report export.
