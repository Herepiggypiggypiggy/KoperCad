# Crane verify (lane 4) roadmap
Scope: an honest harness that compares an imported EPLAN project with EPLAN itself, a ranked list of mismatch classes, and fixes for the causes no sibling lane owns.
Done: net/pin classifier (`crates/novo-tests/tests/eplan_verify.rs`), vector compare against EPLAN's PDF with overlay PNGs (`tools/eplan_verify/pdf_compare.py`).
Next: re-run after lane 1 (pin offsets on the file's ports) and lane 3 (routes/junctions) merge; then take `no_main_function`, `wire_overlap`, and whatever stays in "pin there, no wire".
Open: EPLAN's own `Connection.eod` would be net ground truth (proprietary binary, not decoded; coordinator said skip for now).
