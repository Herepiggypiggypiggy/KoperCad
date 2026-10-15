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

No menu, CLI command or Cabinet-view button yet; the engines and writers are
callable. Side-view drawings, duct cut lengths (R6) and machine formats are later.
