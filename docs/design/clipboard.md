# Copy, cut and paste

Where: Edit menu (Copy, Cut, Paste, Paste with the original tags where free,
Paste in place), Ctrl+C, Ctrl+X, Ctrl+V, Ctrl+Shift+V, Ctrl+Alt+V, Ctrl+K.

Engine: `novo_workflow::clipboard`. A copy is a snapshot (`Clip`) of the
selection: symbols, junctions, the wires between them, their texts, and the
devices, terminals, parts and potentials they refer to. It goes on the system
clipboard as JSON text (`format: kopercad-clipboard`, `version: 1`), so it
pastes on another page, in another project and in another running KoperCAD.
Ctrl+D (smart duplicate) is the same code, capture and paste in one step.

Paste: the drawing follows the pointer as a ghost on the grid; a click drops
it, selected, as one undo step. Esc cancels. Paste in place drops it at the
copied position. Pins that land on wires or pins join, as after a move.

Tags: Ctrl+V gives each copied device the next free number of its letter code
in the structure of the page it lands on. Ctrl+Shift+V (and a paste after a
cut, which moves the circuit) keeps the original tag when no device or cable
has it. No paste makes a duplicate tag. A coil and its contacts copied
together stay one device; a lone contact stays a contact of its device in the
same project, and becomes a device of its own elsewhere (the toast says so).

Wire numbers are not pasted (numbering assigns them); colours and
cross-sections are. Terminals land on the strip with the same tag, created
when missing. Potentials match by name, parts by manufacturer and order number.

Known limits: the menu pastes the last copy made in this program (Ctrl+V reads
the system clipboard, so it also pastes from another running instance);
rotating or mirroring the ghost before the drop is not done.
