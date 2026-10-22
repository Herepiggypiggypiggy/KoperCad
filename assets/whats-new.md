<!-- What's new: newest first. Each entry is `## id · Title`, then a
"Where:" line naming where the user finds it, then one or two sentences.
The id is a date and a letter; the top entry is the newest one a person has
seen once they open the list. Add an entry when a feature PR merges. -->

## 2026-10-22-a · The single-line diagram: supply data, lamps, overload relays
Where: Reports > Single-line diagram.
The diagram has an "Enter supply data" button that opens the Electrical inputs, and the example project now states an illustrative 400 V, 6 kA supply (replace it with yours) so it shows figures. Lamps and sounders on a mains feeder are drawn; a load on one line conductor is taken at the phase voltage (231 V from 400 V). An overload relay in a feeder is drawn as a box on the wire. Lamps on 24 V circuits stay off and are named in a note.

## 2026-10-21-f · Wires between two cabinets are drawn to each entry
Where: 3D workspace, click a wire that ends in another cabinet.
When both cabinets have an entry point, each cabinet draws its own part of the wire to its entry, and the Run card names the other cabinet. The run between the two entries is not drawn, and no length is invented for it. Each leg counts toward the duct fill of its own cabinet.

## 2026-10-21-e · Which wall the cabinet entry is in
Where: Cabinet workspace, Routes panel, "Gland in the" (and the 3D view).
State the wall the gland is in; the 3D view draws the way out from the entry through that edge of the plate. Not stated, the nearest edge is drawn dashed as a guess.

## 2026-10-21-d · Bend radius and heat checks on the Run card
Where: 3D workspace, click a wire; Settings rules "Bend radius", "Gap to hot devices", "Hot from".
The card says whether the wire or cable can take the duct corners of its path (a red cross marks a tight one) and which devices losing heat it passes close to. Both are models, and the card says what they do not know.

## 2026-10-21-c · Pick a duct to list its runs
Where: 3D workspace, click a duct.
The Wire runs panel shows how full the duct is and every run in it; click a run for its Run card, or "Select its runs" to read them together.

## 2026-10-21-a · Cable runs in 3D
Where: 3D workspace, click a wire (or Ctrl+K, "wire runs", then a row).
The Run card shows the path lit up along its ducts, how the length was found, how full each duct is and where the cabinet entry is; "Select all cores of the cable" lights a whole cable.

## 2026-10-16-d · Calculations view
Where: Ctrl+K, "Calculations".
Electrical inputs and calculation inputs for the project, with the results beside them.

## 2026-10-16-c · Cable editing
Where: the cable list on a cable page, or Ctrl+K, "cable".
Rename, delete and retype cables, edit cores and wire numbers, and accept proposals.

## 2026-10-16-b · Heat follows moved parts
Where: Cabinet workspace, the Heat panel.
Moving a part marks the heat and airflow result out of date and solves it again.

## 2026-10-16-a · Right-click menus
Where: right-click in the schematic, the Cabinet or the 3D view.
Each view offers the commands that fit what you clicked, with their keys.
