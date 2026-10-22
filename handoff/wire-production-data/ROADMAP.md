# Wire production data: roadmap
Scope: Pro Panel parity data on the wire production output (gap analysis lane 3).
Done: bundle, sequence, track, direction, end processing with basis in the cut list (xlsx/PDF report, job CSV), bench cut list, circuit sheets, bench cut table in the app.
Next: machine-specific column maps for named wire machines; per-end direction from entry side once ducts carry an entry/exit.
Machine tools that read the job CSV by position: the new columns are appended after "Length basis", old positions are unchanged.
Open: direction is "A>B as listed" (not current direction); end processing codes beyond ferrule/strip need a part-level key. Duct names D1.. are by duct rail id (order added), shared by the report, bench sheets and the Cabinet cut list.
