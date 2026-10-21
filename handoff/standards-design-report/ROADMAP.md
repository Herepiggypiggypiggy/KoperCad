# Standards compliance report, design verification: roadmap

Scope: the standards report (clause rules, `handoff/standards-report`) lists only the clause rules of the selected standards. This lane adds the rest of what an inspector asks: which design checks ran (wire sizing, protection, colours and marking, safety functions, heat, drawing consistency, terminals/cables/PLC, cabinet layout, parts), how each came out (pass, fail, partly checkable, not checkable, not applicable, waived), what each assumed, where its data comes from and whether it is exact, a planning value or a model. Never claims certification.

Done:
- `novo-checks/src/standards/design.rs`: family table (every `ProblemKind` belongs to exactly one family, tested), per-family verdict from the open and waived problems, "not checkable" from data gaps (wires with no protective device, no installation method, no supply short-circuit current, no safety functions, no Heat result).
- Document: "Read this first" limits box, Design checks summary and detail, in HTML, PDF and Excel. Works with no standard selected.
- Heat: the app passes the held Heat result; an out-of-date or missing result is "not checkable", never fine.

Next:
- Selectivity and SCCR rows with their own evidence (today they sit under Protection through problems only).
- Verify the planning tables against licensed text (STANDARDS-VERIFY, needs the owner).
- Show the design verdicts in the Standards check window itself, not only in the saved report.
