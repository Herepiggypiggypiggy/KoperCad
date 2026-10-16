# ui-macro-authoring roadmap

Scope: UI for authoring macro parameters and for exporting macro packs.

Done
- Macro editor window (`macro.edit`): add, change and remove parameters (name, type, default, description) of a library macro.
- `macro.export_pack` and the browser's "Export pack..." button: the engine's `.kcmacropack`.
- Engine: `novo_parts::{ParamDraft, ParamKind, validate_parameters, MacroLibrary::set_parameters, MacroFile::unknown_references}`.

Next
- Edit the parameters of a definition already placed in a project (needs a novo-core command, WS-02: replace a block definition's parameter list with undo).
- Pick which macros go into a pack (today: the whole library, or what the browser search shows).
- Rename a parameter and rewrite its `{name}` references in the members.
