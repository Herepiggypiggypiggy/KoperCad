# Cabinet arrange tools: roadmap

Scope: buttons and engines that place and tidy devices on the plate, previewed and undoable,
honest about being rules and not a search, friendly to a hand workshop (rails and ducts cut by
hand, holes drilled by hand). Batch select and drag belongs to another thread.

## Done (PR 1)
- `novo-layout/src/arrange.rs`: Autodistance (close up, even gaps), Autoplace on rail,
  Auto rails, Auto ducts, Autogroup. Each returns a `Plan` (new `moves`, `title`, `notes`).
- Preview and accept reuse the Place all path (banner, Enter/Esc, one undo step named after the tool).
- Cabinet rail: Arrange entry with a card of the six commands. Also Ctrl K and the plate right-click menu.

## Done (PR 2: autogroup by wiring and heat)
- `novo-layout/src/regroup.rs`: Group by wiring, Group by heat, Group by wiring and heat across rows.
  Links come from the netlist (`cabinet_arrange::wiring_links`: nets of 2..=12 devices, 1/(k-1) per pair).
  Score = weighted Manhattan centre distance (estimate, not routed) + heat coupling (`heat::coupling`) +
  penalties for rows mixing power and signal and for zone-order inversions; start from the layout now and a
  built grouping; swap/move search, max 6000 layouts; never worse than now; dropped if it adds place-rule violations.

## Next, in order (item 2 and 5 below are partly done by PR 2)
1. Auto-reconfigure: one button that runs rails, onto rail, group, close up and ducts together
   on the selection, with one preview. Needs plan composition (new rails feeding later steps).
2. Autogroup across rails (put one kind on one row), using wiring connectivity from the
   schematic (shared nets), not only the letter code.
3. Auto ducts: vertical ducts at the sides, duct class from the rows (power or signal),
   duct sizing from the wire fill of `cabinet_routes`.
4. Autodistance along vertical rails in the UI wording; spread across rows.
5. Heat-aware versions: the tools must keep the heat zone rules of `heat.rs`.
6. Hand-workshop output: show the rail cut lengths and the new hole positions in the preview.
7. QA scenario for each tool (scenario block to be agreed with the QA thread).

## Open questions
- Should Auto rails use stock rail lengths (cut list) or exact lengths? Now: exact, whole 5 mm.
- Should Autoplace on rail rotate devices that do not fit upright? Now: never rotates.
