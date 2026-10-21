# Machine options on the drawing: roadmap

Scope: options as a switch on the drawing. Engine (`novo_workflow::options`) and the Project options window pre-existed
(Variants thread, docs/design/variants.md). This thread adds the "on the drawing" layer.

## Done (PR from branch claude/project-thread-e6bexd)
- `NovoApp::machine()`: the project as the current choice; reports, generated pages, cut list, bench, workshop,
  assembly, plate drawings, PDF, checks appendix and 3D read it.
- Status-bar chip `Options: ...` in every workspace; Reports header "Machine: ... Left out of the documents: ...".
- Ctrl+K `option <name> [value]`, `tie <option> [value]`, `untie`; tie buttons and cabinet heat sum in Project options.
- Engine helpers: `condition_for`, `tie_to`, `describe_condition`, `summary`, `machine_project`.
- QA fixture `qa/fixtures/machine-options.novo` + scenario `qa/scenarios/43-machine-options.qa`.

## Next
- Cabinet workspace and Commissioning mode apply the choice (today they edit/read the full working project).
- Ghost objects of an off option on the schematic canvas instead of showing them normally.
- Option column on BOM and wire list; cost difference once parts carry prices.
- Cable runs and `kopercad-cli` variant export.
