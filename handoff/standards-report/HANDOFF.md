# Standards compliance report: handoff

Branch `claude/project-thread-nb6zc7` (kopercad). Reviewer R2 (engine), R5 (report UI).

Where the user finds it: Check > Standards check (Ctrl+K, "Standards check…"): tick the standards, then Save report as PDF / Save as web page. Palette: "Export standards compliance report" (`export.standards_report`, PDF beside the project as `<project> standards.pdf`).

Code: `crates/novo-checks/src/standards/{document,provenance}.rs` (model, HTML, fix hints, planning-value register), `RuleResult::evidence` (via `Sink::pass_note`), `crates/novo-app/src/app/standards_report.rs` (PDF table mapping, export), `app/standards.rs` (window).

Honesty: the AWG ampacity table, PE and ground steps, colours, voltage limits and motor percentages are planning values (not read from the licensed text). They are listed in `provenance::BASIS`; a verdict that uses one is marked "planning value" in the document. IEC 60364-5-52 TableSet values feed cable calculations only (no rule uses them yet); the document lists that under Not covered. Cross-check this session: only against independent recollection of the usual published tables, no discrepancies seen; that is not verification.

Traps: PDF table cells are single-line, so long text is wrapped into continuation rows by `standards_report::wrap`. Date is left empty on purpose (deterministic output).
