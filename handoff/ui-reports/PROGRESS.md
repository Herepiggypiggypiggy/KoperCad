# UI integration: reports and checks: progress

## Where does the user find it? (checked against main 074138a plus open PRs, 2026-10-15)

| Feature | Engine | Where in the app | Status |
| --- | --- | --- | --- |
| Table of contents, BOM, device list, wire list, terminal diagram, cable diagram, I/O list | novo-reports, main | Generated pages (outline); Reports hub (top bar, palette) with live table, filter, Excel, CSV | hub: PR 1 (draft) |
| Reports as one Excel workbook | novo-reports `xlsx_bytes`, main | Project menu > Export > All reports to Excel; hub button; command `export reports` | PR 1 |
| BOM export | main | command `export bom` (was "not in the app yet"); hub | PR 1 |
| PDF of every page | novo-pdf, main | Project menu > Export; command `export pdf` | existed, moved under Export |
| Cable schedule, plug connection list | #416 | Reports hub automatically (registered in standard_reports); plugs also a default page | auto after merge; check hub shows sections per plug |
| Wire list cut lengths, cut list | #417 | Reports hub automatically; cut list is not a default page, hub "Show page" absent until the page is added | auto after merge; "Add as page" button is a follow-up |
| Terminal diagram wires in/out | #422 | Terminals drawer and hub (terminals report) | auto after merge |
| Design checks list, click to jump, F8 | novo-checks, main | Issue pill, outline Issues mode, F8/Shift+F8 | exists |
| Waivers, new checks, calc library | #412, #413 | Issues mode (right-click to waive, Waived list) | #413 owns; polish planned (grouping, button to waive) |
| Design checks in PDF | #413 | Last pages of the project PDF | #413 |
| Commissioning report | #404 | none yet; needs novo-commissioning to map its records | blocked on that crate |
| Stand-alone report PDF | #404 `export_report_pdf` | none yet; planned hub button "PDF" | after #404 |
| Revision change list | #401 `change_list` | Changes outline shows per-page counts only | revision thread (Revision diff on the canvas) owns |
| Spreadsheet round trip | novo-interop | owned by "UI integration: parts, macros, sheets, DXF" | not mine |

## Log
- 2026-10-15 18:39 UTC thread started. Surveyed; messaged workspaces and design checks threads.
- 2026-10-15 19:10 UTC Reports hub, Export submenu, commands and Dutch words written; app and workflow tests green.
