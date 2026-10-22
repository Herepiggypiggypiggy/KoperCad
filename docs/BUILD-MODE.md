# Build mode

Guided assembly and wiring of a cabinet at the plate: the local answer to a
server-based shop-floor tablet app (Pro Panel gap analysis, C15, lane 2).
No server, no account; it runs in the same program on a Windows tablet or a
laptop at the cabinet.

**Where to find it:** Reports > Build mode, or Ctrl+K and type "build mode".
Esc closes it.

## What it does

For every cabinet with a mounting plate it shows one step at a time in large
type, in the order the workshop works:

1. **Prepare**: the plate and the layout problems that would make later steps wrong.
2. **Drill**: holes one drill diameter at a time, then cut-outs, with positions.
3. **Rails and ducts**: cut and fit.
4. **Devices**: clip-on devices after their rail, then devices on the bare plate.
5. **Wires**: power, then control, then earth, each after every device is fitted.
6. **Labels**, then **Check** (what a person confirms by eye).

Each step has a Done button (Space or Enter also tick it), Problem (note goes to
the punch list in Commissioning, with a link to the device or wire), Back, and a
traffic light in the list. "Also ready now" lists steps that can be done at the
same time, so two people can work on different devices. A step that waits for
others says which. The progress bar counts done, problems and changed steps.

A device step also has **Show on the plate** and **Show in 3D**: Build mode steps
aside behind a "Back to Build mode" bar at the bottom left of the window, the device is
selected in the Cabinet canvas or the 3D view, and the bar brings you back to the
same step. While it is stepped aside Space and Escape do not touch the build.

## Honest limits

- Wires are cut and holes drilled by hand. A step says what to cut or drill and
  where; it does not drive a machine.
- Guessed part sizes, estimated wire lengths and a missing cross-section are
  said on the step. The power/control/earth grouping is the usual practice
  (green-yellow is earth, 2.5 mm² and up is power), not a reading of the schematic.
- A ticked step remembers a fingerprint of its text. If the design changes so the
  step reads differently (a device moved, a wire got longer) it shows as
  "Changed" and counts as open again.
- Ticks are never deleted by redrawing: if a step disappears (a deleted device) and comes back (undo), its tick is still there. A tick that matches no step is simply not shown, and a field file merge skips it.
- Merging has no tombstones, like the commissioning checks: a tick taken back with Undo on one machine comes back if the other machine still has it and its tick is newer than the undo is old. Desk and tablet should run the same build; an older program drops ticks it does not know.
- Undoing a Problem leaves its punch item (close it in Commissioning); raising the problem again on the same step reuses the open item.
- Space and Enter tick a step even when it still waits for others (the banner is advice, because the workshop sometimes fits things out of order). The step is ticked as it stands.
- A wire that is part of two connections gets one step each. A wire between two
  cabinets has a step in each cabinet.

## Tablet and field file

The ticks live in the project file beside the commissioning results and travel in
the same `.kcfield` file (Export ticks... / Import ticks...). Carry the project to
the tablet, build, export the file, import it at the desk: per step the newer
decision wins, a tick for another project or a deleted part is skipped. Large
buttons switch on by themselves in a window narrower than 1200 points, or with the
Large box.

## Where the code is

- Engine: `crates/novo-commissioning/src/build.rs` (`BuildPlan`, `StepKey`, ticks,
  `next`, `ready`, `mark_problem`), merge in `field.rs`, tests in `tests/build.rs`.
- Screen: `crates/novo-app/src/app/build_mode.rs`, tests `build_mode_tests.rs`.
- QA: `qa/scenarios/71-build-mode.qa` (1366x768 and 1920x1080).
