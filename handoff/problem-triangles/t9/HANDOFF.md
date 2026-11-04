# T9 handoff: parts and catalogue triangles

Where the user finds it: Parts window (Library > Parts): beside More, failed catalogue actions; File > Import > Import parts table > Rows: a triangle leads each row; Auto-assign parts dialog; part picker rows; Quality and Review tabs.

Add a triangle: build a `parts_flags::RowFlag::new(severity, title, lines)` and draw it with `parts_flags::inline(ui, &flag)` (or `labelled` with a short plain label). novo-parts `Severity::Note` maps to no flag (`from_parts_severity`). For a failed action call `self.parts_job_failed(job, sev, title, detail)`; call `parts_job_ok(job)` when the same action works.

Census: `crates/novo-app/tests/census/parts.txt`; lower a ceiling when you remove a site (the test also fails when the ceiling is higher than the count).
