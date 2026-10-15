# Assembly checklist (R8)

The workshop sheet for building one cabinet from the project. It answers the
question "what do I do next, and in which order" without anyone opening the
CAD program. It is the R8 item of `docs/PRO-PANEL-GAP-ANALYSIS.md`.

Machine-neutral on purpose: the output is a report (table, CSV, later PDF).
Files for a drilling, cutting or wire-processing machine are a separate,
later option once a machine is named.

## 1. What it is

`novo_reports::AssemblyChecklist` is an ordinary report (id `assembly`). It is
not a proposed default page; the user asks for it when a cabinet is ready to
build. It reads only the model and the existing wire list, so it stays live
through `LiveReports` and never recomputes wiring data itself.

For every enclosure with a mounting plate it makes five sections, keyed
`"{tag}:{step}"` and titled `"{tag}  N Name"`:

| Step | Key suffix | Content |
| --- | --- | --- |
| 1 Prepare | `prepare` | The plate (size, margin), then every rail and duct cut, grouped by kind and length with a quantity and the marks (R1, R2, D1) |
| 2 Mount | `mount` | Rails and ducts by position on the plate, then devices in reading order: part, rail and offset or "plate", x, y, turn |
| 3 Wire | `wire` | The wire list rows that have an end in this cabinet, with length and its basis (routed, free air, estimate) |
| 4 Label | `label` | One line per placed device: the tag text to print |
| 5 Check | `check` | Facts from the model, then the checks a person does, then Built by and Date |

Every section ends in a Done column holding `[ ]` (`ASSEMBLY_BOX`), so the
printed page can be ticked and the CSV opens as a work list.

## 2. Rules

- Mount order follows the plate, top to bottom and left to right, so the
  builder works across the plate and never reaches over finished work.
- Rails and ducts are marked R1.., D1.. in that same order; the Mount and
  Prepare steps use the same marks.
- Equal cuts are grouped: two 400 mm rails are one line with quantity 2.
- Wire lengths come from the wire list (#417), so the sheet and the wire list
  never disagree.
- Check facts: devices not placed, overlaps, devices too close, devices outside
  the plate, rail overflow, guessed part sizes, estimated wire lengths. A fact
  that holds reads "yes"; one that does not names what is wrong.
- With no plated enclosure the report is a single note saying so.

## 3. Not in the first PR

- **Drill table.** Waits on the plate drawing work (#397,
  `model::plate_documents` and `DrillTable`). Added as a step between Prepare
  and Mount once it merges.
- **PDF.** Waits on the stand-alone report PDF (#404, `ReportPdfOptions`). The
  sections already carry a Done column; the PDF adds `row_mm` for the sign-off
  rows.
- **Machine files.** Skipped until a machine is named.

## 4. Tests

`crates/novo-reports/tests/assembly.rs` runs against the Gate 1 motor starter:
step order and titles, cut grouping, mount order, wire filtering against the
wire list, labels, check text and sign lines, overlap, no plate, two cabinets,
determinism, live update and CSV.
