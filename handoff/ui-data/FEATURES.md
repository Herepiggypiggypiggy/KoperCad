# Feature versus where the user finds it

Rule: a data feature is done when a user can reach it without knowing a command id. Menu sections are filled by command-id prefix (`crates/novo-app/src/app/file_menu.rs`): `import.`, `export.`, `parts.`/`macro.`/`library.`, `revision.`, `project.`/`app.`. Register the command in `novo-workflow`, handle it in `run_command`, and it shows up in the project menu.

State as of 2026-10-15 19:00 UTC. "Engine" means code exists and tests pass but no way to reach it in the app.

| Feature | Engine | Where the user finds it today | Gap and next step | Owner thread |
| --- | --- | --- | --- | --- |
| Export PDF | novo-pdf | Project menu > Export > Export PDF, palette `pdf` | none | app |
| Bill of materials export | novo-reports | none (command listed, app does not run it) | Export > BOM through the spreadsheet dialog | this thread |
| Generated pages (reports) | novo-reports | Outline > page list, New > Pages | fine | pages |
| Parts browser | novo-parts | PR #429 (draft): Project menu > Libraries > Parts | menu entry appears with the command; Import is still "Open page" | Parts browser |
| Part Import (one click) | novo-parts | not yet | Import flow PR 2 of the Parts browser thread | Parts browser |
| Macro library | novo-parts macros (#405) | none | browser window + Place flow; design #396 | this thread |
| Spreadsheet export | novo-interop sheets (#414) | Export > Export tables to Excel, palette `export excel` (writes `<project> tables.xlsx` beside the project, never replaces one) | none until import preview exists | this thread |
| Spreadsheet import | novo-interop sheets (#414) | none | Import > Spreadsheet… with preview of the plan (changes, conflicts), apply as one undo step | import dialog thread |
| DXF export | novo-interop dxf | PR #385 (draft) | menu entry arrives with `export.dxf` | DXF/EPLAN interop |
| DXF as page background | novo-interop dxf | PR #385 (draft) | controls in the page properties | DXF/EPLAN interop |
| EPLAN parts import | novo-interop eplan | none | Import > EPLAN parts… sharing the dialog frame | this thread |
| Issue revision | novo-revisions | Project menu > Revisions > Issue revision | none | revisions |
| Revision table on frame | novo-pdf / canvas | on the page frame (#343, draft) | none | revisions |
| Change list between two states | novo-revisions `change_list` (#401) | Outline > Changes shows changes since last issue | Revisions > Compare… (design #400) | this thread |
| Project standard, preferences | app | Project menu > Settings | none | app |

## Unified dialogs

Import and export dialogs share one frame: title, one-line "what happens", options, preview (rows or counts), a primary button on the right, Cancel. Nothing is written until the primary button. Destructive or model-changing imports show the plan first and apply as one undo step.

## Order of work

1. Menu sections by prefix (this PR).
2. Spreadsheet export dialog (smallest, no model change).
3. Spreadsheet import preview dialog.
4. Macro browser and Place flow.
5. Compare revisions window.
6. EPLAN parts import, DXF page background.
