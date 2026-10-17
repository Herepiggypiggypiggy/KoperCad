# Design checks panel: roadmap

Owner of the issue interface in novo-checks (ProblemKind, Problem), waivers, the Issues list and the PDF report pages.

## Done (PR #413, draft)
- Kinds: UnplacedDevice, WireWithoutDuct, OverfullDuct (physical.rs); VoltageDrop and ShortCircuit are label-only kinds for the calc thread.
- Waivers: `waive`, `check_open`, `waived_problems`, `report_lines` (waiver.rs); Issues list right-click, Waived list with Restore.
- PDF: `Appendix` option in novo-pdf; app prints design checks pages when there is anything to report.

- Stacked PR on #413: layout checks from novo-layout's report (DeviceCollision, DeviceOutsidePlate, DevicesTooClose, RailOverflow); they appear in the Issues list and the PDF pages like any other kind.

- Merged: layout problems (overlap, off the plate, too close, full rail) and the report perf fixes.

## Next
1. Calc thread adapter (voltage drop, breaking capacity, disconnection, thermal withstand, motor starter, protective conductor) on top of #413 + #412.
2. Routing thread: UnsizedDuct and GuessedWire infos in physical.rs.
3. CLI listing of problems with waivers; Python access.
4. Hands-on test of the Issues list on Windows.
