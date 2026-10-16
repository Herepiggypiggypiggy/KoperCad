# Handoff: fault study from the schematic editor

## State
Command `sim.add_fault` ("Add fault to selection", Simulate menu, palette, right-click "Add fault..." on a wire, device or terminal). It opens a small window (where, what, resistance, time) and stores a `ProjectFault` in the `faults` list of `<project>.faults.json`. No novo-core or novo-storage change: the sidecar the Fault scenarios window already reads carries the list. The window shows the faults, each run as a scenario (`Study::prepared`), and shows the Example study only when the project has no study file.

## Traps
- The study network is still its own input, not read from the drawing. A fault whose target is not a node of the study hangs on the busbar through a stub impedance with no protection, and the window says so.
- Needs a saved project (the sidecar sits next to the file).
- The terminal selection menu is tall; the lesson check clicks rows by position, so Add fault shares a line with Save as macro there.
- Added faults are not in the undo stack; Remove in the window deletes one.

## Resume
```
cargo test -p novo-sim --test project_faults
cargo test -p novo-app faults
```
