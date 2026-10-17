# SAFE-2 handoff
Model, rating and check: crates/novo-checks/src/safety.rs (kinds SafetyLevelNotMet error, SafetyBinding warning). Stored as JSON in project info `safety.functions`.
Window section: crates/novo-app/src/app/safety_functions.rs, shown in Check > Safety functions above the free calculator. Edits commit as one undo step once nothing is focused or dragged.
Binding: tag text -> exact device tag; reliability from the part or from device/part properties `b10d_cycles`, `mttfd_years`.
Honest limits: a calculation aid; PFHd of categories 2-4 is a closed-form model (see novo-safety docs).
Next: report page (novo-reports), see ROADMAP.
Report: crates/novo-reports/src/safety.rs (plain input), mapping app/safety_report.rs, button in the Safety window. Table K.1 could not be sourced reliably (Reer page is an image; Omron PDF text lost the grid); do not fill it from memory.
