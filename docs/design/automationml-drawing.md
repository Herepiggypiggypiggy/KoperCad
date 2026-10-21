# AutomationML drawing profile (WS-24)

Status: import built (`novo_interop::aml`, File > Import and export > Import AutomationML).
Export of drawn pages from KoperCAD is not built.

## 1. Why

KoperCAD should read and write open standards, not a format of its own. AutomationML (IEC 62714, CAEX 2.15)
is an open IEC standard that engineering tools already exchange, so it is the carrier for a project that
comes from another program, including the crane configurator (private, outside this repository). The plain
AutomationML export and import (`docs/design/interop-migration.md` §3.2) carry the plant: structure, devices,
terminals, cables and nets. This profile adds what is needed to get **drawn pages with connected wires**
without rebuilding anything from geometry: where each symbol sits, its pins, and which pins are one net.

Any program can write it. Nothing in it names EPLAN, the configurator or KoperCAD internals; the roles live
in the `KoperCADRoleClassLib` that is written into the file, with the standard base roles as parents, so a
reader that does not know the profile still sees the structure.

## 2. Layout

Under the project element (role `Project`):

| Element | Role | Attributes |
| --- | --- | --- |
| `Pages` group | `PageGroup` | |
| a page, named by its label (`=A1+CAB1/01`) | `Page` | `Title`, `Function`, `Location`, `Number` |
| a symbol with no device (an earth symbol), child of its page | `Symbol` | as below |
| a plant section or location (`=A1`, `+CAB1`) | `PlantSection`, `Location` | `Tag` |
| a device, named by its designation (`=A1+CAB1-K1`) | `Device` | `Tag`, `Description`, `Manufacturer`, `OrderNumber`; optional `Width`, `Height`, `Depth` (also `Breite`/`Hoehe`/`Tiefe`, `Breedte`/`Hoogte`/`Diepte`; the part's size, `Unit` `mm` when absent, also `cm`, `m`, `in`) |
| a terminal strip (`+CAB1-X1`) | `TerminalStrip` | `Tag`; one `Terminal` interface per terminal |
| a symbol of a device or strip, child of it | `Symbol` | `Page` (a page name), `Kind`, `Terminal` (strip only), `Frame` (`x`, `y` in mm) |
| a pin, `ExternalInterface` of a symbol | `Pin` interface | `Name`; `X`, `Y` offset from the symbol in mm; `Faces` (`up,down,left,right`) |
| a cable and its cores | `Cable`, `CableCore` | `Tag`; cores by name |
| `Nets` group, one element per net | `NetGroup`, `Net` | `Potential` |
| net point, `ExternalInterface` of a net | `NetPoint` interface | |
| link from a net point to a symbol pin | `InternalLink` | `RefPartnerSideA` pin id, `RefPartnerSideB` net point id |
| a straight run of a net's wire, child of the net | `Wire` | `Page`; `X1`, `Y1`, `X2`, `Y2` in mm |
| an interruption point (a `Symbol` with `Kind` `interruption`), child of its page | `Symbol` | `Net`: the name its pair carries; one pin at offset 0,0 |

A device has one element and one `Symbol` child per place it is drawn (a coil on one page, its contacts on
others). Positions are millimetres on the page, origin bottom left, y up. `Kind` is a plain device word:
`coil`, `contact`, `terminal`, `lamp`, `motor`, `earth`, `pin`, `plug`, `power_poles`, `thermal_switch`,
`enclosure`, `generic`; any other word draws as a box.

## 3. What the importer does

- Pages: one project page per `Page`, named by label and title. A page that already exists is reused and its
  device-less symbols are not drawn again.
- Symbols: a KoperCAD library symbol when the kind has one and the pin names match (coil, NO and NC contact,
  changeover, terminal and tap terminal (3 pins), lamp, earth, device pin, interruption point, power poles
  (2, 4, 6 or 8 pins: a contactor with 1 to 4 poles), overload trip contact, motor (2, 3, 4, 6 or 7 pins:
  DC, single phase, three phase, with thermistor contacts, six ends) and a plug (2, 4 or 5 pins), the pin
  names deciding which pin is which); otherwise a labelled box. Either way the
  pins are exactly where the file has them: the library symbol is turned to face the way the file's pins face
  and scaled to the file's pin pitch (EPLAN's 8 mm contact on KoperCAD's 16 mm symbol is drawn at half size,
  and a lead covers any pin the scaling does not reach). A box has each pin on the side it faces, so rows of
  poles give a flat box between the rows. Pin names are the file's. No text is added here.
- Wires, when the net has `Wire` runs: the runs are joined where they meet. Two runs meeting where no pin sits
  are one wire with a bend; three or more make a junction; a run end on a pin is that pin; two pins on one spot
  are wired to each other. Every wire starts and ends exactly on its pin or junction and follows the file's
  route, so none is chained by reading order or cuts through a symbol.
- Wires, when the net has no runs (an older file): each net becomes right-angle wires between exactly its
  pins, chained in reading order. A net that reaches pins on several pages gets an interruption point named like
  the net on each page, which KoperCAD pairs by name.
- One undo step; it only adds (a device whose tag exists is left as it is, with its symbols).
- Counted and named in the preview: links that end on no drawn pin, symbols of a skipped device, elements
  without a usable tag, repeated tags.

## 4. Exact and approximated

The import report lists the approximated parts apart from what was left out: **Guessed** (symbols drawn as boxes, wire routes the file does not give, devices without a footprint) shows in the import dialog, in the text report and as `guessed` in `kopercad-cli import --json`.

Exact: pages, positions, pins, nets, tags, part numbers, the part's `Width`, `Height` and `Depth` when the file
gives them (they become the part's `width_mm`, `height_mm`, `depth_mm`; a size the catalog already has stays), and (with `Wire` runs) the wire routes. Approximated:
the symbol artwork (a library symbol or a labelled box, fitted to the file's pins) and, for a file without runs,
the wire routes.

## 5. Where the crane configurator writes it

Every build of the crane configurator writes `<order>.aml` next to the EPLAN project; a built project can
also be exported on its own with `scripts/kopercad_export.py`. In KoperCAD: File > Import and export > Import
AutomationML. The EPLAN route stays as it is.

Tests: `crates/novo-interop/tests/aml_drawing.rs` (a hand-written file; an opt-in test reads a real export
named by `KOPERCAD_CRANE_AML`) and `crates/novo-cli/tests/aml_geometry.rs`, which looks at the result as
geometry: pins on the file's spots, wires between their pins on their routes, no wire across a symbol it does
not connect to (invented page `tests/fixtures/aml/hoist-feeder.aml`; the same checks run on a real export).
To look at an import: `cargo run -p novo-cli --example aml_pages -- file.aml out_dir` writes every page as SVG.
