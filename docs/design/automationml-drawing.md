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
| a device, named by its designation (`=A1+CAB1-K1`) | `Device` | `Tag`, `Description`, `Manufacturer`, `OrderNumber` |
| a terminal strip (`+CAB1-X1`) | `TerminalStrip` | `Tag`; one `Terminal` interface per terminal |
| a symbol of a device or strip, child of it | `Symbol` | `Page` (a page name), `Kind`, `Terminal` (strip only), `Frame` (`x`, `y` in mm) |
| a pin, `ExternalInterface` of a symbol | `Pin` interface | `Name`; `X`, `Y` offset from the symbol in mm; `Faces` (`up,down,left,right`) |
| a cable and its cores | `Cable`, `CableCore` | `Tag`; cores by name |
| `Nets` group, one element per net | `NetGroup`, `Net` | `Potential` |
| net point, `ExternalInterface` of a net | `NetPoint` interface | |
| link from a net point to a symbol pin | `InternalLink` | `RefPartnerSideA` pin id, `RefPartnerSideB` net point id |

A device has one element and one `Symbol` child per place it is drawn (a coil on one page, its contacts on
others). Positions are millimetres on the page, origin bottom left, y up. `Kind` is a plain device word:
`coil`, `contact`, `terminal`, `lamp`, `motor`, `earth`, `pin`, `plug`, `power_poles`, `thermal_switch`,
`enclosure`, `generic`; any other word draws as a box.

## 3. What the importer does

- Pages: one project page per `Page`, named by label and title. A page that already exists is reused and its
  device-less symbols are not drawn again.
- Symbols: a KoperCAD library symbol when the kind has one and the pin names match (coil, NO and NC contact,
  changeover, terminal, lamp, earth, device pin); otherwise a plain box with the file's pins. Pin names are
  the file's. No text is added.
- Nets: each net becomes right-angle wires between exactly its pins, chained in reading order. A net that
  reaches pins on several pages gets an interruption point named like the net on each page, which KoperCAD
  pairs by name. Nothing is guessed from where pins face each other.
- One undo step; it only adds (a device whose tag exists is left as it is, with its symbols).
- Counted and named in the preview: links that end on no drawn pin, symbols of a skipped device, elements
  without a usable tag, repeated tags.

## 4. Exact and approximated

Exact: pages, positions, pins, nets, tags, part numbers. Approximated: the symbol artwork (the library
symbol's pins land where it has them, the first pin is placed on the file's first pin) and the wire routes.

## 5. Where the crane configurator writes it

Every build of the crane configurator writes `<order>.aml` next to the EPLAN project; a built project can
also be exported on its own with `scripts/kopercad_export.py`. In KoperCAD: File > Import and export > Import
AutomationML. The EPLAN route stays as it is.

Tests: `crates/novo-interop/tests/aml_drawing.rs` (a hand-written file; an opt-in test reads a real export
named by `KOPERCAD_CRANE_AML`).
