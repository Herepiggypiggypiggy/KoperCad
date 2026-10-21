# Variants and generated engineering

Owner: Variants thread (brief 6 of `docs/AMBITION-REVIEW.md`, PR #504). Status: engines and first UI shipped
in one PR; round trip with the spreadsheet engine and the 3 entry points below are in.

## Goal

A machine builder sells options. The same project must become "with or without a conveyor", "400 V or 480 V",
"one motor or three", and a 60-line motor list must become 60 starters without drawing them. Three parts:

1. **Parametric macros**: typed parameters, calculated values, optional parts, repeats.
2. **Project options and variants**: yes/no, list, number and text options; conditions on pages and devices;
   named variants; a configuration view that says exactly what an option adds (pages, devices, order numbers).
3. **Spec-sheet generator**: one row per macro placement in a sheet (Excel or CSV), generated as pages, written
   back from the project so Excel can edit it and the pages regenerate.

No change to `novo-core`: everything is stored in `properties` that the model already carries, so an older
KoperCAD opens the file and ignores it.

## Where the user finds it

| What | Where |
| --- | --- |
| Options, variants, conditions, "what does this add", lint | Ctrl+K `options` (window *Project options and variants*), project menu once #438 lands |
| Switch variant | Ctrl+K `variant <name>` |
| PDF of one variant | Ctrl+K `export variant <name>`, or the window's *Export PDF of the current choice* |
| Generate pages from a spec sheet | Ctrl+K `generate` (window with preview, problems by line, Add or Replace) |
| Spec sheet back out | Ctrl+K `export spec` (`<project> spec.xlsx`) |
| Parametric macro placement | `novo_workflow::parametric` (used by the generator; macro ghost/browser threads call it) |

## Options on the drawing (the machine of the current choice)

The working project always keeps every option's pages, devices and wires. Everything that describes *the machine*
reads it through `NovoApp::machine()` (`crates/novo-app/src/app/machine.rs`): the project with what the current
choice leaves out removed, and the choice named in the project name (`Press line [Cooling fan, Supply 480V]`), so
the title block and every report header say which machine a document is. The copy is made once per project revision
and only when an output asks. A project without options is read as it is.

| Output | Follows the choice |
| --- | --- |
| Reports workspace (BOM, device list, wire list, cut list, labels, ...) and its Excel/CSV/PDF export | yes, with a "Machine: ... Left out of the documents: ..." line on top |
| Generated pages on the canvas and in the PDF | yes |
| PDF of the project (File > Export PDF) and *Export PDF of a variant* | yes (the variant export names the variant in the file) |
| Wire cut list, wire production, bench pack (cut list, ferrules, labels), workshop pack, assembly checklist, plate drawings | yes |
| 3D view and the heat run started from it | yes (inactive devices, wires and ducts of an off option are not drawn) |
| Design checks appendix of the PDF | yes |
| Schematic canvas and Cabinet workspace | **no**: they edit the full working project, so nothing is hidden while you draw |
| The Cabinet's own heat run and sizing | no, same reason (the 3D heat run does follow) |
| Commissioning mode | no (it reads the working project) |

How it is switched and tied:

| What | Where |
| --- | --- |
| The choice, always visible | status bar chip `Options: ...` in every workspace (click opens Project options) |
| Switch | Ctrl+K `option conveyor on` (also `option supply 480V`, `option fan` flips a yes/no option); the message says what it added or took away |
| Tie the selection | Ctrl+K `tie fan` / `tie supply 480V`, or the buttons under *Tie the selection* in Project options: selected devices (through any of their symbols) and wires, else the page on screen, appear only with that answer |
| Untie | Ctrl+K `untie`, or *Always* in the window |
| Heat difference | Project options, *What the current choice includes*: sum of device losses in the cabinets, with the change against a compared variant |

A tie replaces the object's condition; a combined rule (`opt.a && opt.b`) is typed in the window's conditions box
and shown as typed. A device tied to an option takes all its symbols with it, on every page.

Limits, said plainly: cost is not shown because project parts carry no prices; the heat figure is the sum of the
loss figures of the parts (a model of what the devices put in, not a temperature; devices without a figure are
counted and named as 0 W); the Cabinet workspace and Commissioning mode do not apply the choice yet; ticking a
device on the schematic does not ghost it when its option is off.

## Expression language (`novo_workflow::expr`)

Numbers, yes/no, texts. `+ - * / % ^`, comparisons, `&& || !`, and `min max abs ceil floor round sqrt pow if
next_size one_of choose text len concat`. Names are `opt.<option>` and the macro's parameters. Unknown names
are errors (never a silent default). `next_size(x, 6, 10, 16, ...)` picks the smallest standard size that is at
least `x`, and says so when `x` is larger than every listed size. No loops, no assignment: a macro file from
someone else can only compute.

## Parametric macros (`novo_workflow::parametric`)

Stored in the block definition's `properties`, so they travel in `.kcmacro` files and project files:

- `kc.param.<name>`: JSON `ParamSpec` (label, kind number/choice/bool/text, min, max, unit, choices, `derive`,
  group). A parameter with `derive` is calculated, not asked.
- `kc.if` on a template member: the member is placed only when the expression is true. Wires, texts, symbols and
  junctions that would dangle go with it. The placed definition is a copy named `<macro>~<shape key>` (the same
  shape always names the same copy) carrying `kc.base`; the project keeps linked blocks, not frozen copies.
- `kc.repeat`: `{"count": "motors", "pitch": [x, y]}` places the macro `count` times, each with its own `index`.

Input is checked before anything changes: range, list, number format, calculated values (a failing calculation,
e.g. a power larger than the table, is reported, not guessed). Problems name the parameter.

`enrich_starter` gives the shipped *Motor starter DOL* a power range, a voltage list and a calculated rated
current `P / (sqrt(3) U 0.8)` (a planning value; 0.8 is efficiency times power factor), printed on the motor and
overload relay. The Macro library thread should move these specs into `starter.rs` and the `.kcmacro` files.

## Options and variants (`novo_workflow::options`)

- Project info properties: `kc.options` (definitions), `kc.variants` (saved choices), `kc.option.<id>` (current
  values). Option ids are lowercase letters, digits and `_`.
- Objects carry `kc.when` (an expression over `opt.*`). A page that is off takes its contents; a device takes
  its symbols and wires; terminals follow their strip, cabinet placements their device.
- `configuration` lists active/inactive pages and devices and the BOM of what stays (quantity per manufacturer and
  order number, assigned parts only; devices without a part are listed, not guessed). `diff` is the exact answer
  to "what does the conveyor add".
- `variant_project` returns a copy of the project with inactive objects removed, for PDF, reports and exports,
  without touching the working project (so switching back loses nothing).
- `lint` finds conditions that cannot be evaluated, options nothing uses and variants naming unknown options.
- A condition that fails to evaluate keeps the object and is reported, never silently dropped.

## Spec sheet (`novo_workflow::generate`)

Columns `Macro` (required), `Page`, `Structure`, `When`, then one column per macro parameter. A header that
matches nothing is a warning (a typo in `Powr` never silently becomes a default). All rows are checked first; if
any has a problem nothing is generated and every problem is listed with its line. One undo step. Layout is a
column flow (stack down, new column, new page) meant as a first draft. `Replace` removes pages (and their
devices) an earlier run generated, so a sheet edited in Excel regenerates cleanly. `spec_from_project` writes the
sheet back from the placed blocks (calculated values left out).

`novo-interop` `sheets::{read_grid, write_grid}` read and write the sheet as `.xlsx` (tags and `=A1` stay text).

## Validation

Unit and integration tests in `novo-workflow` (expressions, motor current/breaker sizing against hand
calculations, calculated and bad values, optional parts and dangling wires, repeat with fresh tags and one undo
step, options on/off diff with exact pages/devices/BOM lines, variant project, lint, generation with problems by
line, page flow, `When`, replace and round trip), `novo-interop` (grid round trip), and app tests driving the
real windows (variant switch and PDF page counts, a shipped macro generated from a sheet, spec export).

## Next

- Macro ghost/browser parameter form in the inspector (Macro threads call `parametric::instantiate`).
- Option column on the BOM and wire list (the title block and report headers name the choice since the machine view),
  `kopercad-cli generate`, a Cabinet/Commissioning switch to the choice, ghosting of off objects on the canvas.
- Cabinet layout carry-over for generated devices, and `kc.if` editing in the macro save dialog.
- Multi-language and customer forms belong to brief 11.
