# drawing-standard: reviewer notes

Teaches: open Drawing standard, choose NFPA 79, understand it is a project rule, switch the placed symbols, check the result.
Status `draft`, and it must stay draft: two things it teaches do not exist in the app yet. Builder and tests: `crates/novo-tests/tests/lessons/standard.rs`.

What is true today (verified in code)
- `project.standard` ("Drawing standard", no default key; Ctrl+K then "standard") opens the "Project settings" window (workbench.rs:550, project_settings.rs:55). Two radio buttons, "IEC 60617 (default)" and "NFPA 79 / ANSI Y32.2" (project_settings.rs:63-66).
- The choice is a project rule (`ProjectRules.standard`, stored in the file only when not IEC, one undo step) and shows as " · NFPA" in the window title (app.rs:843). It is not a view or user setting.
- The window itself says "placed symbols stay as drawn" (project_settings.rs:58-60): choosing NFPA changes no symbol.
- Engine for the switch exists: `novo_workflow::switch::plan(project, Pack::Nfpa).command()` (#447), one undo step, re-keys every symbol to its twin, leaves tags/pins/positions/wires/parts alone. finished.json is exactly that applied to project.json (asserted).
- All 174 IEC symbols have an NFPA twin with identical function, behaviour, connection ids and connection points (checked for this circuit in standard.rs). Only the potential feed is drawn the same in both standards. Company-library symbols are "foreign" and silently left alone.

GAPs (owner)
1. No "Switch placed symbols" control (WS-01; WS-25 handoff calls it half-done). Step `switch-symbols`.
2. The editor and PDF export load only the IEC library (Editor::default -> Catalog::builtin(); view.rs:257; pdf.rs:49), so an NFPA-keyed symbol is drawn as a placeholder box, and the place wheel never finds an NFPA twin (wheel.rs:356-363). Needs the canvas art/catalog to follow `rules.standard` (WS-01/WS-06/WS-03).
3. No list or marker for symbols left without a twin (`devices_without_counterpart`) in the UI (WS-01/WS-14).
4. Lesson-engine: no check kind for a project rule; no solve operation for the settings window.
Oddity: the plan back to IEC writes the rule as "iec", the radio button writes "" for IEC (same meaning, file differs by one line).
