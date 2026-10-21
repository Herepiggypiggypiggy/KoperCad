# Single-line diagram: roadmap

Scope: a generated power distribution single-line diagram, derived from the project's supply, protective devices, cables and loads. Shows cable size and length, protection ratings, load currents and voltage drop. Viewable in the app, exportable as a page and PDF.

## Done (first PR)
- Engine `novo-checks::single_line`: topology from nets (pass_through poles, converter outputs as new sources), per-run cable (size, length, Iz), current (stated, calculated from kW, or sum of branches), voltage drop per run and from the supply, verdict (Ib > Iz, In > Iz, drop limit).
- Report `novo-reports::single_line` (`singleline`): drawing sheets (wide branches split with the ancestor chain repeated) and a schedule table.
- App: Reports > Single-line diagram (Diagram and Schedule tabs, sheet stepper, zoom, Fit). Ctrl+K "single line". Reports > Add report as drawing page. PDF, Excel, CSV through Export.
- Fixture `novo-tests::distribution_board`, QA scenario 58, `qa/fixtures/distribution-board.novo`.

## Next
1. CLI export (`kopercad-cli export single-line`).
2. Single-phase circuits whose only load is not a motor, heater, drive, transformer or power supply are not drawn yet.
3. Diversity factors: DONE in the follow-up PR (device property `diversity_factor` on the sum below a device, project property `supply.diversity_factor` at the supply, Electrical inputs > Distribution; shown in the box as ×f, in the schedule basis and the legend; out-of-range values are ignored and said). Still open: factors by load type (IEC 60439 table) and a per-group demand calculation.
4. Short-circuit current along the run: DONE in the follow-up PR (Ik at each box from the supply through the cables; ceiling where data is missing; not carried past a converter; Icu check). Still open: motor contribution, busbar impedance, minimum Ik for disconnection.
5. Verify ampacity tables and IEC 60204-1 clause numbers against a licensed copy (Tadjiro).

## Open questions
- Should the diagram sit in the project PDF by default? Today it is added with "Add report as drawing page".
