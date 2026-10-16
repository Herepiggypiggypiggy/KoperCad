# Migration and interop (WS-24, gap #10)

Status: first slice built (STEP, AutomationML, DWG); the rest is planned below.
Extends `docs/design/interop.md`. Brief: `docs/AMBITION-REVIEW.md` section 4, gap 10.

## 1. What the designer thinks

"I have ten years of projects in another tool, a customer who sends DWG, a
mechanical colleague who wants my cabinet in their CAD, and a PLC engineer
who wants my I/O. I will not retype any of it, and I do not want a converter
that quietly drops half of it."

So the rule of this lane is the one the review set for every lane: **what is
carried is counted, and what is not carried is named**. Every engine returns an
`interop::report::Report` (what moved, what was left out and why, notes). The
app shows it before an import is applied and after an export is written; the
CLI prints it.

## 2. Licences: what we use and what we do not

| Format | How | Licence position |
| --- | --- | --- |
| STEP (ISO 10303-21, AP214) | We write it ourselves from the published structure of the standard's physical file; nothing is copied from a CAD kernel | Open standard, no SDK |
| AutomationML (IEC 62714, CAEX 2.15) | We write and read the XML ourselves | Open standard, no SDK. Standard AutomationML base role names are used; KoperCAD's own roles live in `KoperCADRoleClassLib` inside the file |
| DXF | Existing reader and writer (`dxf`) | Written from the public format description |
| DWG | **Not read by KoperCAD.** The user's own free converter runs as a separate process, then the normal DXF import runs | DWG has no public specification and the Open Design Alliance licenses its libraries per product. LibreDWG (GPL) and the ODA File Converter (free under ODA's terms) are never linked, bundled or redistributed; the user installs one, so its licence is between them and its maker |
| EPLAN | Part exports are read as the user's own data (`eplan`). No EPLAN library content, symbols or file formats are copied | See `docs/design/interop.md` section 5 |

The converter for DWG must also work without admin rights: LibreDWG's portable
zip unpacked under `%LOCALAPPDATA%\KoperCAD\tools\libredwg` is found by itself,
or the person picks a program with "Choose DWG converter".

## 3. Built in this slice

### 3.1 STEP export of the cabinet (`novo_interop::step`)

- Source: the 3D view's scene of an enclosure (`novo_view3d::build`), so what
  the person sees in the 3D view is what the mechanical colleague gets.
- Output: one STEP assembly per enclosure with a mounting plate. A product per
  item (housing, mounting plate, DIN rail, duct, each device by its tag, door),
  placed by its pose, coloured like the 3D view, door and housing see-through.
- Exact boundary representation (planar faces, straight edges), never a mesh.
  A DIN rail and a duct are the few boxes their section is made of.
- The enclosure is written as a **housing** (2 mm walls, open front, back wall
  behind the plate), not a solid block, because a solid block would swallow
  everything inside it in the receiving CAD. The wall thickness is a constant
  (`HOUSING_WALL_MM`) until the enclosure's catalogue data carries a real one.
- Depth comes from the enclosure's `depth_mm` property, 250 mm otherwise.
- Not carried (counted in the report): wires (not yet solids), a part's own
  maker geometry (written as its bounding box), an open door (written closed).
- Validation: the file is read back by KoperCAD's own STEP reader in a test, and
  was checked with OpenCASCADE: reads without errors, valid shape, solids with
  the right volumes and bounding box, product names and the assembly tree
  preserved. Deterministic output (same project, same bytes).
- Where: File > Import and export > Cabinet as STEP; command field `export step`;
  `kopercad-cli export step project.novo -o cab.stp [--enclosure +CAB1]`.

### 3.2 AutomationML (`novo_interop::aml`)

- Export: the plant becomes one CAEX instance hierarchy. Locations (`+CAB1`),
  plant sections (`=A1`) group devices; terminal strips carry their terminals as
  interfaces (level, jumper, kind, article); cables carry their cores; placed
  devices carry the standard AutomationML `Frame` attribute; nets are elements
  with one interface per member pin and one internal link per pin, so the
  netlist (junctions included) is exchanged without page geometry.
- Read: KoperCAD's own file, or another tool's AutomationML. The tag may be in
  an attribute called `Tag`, `TagName`, `DeviceTag`, `Designation`,
  `ReferenceDesignation`, `BMK` or `Kennzeichen`, or be the element name; the
  manufacturer and order number accept English and German names. A location
  above a device completes the device's tag. Attributes the project has no field
  for are kept as `aml.<name>` properties, so nothing is lost silently.
- Import is a preview first (`AmlPlan`): what would be added, what already
  exists, what was not carried. Applying is one undo step and only adds.
- Not carried over on import (counted): connections (a wire needs symbols on a
  page; a later slice will generate pages from nets), elements without a usable
  tag, repeated tags, cabinet positions.
- Where: File > Import and export > Plant as AutomationML / Import AutomationML;
  command field `export automationml`, `import automationml`;
  `kopercad-cli export aml project.novo -o plant.aml`.

### 3.3 DWG through a free converter (`novo_interop::dwg`)

- The file's version mark is checked first (so a PDF renamed `.dwg` or a
  pre-R13 file gets a clear message before anything runs).
- LibreDWG `dwg2dxf` and the ODA File Converter are driven by their own command
  lines as separate processes, with a time limit, no console window and output in
  `%LOCALAPPDATA%` cache. The result goes through the normal DXF import, with its
  counted list of skipped entities.
- Conversion runs in a background thread; the app stays usable.
- Where: File > Import and export > Import DWG as background / Choose DWG
  converter; command field `import dwg`; `kopercad-cli convert-dwg in.dwg -o out.dxf`.

## 4. Next slices (in order)

1. **Bring a standard in.** Title blocks and frames from DXF (the user's own
   standard folder, kept off the repo), symbols from DXF and SVG into a user
   symbol pack (connection points picked on import, preview before adding),
   macros from public exchange formats. Fixtures are the user's folder; tests use
   synthetic equivalents.
2. **Project-level migration.** Device lists, terminal plans and cable lists from
   another tool's Excel/CSV export mapped to KoperCAD records (the `sheets`
   vocabulary plus a mapping table the user can save), nets from AutomationML
   turned into generated pages with real wires, round trip report ("this project
   came in with N of M objects; here is the rest").
3. **PLCopen XML** and the PLC hardware config (shared with the PLC lane).
4. **STEP depth.** Wires as swept tubes, maker geometry embedded for parts that
   have it, door at its real angle, real enclosure wall thickness from the
   catalogue.
5. **Python API for bulk work** over all of the above (WS-28 owns the bindings).

## 5. What is a model, what is exact

STEP geometry is exact for what it contains; its *content* is the 3D view's
model (housing walls are a constant, a device without depth is 60 mm and marked
as guessed in the view). AutomationML carries what the project knows and says
what it drops. DWG fidelity is the converter's, and KoperCAD says which
converter was used.
