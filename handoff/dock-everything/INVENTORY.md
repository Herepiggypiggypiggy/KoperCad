# Every popup, tag and window in KoperCAD, and how it docks

Written 2026-11-02 for Tadjiro's requests 1, 3 and 7 (dock everything, fold everything, right side by default).
Source: a sweep of `crates/novo-app/src` for `panel::dialog`, `panel::modal(_fill)`, `Panel::new`, `Float::new` and `egui::Area`.

## How docking works now (one mechanism for all of them)

- `panel::dialog(..)` returns a `Dialog` (was a bare egui window). All 53 command windows go through it, so all of them dock without per-window code.
- Narrow command windows (default width 640 or less) open **docked at the right edge** as a pane. Wide ones (print preview, report designer, learning window ...) open floating but have the same title-bar buttons, and can be dropped on the left, right or bottom snap zone.
- Every pane has a **fold arrow** (header only) and buttons to float it or send it to the left, right or bottom strip. Floated by hand = stays floated until View > Reset panel layout.
- Forms and settings windows (`panel::modal(_fill)`) dock the same way (and stop blocking while docked). Windows that ask a question you must answer first stay modal.
- Cards (`Panel`) were already dockable when they had a title bar; the missing ones got `.dockable()`.

## A. Command windows (`panel::dialog`), 53, all dockable

Open docked right: Ask Claude, Autopilot, Bench output, Rails and ducts, Place rules, Cut list: rails and ducts, Compare revisions, Gamepad map, Calibrate gamepad, Renumber devices, Drive run (stays floating: draws above the live view), Electrical inputs, Fault scenarios, Add fault, Hoist run, Languages, Place macro, Macros, Edit macro parameters, Motor at variable frequency, Pages and devices on this net, Set wire number, Wire numbering, Export PDF, PLC overview, PLC program, Properties of the selection, Simulate motor start, Size the enclosure, Enclosures, Standards check, Project options and variants, Generate pages from a spec sheet, Winch scenarios, Safety functions, Parts, add-on panels, controller panels, terminal edit windows (rename, renumber ...), Cooler layout, Cooling what-if, Cooling advice, Photo (commissioning), Net trace.

Wide, open floating, dockable by button or by dropping on an edge: Calculations (760), Report designer (1040), EMC check (680 to 700), Hoist in 3D (760), Icon gallery (880), Learning window (980), Print preview and plot sets (1080), Protection (920).

## B. Forms and settings (`panel::modal`, `modal_fill`), dockable

Preferences, Project settings, Project rules, Keyboard shortcuts, What's new, Describe devices from parts, Model labels, Export centre, Parts import, Parts table import, Assign parts to all devices, Import I/O list, Import PLC tags, Add PLC card, Renumber I/O, New pages, New cabinet, Properties (outline), Save as macro, Sheet import, Controller profile from an I/O table, AML import, EPLAN export, EPLAN import, EPLAN parts import and reading, EPLAN export guide, New crane, New plant.

## C. Questions that stay modal (no dock buttons, by design)

Save changes?, Restore unsaved work?, Delete/confirm in the outline, Restore revision, Issue revision, Handover ask, Name the terminals like the pins, Use your company pin names?, Add-on install prompt, About KoperCAD, About the numbers, crash report.

## D. Cards on the canvas (`Panel`)

Dockable (already, V1c/V1d): Inspector (schematic and cabinet), Wire panel, Review notes, Live card, Controller, Changes, EMC nets, 3D: Heat, Vents and fans, Wire runs, Statistics, EMC panel, EMC legend.
Made dockable in this change: Defects and walk-throughs, Heat legend (temperature and change B-A), 3D Layers card, Properties of the selection card.
No title bar, so nothing to dock (small tool strips and chips): simulation chip, 3D toolbar, lesson card, walkthrough, 3D tool strips (move, nudge, align, measure, place, wire-it, build order, hints), tool dock, rail flyouts.

## E. Transient things that are not windows (never docked)

Toasts, hover tips, defect banner, refusal card, command field, right-click menus (plate menu, place wheel), HUD, drag-gap labels, empty-sheet hints.

## Not done here (other requests)

Request 2 (move explanation text into a ? popup) and request 8 (remove the 3D Layers card, fold it into the left rail) are separate lanes. The Layers card is dockable meanwhile.
