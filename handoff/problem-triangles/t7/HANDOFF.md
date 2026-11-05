# T7 handoff: schematic edit refusals

State: PR open (see PROGRESS.md for what is in it). Mechanism in `crates/novo-app/src/app/flags_edit.rs`; tests `flags_edit_tests.rs`, `status_bar.rs::refusal_tests`; scenarios `qa/scenarios/9470*`, `9471*`.

Use it from any app code: `self.refuse(Level::Warning|Error, "Head: reason")` (selection, else tool), `self.refuse_titled(level, "Title", reason)`, `self.refuse_tool(level, text)` from a dialog. "Head: reason" with a head of at most 28 characters becomes title and detail.

Lanes T8..T14 can move their toast sites the same way; `numbering.rs` and `link.rs` still raise toasts (T8). Add a ceiling test per area like `problem_census_schematic_edits`.

Traps: the transient flag expires on pointer release or key press after two frames; a refusal raised while a text field has focus survives the Enter that raised it. The popover keeps the flag while open.
