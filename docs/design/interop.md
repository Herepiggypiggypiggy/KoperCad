# Import and export: DXF and EPLAN parts (WS-24)

Status: proposal; the engines in `novo-interop` follow it. It extends
`docs/DESIGN.md`; where the two disagree, DESIGN.md wins. Excel round trip
is a later step (§7).

## 1. What the designer thinks

"I have a mechanical drawing of the cabinet, a customer's DXF, and a parts
list from my old EPLAN." The designer wants to **bring the drawing in as
background**, **send a page out to a mechanical colleague**, and **get the
parts they already maintain without retyping them**. Nothing about this
should ask for an account, a server or admin rights, and nothing should
silently lose data: what is skipped is counted and shown.

Problems to avoid (not copy):
- DXF dialogs with twenty options (version, units, layers, fonts). Here
  there are none: the export writes millimetres in the most widely read
  DXF dialect, the import reads any ASCII DXF and tells what it skipped.
- Parts import that needs the original program installed. Here an EPLAN
  parts export file (XML or the delimited text export) is read directly.
- An import that overwrites the user's catalog. Here an import only
  produces a *preview* (`novo_parts::MergePreview`) the user confirms.

## 2. Layers of the crate

| Layer | Knows | Does not know |
| --- | --- | --- |
| `dxf` | The DXF file format: layers, lines, polylines, circles, arcs, text. DXF axes (y up, degrees counter-clockwise). | The project, the canvas, KoperCAD's y-down page. |
| `eplan` | EPLAN parts exports (XML, delimited text) and how their fields map to a catalog part. | Files on disk, the user's catalog location. |
| `bridge` (later PR) | Page scene to `dxf::Drawing` and back; the y flip lives only here. | The DXF text format. |

Every layer is a pure function from text/bytes to values and back. Disk
access, dialogs and "which page" belong to the callers (`novo-app`,
`novo-cli`, Python).

## 3. DXF export

- Dialect: ASCII DXF `AC1009` (AutoCAD R12). It has no handles, no object
  tables beyond layers and linetypes, and every CAD program, viewer and
  CNC tool reads it. Polylines are `POLYLINE`/`VERTEX`/`SEQEND`.
- Units: millimetres (`$INSUNITS` 4). The engine never rescales on write.
- Entities: `LINE`, polyline (open or closed), `CIRCLE`, `ARC`, `TEXT`.
  Multi-line text is one `TEXT` per line (R12 has no multi-line text).
- Non-ASCII text is written as `\U+XXXX`, which every reader decodes; the
  file itself stays 7-bit ASCII so no code page can garble it.
- Layers are kept with their colour (AutoCAD colour index). Names are
  sanitised (`<>/\":;?*|=\`` become `_`). Layer `0` always exists.
- Deterministic: same drawing, same bytes.

Page export (bridge) maps the page scene's layers to DXF layers by name
(`Frame`, `Wires`, `Symbols`, `Text`), flips y so the page opens upright,
and draws symbol artwork as geometry, not as blocks, so a recipient needs
nothing from us.

## 4. DXF import

- ASCII DXF of any version from R12 on. Binary DXF is refused with a clear
  message ("save as ASCII DXF"), not misread.
- Read: layers (colour, off/frozen state), `LINE`, `LWPOLYLINE`,
  `POLYLINE`/`VERTEX`, `CIRCLE`, `ARC`, `TEXT`, `MTEXT` (formatting codes
  stripped, `\P` becomes a new line), `INSERT` of a block (exploded with
  the insert's offset, scale and rotation; nested up to 8 deep).
- Polyline bulges become `ARC` entities, so a rounded rectangle arrives
  as lines and arcs, not as an approximation.
- Everything else (`SPLINE`, `ELLIPSE`, `HATCH`, `DIMENSION`, 3D solids,
  ...) is skipped and counted by type in the result, so the import
  dialog can say "12 splines and 3 hatches were not imported".
- Encoding: UTF-8 when valid, else Windows-1252 (older files), plus the
  `%%d %%p %%c` shortcuts and `\U+XXXX`.
- Units: `$INSUNITS` is read and `Drawing::scaled_to_mm` converts; a file
  without units is taken as millimetres.
- Imported drawings are **background geometry**: they never create
  devices or wires. How the page stores them (an underlay on the page) is
  a `novo-core` decision (WS-02) and is not part of the engine PRs.

## 5. EPLAN parts import

EPLAN's parts management exports **XML** (property ids such as
`P_ARTICLE_PARTNR`, `P_ARTICLE_TYPENR`, `P_ARTICLE_MANUFACTURER`,
`P_ARTICLE_DESCR1`) and a **delimited text/Excel** export with one part
per row. The import reads both, tolerantly:

- XML: every element that has an order-number property is one part; the
  property values are read from attributes or child elements, so small
  differences between EPLAN versions do not matter. A small built-in XML
  reader is used (no new dependency, offline).
- Delimited text: handled by `novo_parts::import_catalog`'s header
  vocabulary (English, German, Dutch) after EPLAN's own column names are
  mapped to it.
- Mapping to `CatalogPart`: manufacturer, order number, type number,
  description (first non-empty of `DESCR1..3`), dimensions
  (width, height, depth, mm), plus technical data kept as `data` with
  SI keys when the unit is known. Unknown properties are kept under an
  `eplan.<id>` key so nothing is lost and a later export can restore it.
- The result is a list of `CatalogPart`s plus a report (parts read, rows
  skipped with the reason). Merging into the user's catalog uses the
  existing `Catalog` merge preview; this crate never writes the catalog.

The format knowledge comes from EPLAN's published property names and
public example exports, not from a licensed copy of the program; the
reader is deliberately tolerant and each real-world export that fails
becomes a fixture and a test.

## 6. Delivery in small PRs

1. This design, `novo-interop` crate, `dxf` model, writer, reader, tests.
2. `eplan` XML and delimited import, tests.
3. Bridge: page scene to DXF, DXF to a drawing the app can show as
   background (needs the WS-02 underlay decision), CLI/Python entry points.
4. Excel round trip for the parts list and the I/O list (WS-24, after
   WS-17's PLC tag table export).
5. Spreadsheet round trip for devices, terminals, wires and cables:
   `docs/design/spreadsheets.md`, module `novo_interop::sheets`.

## 7. Out of scope here

DWG as a native format (closed; a free converter turns it into DXF, see `interop-migration.md`), DXF 3D, import of symbols as
KoperCAD symbols (a later, separate design), writing to EPLAN's own project
formats.
