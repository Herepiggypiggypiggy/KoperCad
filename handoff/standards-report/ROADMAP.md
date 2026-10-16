# Standards compliance report: roadmap

Scope: a document a customer or inspector can open and print: per requirement a verdict (pass, fail, not checkable), the evidence in the project, where to fix, and what the verdict rests on. IEC 60204-1 first, NFPA 79 and UL 508A selectable. Never claims certification.

Done (this PR): evidence on passes, planning-value register, document model and HTML, PDF via the report pipeline, set selector and export buttons in Standards check, command `export.standards_report`.

Next:
- STANDARDS-VERIFY: the owner reads the licensed clauses, records confirmed values as project overrides (`standards.<set>.<name>`), then the matching `provenance::BASIS` entries can move to a "confirmed" status.
- IEC 60204-1 conductor ampacity rule using the IEC 60364-5-52 TableSet (needs an installation-method property on wires/cables).
- Evidence for the remaining rules (colour rules count wires only).
- File menu entry (Export submenu) for the command.
