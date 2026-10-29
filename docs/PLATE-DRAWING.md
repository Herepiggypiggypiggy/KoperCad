# Plate drawing and drill table

R5 of `docs/PRO-PANEL-GAP-ANALYSIS.md`: a dimensioned mounting-plate drawing and
a numbered drill-hole and cut-out table, as PDF and DXF. Generic output only;
machine formats wait until the workshop names its machine.

## Where it lives

| Piece | Crate | Entry point |
| --- | --- | --- |
| Hole and cut-out data, numbered table | `novo-layout` | `drill::drill_table`, `model::features` |
| Marks (lines, circles, text) and tables | `novo-layout` | `plate_drawing::documents`, `model::plate_documents` |
| PDF | `novo-pdf` | `plate::export_plate_pdf`, `plate::write_plate_pdf` |
| DXF | `novo-interop` | `plate::plate_dxf`, then `dxf::write_dxf` |

`model::plate_documents(project, structure)` reads the placements (#340) and
the part keys and returns the documents plus the table. Both writers draw the
same marks, so the two outputs cannot disagree.

## Part keys

Read from the first part of the device that has the key (they are the R0 keys,
free-form property text):

| Key | Text | Meaning |
| --- | --- | --- |
| `mounting` | `din_rail`, `screw`, `plate`, `door_cutout` | `door_cutout` puts the device's holes and cut-out on the door; anything else on the plate |
| `hole_pattern` | `x,y,d; x,y,d` | Screw holes, mm from the top left of the unrotated device, diameter last |
| `cutout` | `rect WxH` or `round D`, optional `@x,y` | Cut-out; `@x,y` is its centre from the device top left, the device centre when left out |

Rotated devices turn their holes 90 degrees clockwise with the footprint.
Text that cannot be read is listed in `DrillTable::problems` with the device tag;
it is never silently dropped.

DIN rails get their own M5 clearance holes (5.5 mm): 10 mm in from each end and
at most 200 mm apart, along the rail (vertical rails run down). A rail shorter than 20 mm gets one hole at its middle. Ducts get none yet.

## The drawing

- Plate edge, margin, rails (35 mm, centred on the rail line), device outlines
  with tags, numbered holes (`1`, `2`, ...) and cut-outs (`C1`, ...).
- Overall width and height, and ordinate dimensions: the X of every feature along
  the top edge and the Y along the left edge. Labels closer than one text height
  to the previous one are left out; the table has every coordinate.
- Standard scale (1:1 to 1:100) picked so the plate fits an A3 sheet; text is
  2.5 mm on paper at that scale.
- A door sheet appears only when a device has door holes or a cut-out. The door has
  no model yet, so it is drawn as large as the plate (R2 adds the enclosure fields).

Coordinates in the table are millimetres from the plate's top left, x right, y
down. In the DXF the same corner is the origin and y points up, so a table point
(x, y) is at (x, -y) in the file. DXF layers: PLATE, MARGIN, RAILS, DEVICES,
HOLES, CUTOUTS, DIMENSIONS, TEXT, TABLE.

Table rows are grouped by diameter (one tool change per size), then top to
bottom, left to right. A long table continues on further PDF sheets.

## Not done

The plate drawing has a Cabinet export button and a command; there is no CLI command yet. Side-view drawings, duct cut lengths (R6) and machine formats are later.

## Workshop pack: 1:1 drilling template

Reports > Export > Export workshop pack (folder), or the Workshop pack button in Bench output, writes `<project> workshop` next to the project file. The first file is the drilling template of each mounting plate at true size on A4 or A3: tiles overlap by 10 mm and carry grey registration crosses, a cross hair at every hole and cut-out centre with its number and size, and a 100 mm scale-check ruler. Print at 100%. Hole positions are exact in the file; on paper they are only as good as the print scale and the taping (about 0.5 mm is typical, an estimate). The folder also holds the bench cut list, ferrules, labels and the assembly checklist.

## Hand drilling plan

Cabinet > Output > Export > Drill plan (hand, PDF), the command "Export hand drilling plan" (Ctrl K, File > Export drawings), and `1 Drill plan <enclosure>.pdf` in the Workshop pack. A4 portrait, one sheet per surface with work (plate first, then door and walls), for a workshop that marks and drills by hand.

Each sheet has: the datum (0 / 0 is the top left corner of the plate seen from the front, X right, Y down, in mm) with a small sketch; a "before you start" list with tick boxes (measure the real sheet, scribe the datum, check rails and devices against the layout); the tool list (one drill per diameter, a saw or punch per cut-out size) with counts; and the check-off table, grouped by tool, smallest drill first, then duct fixings, then cut-outs. Every row has a tick box, the number (the same hole numbers as the plate drawing and drill table, `C1` for cut-outs, `D1` for duct fixings), X and Y from the datum, the distance from the nearest edges in words (a check from the far side), the source (device tag, rail, opening) and the basis.

Basis says where a number comes from, and the sheet never states more than it knows:

| Basis | Meaning |
| --- | --- |
| maker data | the part's `hole_pattern` / `cutout` carries `drill_basis = maker`. Nothing sets this key yet: set the part property `drill_basis = maker` by hand until an importer writes it (follow-up in the parts importer thread) |
| typed in | `drill_basis = typed` (entered by hand) |
| part data | the part has the data, no recorded origin; shown as is, never promoted to maker data |
| planning rule | KoperCAD defaults: rail screw holes (5.5 mm, 10 mm from the ends, at most 200 mm apart), duct fixings (4.5 mm, 25 mm from the ends, 200 mm pitch), a 4 mm pilot hole for drills over 8 mm, a 10 mm corner hole for rectangular cut-outs. The maker's instructions win |
| project opening | a wiring opening entered in the project |

The basis is looked up by the device tag shown in the Source column, so two devices with the same tag share one basis. Where a device sits on the plate is the layout (a model), not a measurement; the sheet says so and asks for a check against the real rails and devices. Cut-out sizes for the tool are rounded up to the next 0.5 mm. Part text that could not be read is listed at the end, not dropped.

Code: `novo-layout::drill_plan` (plan and page marks), `novo-layout::model::drill_plan` (reads the project), `novo-pdf::drill_plan` (writer). Part key: `drill_basis` (`novo_layout::drill_plan::BASIS_KEY`).
