# Workflow: a motor starter

A direct-on-line starter is the classic first circuit: a breaker, a contactor, an overload relay and a motor on the main page, and a start/stop circuit on the control page. This page follows it from empty page to parts list.

![The motor starter, main page](images/example-schematic.png)

## Draw the main circuit

1. File > New project, then add a page (File > Pages > New page).
2. **Q**, type `breaker`, Enter, click to place. Repeat for the contactor (`contactor`), overload relay (`overload`) and motor (`motor`).
3. **W** from pin to pin. A proposal appears where two pins face each other; **Enter** accepts.
4. The contactor coil goes on the control page. KoperCAD links coil and contacts: each contact shows the coil's page and column, and the coil lists its contacts. These are the cross references; click one to jump, **Alt+Left** comes back.

## Wire numbers

Wire and potential numbers follow a scheme you choose (Edit > Wire numbers, or **Ctrl+K** and type `numbering`).

- Numbers are filled in stable order: adding a wire does not renumber the others.
- A number you type is **locked** and kept.
- Conflicts (two potentials with one number) are listed as problems with a jump to the place.
- Numbers pass through devices that only connect, such as an overload relay's main contacts, as your scheme says.

Renumber always shows a preview and is one undo step.

## Parts

Assign a part to a device from the Parts panel. The catalogue holds only what you import: maker data and tables via File > Import parts. Suggestions are limited to parts that fit the device. A device with no part still appears on the parts list, with a blank part number.

Weights and prices appear only when the part data has them.

## Terminals and cables

When a wire leaves the cabinet it ends on a terminal. Terminals with the same name join into one strip; the Terminals panel (**Ctrl+3**) shows each strip with its terminals. Wires that run between locations are proposed as **cables**: the Cables panel (**Ctrl+4**) lists the cores and shield, and the cores keep the cable's wire numbers.

## Try it live

Press **Shift+L** for the live view. Hold the Start button: the contactor pulls in, the seal-in contact holds, the motor lamp lights. Use it to see why a circuit does not behave (the lessons *coil won't pull in* and *motor won't stop* use it). The motor start curve is an estimate; see [Model or exact?](numbers.md#motor-start).

## Check the design

The **Issues** pill in the status bar lists problems with severity, each with a jump to the place. Fix or waive them; a waiver keeps its reason. The Check menu holds Problems, the Standards check, Calculations and Safety functions; read [what the standards check is and is not](numbers.md#standards-checks).

## Reports

Reports menu: Bill of materials, Wire list, Cable diagram, Terminal diagram, I/O list and more, as live pages that stay in step with the drawing. Export to PDF or Excel. See [the workshop pack](workshop.md) for output you can carry to the bench.
