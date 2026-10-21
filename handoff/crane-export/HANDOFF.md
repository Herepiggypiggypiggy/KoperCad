# Crane export handoff

State: import built and tested; PR from branch `claude/project-thread-qso0sj`.

Where: `crates/novo-interop/src/aml/` (`read.rs` parses `Page`, `Symbol`, pins and nets; `draw.rs` places
symbols and wires; `plan.rs` previews and applies as one undo step). Test `crates/novo-interop/tests/aml_drawing.rs`
(hand-written file; opt-in real file through `KOPERCAD_CRANE_AML`, never committed).

User-facing: File > Import and export > Import AutomationML. No new dialog.

The writer lives in the private `eplan-crane-configurator` repo (`scripts/kopercad_export.py`, run after every
build by `projectcheck.run_and_report`). The format is in `docs/design/automationml-drawing.md`; keep both in step.

Decisions: AutomationML instead of a custom JSON (the project wants open standards); no texts are added on
import; boxes carry no label; nets are exact, routes are plain right-angle runs.
Trap: an earth symbol has no device, so the file lists it under its page.
