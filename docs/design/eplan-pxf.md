# EPLAN PXF reader (EPLAN-PXF-READ)

Status: slices 1 to 3 built (streaming reader, import plan with preview and one-step apply,
File > Import > EPLAN project (PXF), `kopercad-cli import eplan`). Nets, symbols, texts and
interruption points follow.

## 1. What and why

EPLAN's project export (`.epj`) and page macro (`.emp`) are the same plain XML, the PXF
exchange format. Reading it is the way to bring a whole EPLAN project into KoperCAD
without EPLAN's API or a licence. The user exports from their own EPLAN
(`export /TYPE:PXFPROJECT`), KoperCAD reads the file.

Rule of the lane, as for all interop: what is carried is counted, what is not carried is
named (`novo_interop::report::Report`). No EPLAN macro, symbol library, manual text or
vendor data is in this repository; fixtures are written by hand and synthetic.

## 2. Field table (the layout the reader assumes)

Taken from projects exported by one EPLAN release (PxfVersion 1.23), as described in
`/mnt/project-files/interop/crane-configurator-findings.md`. Not verified against a
second release or against EPLAN's documentation.

| Meaning | Where |
| --- | --- |
| Root | `EplanPxfRoot` with `Type` (`PageMacro` for `.emp`), `Version`, `PxfVersion`, `SchemaVersion`, `NumMainObjects` |
| Object | element `O<type>` with id `A1="<type>/<number>"`; ids are unique, devices and cores share one number space |
| Page `O4` | `P49` attributes `P1100` function, `P1200` location, `P11012` number, `P11013` suffix; title `P11 P11011`; number also `A1101` |
| Device tag | `P150`: `P1100` function, `P1200` location, `P20012` prefix, `P20013` letters, `P20014` number, `P20015` sub index; `P144` displayed tag |
| Device data | `P20027` type text, `P20100_1` part number `MAKER.TYPE`, `P20030` terminal text, `P20011` cable text |
| Cable core `O59` | `P31004` core number |
| Interruption point `O70` | tag in `P150`, free text `P24810`; same full name pairs up two by two in page order |
| Text `O30` | multi-language string `xx_XX@text;` |
| Position | `S40x1201 A762="x/y"`, millimetres, y up, origin bottom left |
| Symbol | `A1381` number, `A1382` variant, `A1383` library set |

## 3. Exact and approximated

Exact: everything in the table, read as written. Multi-language strings follow the
format's own rule (a `;` ends an entry, so a `;` inside a text truncates it).

Approximated, and said so in the report or here:
- Any object carrying a tag is a device; a terminal text makes it a terminal, a cable
  text a cable. Object type numbers other than the ones named are not interpreted.
- A free text is the first multi-language value inside an `O30`.
- An object sits on its enclosing page, else on the last page seen.
- The device name joins prefix, letters, number and `.sub`; the display tag is kept
  next to it so a mismatch is visible.

Not carried in slice 1 (counted): lines, arcs, connection nodes (wires), groups, boxes,
function blocks, symbol definitions. Wires come with the net rebuild by EPLAN's rules
(facing pins, node legs, interruption pairs); symbols come with the symbol mapping lane.

## 4. Slices

1. **Reader and model** (this one): `novo_interop::eplan::pxf::{read_pxf, read_pxf_str}`,
   `PxfFile`, `PxfFile::report()`, interruption pairing. Streams from any `BufRead`.
2. (built) Import plan: `PxfPlan` previews against the open project (what is added, what exists,
   the "not carried" list), `apply` is one undo step through commands; pages, structure,
   devices with tags and part numbers, terminals, cables with cores, texts.
3. (built) UI and CLI: File > Import > EPLAN project (PXF) (command field `import eplan`) with
   the preview dialog; `kopercad-cli import eplan project.epj [--project p.novo] [-o new.novo] [--json]`.
   Without `-o` nothing is written.
4. Interruption points to KoperCAD cross references; `.emp` into the macro library.
5. Nets, with the EPLAN-NETS lane.

## 5. Safety

The tokenizer never follows an entity or a DOCTYPE, caps a single tag at 64 MB and
nesting at 512 levels, and reads once front to back.
