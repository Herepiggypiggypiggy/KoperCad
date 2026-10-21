# Safety function sheets handoff
Engine: crates/novo-reports/src/safety.rs `safety_sheets` (one Section = one page per function; rows Required, Duty, Sensor/Logic/Actuator, Result, Assumed x3, Standards, Disclaimer). Input extends SafetyReportInput (risk, duty, dc_percent, ccf, drawn_at, sources).
Mapping: crates/novo-app/src/app/safety_report.rs (`report_input`, `export_safety_sheets`). Drawing locations come from Project::cross_references of each bound device (page.column text).
Honest labels printed on every sheet: ESTIMATE not certification; closed-form PFHd, not Table K.1; values as entered; IEC 60204-1 clause numbers unverified and not quoted.
Limits: PDF has locations as text, no clickable links yet.
