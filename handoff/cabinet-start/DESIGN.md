# Cabinet start from a real Rittal enclosure: design

Status: design, 2026-11-04. Owner: thread "Start cabinet workflow from a real Rittal enclosure".
Ask (Tadjiro): the cabinet workflow starts by choosing a real Rittal enclosure in a designed panel
like the start screen (#1833), and the cabinet is built from that enclosure and its CAD files.
Everything downstream (heat, door, routing, drilling, EMC, BOM) must use the same enclosure:
see `CHAIN.md`.

## Today (checked in main 210c4ce)

- An enclosure is a `Structure` with `enclosure: true` and an optional `MountingPlate {width, height, margin}`
  (`novo-core` entity.rs). Depth lives in the property `depth_mm` (default 250 in the 3D view).
- `novo_layout::enclosure::Enclosure` (interior width/height/depth, plate setback, door) is a separate,
  typed-in value set from the door dialog (`cabinet_door.rs`).
- `novo_thermal::model::Enclosure` (outside size, wall, material, installation, plate rect) is built by
  `novo-thermal/src/bridge.rs` from the above plus typed settings.
- `novo_layout::sizing::EnclosureCatalog` is the user's CSV list of sizes (name, outside size, plate).
- #1862 reads a real mounting plate from STEP (size, thickness, round holes) and offers "Maker plate".
  A real TS 8 8455.600 enclosure reads as a 21-solid assembly: its box (1150 x 1860 x 883) is not the cabinet.

So three places each hold their own idea of the enclosure, all typed. The new panel makes one record.

## One record: `EnclosureSpec`

Stored on the Structure (first as typed `properties` keys prefixed `encl.`, because only WS-02 changes the
`novo-core` API; a PR to WS-02 proposes the typed field once the panel works):

| Field | Meaning | Source |
| --- | --- | --- |
| article | Rittal article number, e.g. `8455.600` | catalogue row |
| series, kind | TS 8, AX, KX, CM, TP...; wall/floor/free standing | catalogue row |
| outer W x H x D | mm | catalogue row, confirmed by STEP body |
| wall | sheet thickness mm | catalogue / STEP |
| interior W x H x D | mm | derived: outer minus walls, then door inner depth |
| plate | article, W x H x thickness, setback from back wall, holes, cut-outs | catalogue + plate STEP (#1862) |
| rails | TS 35 rails fitted at delivery (position, length) | catalogue + STEP |
| door | hinge side, max angle, inner depth, number of doors | catalogue |
| openings | vents, gland plates, cut-outs, fan/filter positions (wall, centre, size) | STEP body + accessories |
| accessories | roof, plinth, door lock, cooling unit, each with an article number | panel choices |
| envelope | for the 3D heat view: mesh of the body or parametric ribs, roof, plinth, door swing (heat-shell-ui.md section 7, #1856) | catalogue / STEP |
| installation | free standing, wall mounted, row, covered faces, plinth, finish, absorptance (wiring lane reads these; `assumed` fallback) | panel, defaults assumed |
| step_ref | file name and SHA-256 of the user's STEP; the file itself is never stored in the project | drag and drop |
| provenance | per field: `catalogue`, `step` (read from the maker's STEP), `measured` (user measured it) or `assumed` (typed or default); saved with the project; shown behind "?" and in reports | panel |

Honesty rule: a number read from the STEP wins over the catalogue, a catalogue number wins over a typed one,
and a field whose source is `assumed` is flagged in the cabinet report so nobody mistakes it for the maker's.

## Catalogue data

Ship catalogue rows (article, series, outer size, plate size, wall, weight, door, mounting-plate article,
accessory articles) without any Rittal CAD file. Row sources, in order: the user's Rittal BMEcat/ETIM import
(`novo-parts`, ETIM reader exists), the Rittal Data Portal XML the parts importer already reads, and
hand-checked rows from public datasheets for the common TS 8 / AX / KX sizes. Each row says which source it came from.
No row is invented: a size missing from the sources is not listed; the "Other size" path stays (typed, flagged).

## The panel (same look as the start screen)

A centre window (`centre.rs`, the shared window) styled with the start-screen parts (`row_tile`, `card_tile`,
`paint_surface`, `section_label`, `ui_palette()`). One screen, no wizard pages, left to right:

1. **Series and type** (left column tiles): drawn icons per series, free search box ("8455", "600 x 1800").
2. **Size** (centre cards): one card per article with a small front drawing to scale, outer size, plate size.
3. **Mounting plate and accessories** (right column): plate article (default: the one that fits), then toggles
   for roof, plinth, door type, cooling. Each row shows its article number.
4. **Preview**: a 3D view of the enclosure with the plate, rails and accessories, orbit and door slider.
   While no STEP is given the preview is the catalogue box (labelled "box from catalogue"); with the STEP
   it is the real body.
5. **STEP step**: a drop target "Drop the STEP of 8455.600" with the article number big, a button that opens
   the maker page in the user's browser (a plain link, nothing automated, nothing downloaded by the program),
   and a result line (body found, interior, plate setback, openings). A STEP is optional: without it the
   cabinet is built from catalogue data and the chain tells where it is less exact.
6. **Create cabinet** (primary button): one undoable command.

Almost no explanatory text: each section has a "?" with the sentence; empty and error states are one line.
Keyboard: `/` focuses search, arrows move between cards, Enter confirms, Esc closes. Respects Reduce motion.

Where the user finds it: Start screen tile "New cabinet" (replaces the typed size form), Cabinet workspace
empty state, File > New cabinet, command palette "New cabinet", and Cabinet inspector "Change enclosure".

## What Create does (one command, one undo step)

1. Creates the `+CAB` Structure with `enclosure = true`, the plate, `EnclosureSpec` and tag.
2. Places the factory rails and sets plate holes and cut-outs from the plate STEP (#1862 path).
3. Writes the door, vent and gland openings so heat, door and routing read them (see CHAIN.md).
4. Adds the enclosure, plate, rails and accessories to the part list with article numbers (BOM, drilling plan).
5. Keeps existing devices when the panel is used on a cabinet that already has some ("Change enclosure"):
   the result lists devices that no longer fit instead of moving them.

## STEP reading (shared with the Rittal build thread)

#1862's `part3d::read_body` handles a plate. For the enclosure the reader must:
- split the assembly into solids and pick the body (largest closed shell with a thin wall and an open front),
  classify the rest as door, roof, plinth, plate, rail, accessory;
- measure outer size, wall thickness, interior and plate setback from the body;
- find openings (cut-outs in walls) as through-holes of the body;
- never trust the whole-file bounding box (finding 2 of `REAL-FILES.md`); compare with the catalogue size and
  report a mismatch in one line.
The STEP health-report thread owns the health checks; the Rittal build thread offered to take the body reader.
This thread consumes the result type; it does not duplicate the reader.

## PR plan (each from main, none stacked, each names where the user finds it, QA at 1366x768 and 1920x1080)

| PR | Content | Depends on |
| --- | --- | --- |
| E0 | This design and CHAIN.md | none |
| E1 | `EnclosureSpec` model, properties keys, catalogue rows and loader, tests | #1862 merged |
| E2 | Enclosure STEP body reader (Rittal thread) | #1862 |
| E3 | The panel: series, size, plate, accessories, catalogue preview, Create command | E1 |
| E4 | STEP step in the panel: drop target, real-body preview, openings | E2, E3 |
| E5.. | One PR per downstream area from CHAIN.md | E1 |

## Legal and privacy

CADENAS PARTcommunity forbids automation: the program never fetches, scrapes or signs in. The user downloads the
STEP in his browser; the program reads it locally. STEP files stay out of the repository, the project file and the
QA evidence; evidence uses synthetic enclosures. Only catalogue data without CAD geometry ships.

## Open questions for Tadjiro (batched, not blocking)

1. Which series does he really use (TS 8, AX, KX, CM)? Start with those.
2. A Rittal BMEcat file, to fill the catalogue from the maker instead of hand-checked rows.
3. A measured real cabinet (outer, wall, plate setback) to validate the STEP reader and heat model.
