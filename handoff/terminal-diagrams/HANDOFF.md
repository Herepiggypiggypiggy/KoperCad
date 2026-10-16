# Terminal diagrams: handoff

Split (Supervisor/coordinator, final 2026-10-15): this thread owns the terminal strip rows and the drawn strip diagram (bridges/jumpers, end plates) with its PDF/CSV; "Cable and terminal strip plans" (session_01RZjDVCS7GLdP7YT6jnyYoF, PR #416) owns the cable schedule, the tabular strip plan and the plug list (`plugs`, `export plugs`). Do not touch plugs.

Data: novo-terminals `screen` (ScreenRow: wires_internal, wires_external, bridged_with). Report: novo-reports diagrams.rs `TerminalDiagram` columns incl. Wire in, Bridged to, Wire out, Location (Excel/CSV keep the table).
Drawn diagram: novo-reports strip_diagram.rs (`strip_diagram`, `strip_sheet`): blocks per terminal, jumper bars, end plates, as a PageLayout; `LiveReports::layout_sheet` uses it for the `terminals` generated page, so canvas and PDF print it.

How to reach it: add the generated page "Terminal diagram" (default generated pages, or Page > generated) ; each strip prints as a drawn sheet. Terminals drawer shows the same rows.
Done: Terminals drawer columns (Terminals drawer in the dock), `kopercad-cli export terminals` (CSV or .xlsx).
Check: cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
