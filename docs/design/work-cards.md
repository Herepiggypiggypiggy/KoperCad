# Work cards (workshop pack from the Build order)

The Build order knows the sequence of a cabinet: rails, ducts, devices, wires, labels
(`novo_layout::assembly::steps`, `docs/design/assembly-checklist.md`). The work cards turn that
sequence into paper for a workshop that cuts wires and drills holes by hand: one half page per step,
printable on an ordinary office printer, ticked with a pencil.

## What is in the PDF

`<project> work cards.pdf` (also `1 Work cards.pdf` in the workshop folder):

1. **Cover**: how to print (100 %, no fit-to-page, check the 100 mm ruler), how exact it is, and the
   **tool list** for the whole cabinet: one drill line per diameter with the hole count, the cut-out
   tool per cut-out, the rail and duct cuts grouped by length with their marks, the wire tools.
2. **Tick-off sheet**: every step of the build order with a box (38 rows per page).
3. **Cards**, two per A4 page (cut or fold at the dashed line). Each card says
   `STEP n of N`, the phase and mark, what to do, the tools, and carries tick boxes ("Cut", "Holes
   drilled", "Fitted", "Checked") and a 100 mm ruler.
   - **Rail card**: the rail strip with its holes and the devices clipped on it (a box as wide as the device along the rail at its distance from the rail start, with its designation when the box is wide enough, and the distances in a note) (numbers match the tiled drilling template), the holes
     as distances from the *nearest* plate edges ("65 mm from the left edge, 225 mm from the bottom
     edge"), where the rail starts.
   - **Duct card**: cut length and width, top left corner from the nearest edges.
   - **Device card**: clip position along the rail, or the footprint template with the screw holes and
     any cut-out, the same distances from the edges, and the wires that end at this device.
   - **Wire cards**: per circuit group (Power, Control, PE), nine wires per card with from, to, size and
     colour, the cut length and a Cut and a Run box. `~` marks an estimated length.
   - **Label cards**: the label of each device in mounting order.

## Rules

- The step numbers are the print pack's build order, step for step. A test compares the two.
- A template is drawn at 1:1 where it fits its area, else at the next of 1:2, 1:2.5, 1:5, 1:10, 1:20,
  1:50, and then the card says "NOT true size: measure from the list, not the drawing". A test checks a
  45 x 90 mm footprint is a 45 x 90 mm rectangle in the PDF and a 400 mm rail is not drawn 400 mm long.
- Hole positions are the drill table's: the card's local position plus the device's footprint origin
  equals the table row (test), and the hole number is the tiled template's number.
- Wires come from the assembly checklist's wire rows (an end in this cabinet) with the cut list's
  length (end allowances of the wire rules), grouped as the build order groups them.
- The group of a wire is the model of the usual practice in `Circuit::of`, not a reading of the schematic.

## Where the user finds it

- Reports > Export > **Export work cards (PDF)**, and the command field (`export work cards`).
- 3D > Layers > Build order: the strip has a **Work cards (PDF)** button.
- 3D Build order strip, on a part step: **Print this step** writes `<project> work card <mark>.pdf`, that
  step's card alone on one A4 page.
- Reports > Bench output: **Work cards (PDF)** beside Bench packet.
- Reports > Bench output > Workshop pack (folder) adds `1 Work cards.pdf` and lists it on the bench
  packet cover.

## Honest limits

- Positions come from the cabinet layout, not from a measured plate. Every card says so.
- True size depends on the print dialog; the ruler on every card is the check, and the print-scale check
  page repairs a stretched print.
- The tool list is derived from the hole diameters and cut-outs and is a suggestion, not a standard.
- Door and wall features are on a card through the device that carries them, and the Heat panel's grilles
  and fans have a "CUT-OUT" card each at the end of the cabinet's cards (not build order steps: a wall
  is cut before the plate goes in). They show the opening as the Heat panel and solver cut it (moved and trimmed to lie inside its wall), not as typed. Their centre is measured from the interior's edge, not the outer
  door edge, and a top, bottom or side wall has no stored position across its depth, which the card
  says. Wiring openings (`opening.*`) and plain door cut-outs without a device are still only on the
  cut-out cards and the tiled template.
- Duct fixing points are the duct maker's; the model has none.
- Not built: a drill-size
  choice per part (the tool is the part data's hole diameter), a wire "strip length" per end.
