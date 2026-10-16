# EPLAN-PXF-READ handoff
Branch: claude/eplan-pxf-read-ocop20 restarted from main after #607 merged. Slice 1 is on main; follow-up PR fixes the quadratic tag scan.
Code: crates/novo-interop/src/eplan/pxf/{stream,text,model,read,tests}.rs, fixture crates/novo-interop/tests/fixtures/pxf/synthetic-small.epj.
Source knowledge: /mnt/project-files/interop/crane-configurator-findings.md (findings thread cse_01VQS5EbJ8bqSb6DPZ72Kuyw). The crane repo is private and customer-derived: never copy macros, symbol libraries, manual text, motor data or secrets; fixtures stay synthetic.
Field table is assumed from that document, never checked against a real file: say so in every report.
Reviewer R2 cse_01SEokxUZ1MKDwyQvLm4DPys, lead A cse_01FiLvRBKEcNNHbQEGhYyL1S. After each push message the reviewer the head SHA.
Resume: cargo test -p novo-interop --lib pxf; next is slice 2 (look at eplan/../aml/plan.rs for the preview/apply pattern).
