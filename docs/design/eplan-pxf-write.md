# EPLAN PXF writer (EPLAN-PXF-WRITE)

KoperCAD project in, an EPLAN-importable PXF project file (`.epj`) out. Entry points:
`File > Export > Export EPLAN project (PXF)` (command field `export eplan`) and
`kopercad-cli export eplan project.novo -o project.epj` (never replaces a file).
Code: `crates/novo-interop/src/eplan/pxf/{write,dialect}.rs`. Knowledge source: only
`/mnt/project-files/interop/crane-configurator-findings.md`; nothing is copied from EPLAN.

## Target release

EPLAN **2024** (PxfVersion 1.23), which is what Tadjiro runs now. Everything that belongs to one
release lives in `Dialect` (`pxf/dialect.rs`): root `Version`/`PxfVersion`/`SchemaVersion`, the
`ESymLib` rows, the sheet size the position origin refers to and the default language. A new
release is a new `Dialect` constant plus a check of the rows below against a file it wrote.

**EPLAN 2027 (planned upgrade): unknown.** I know of no source for its PXF version, schema
version, symbol library ids or id rules, and I do not guess them. There is no 2027 dialect, so
nothing is written for it. Before the upgrade: export one small project from 2027, run
`kopercad-cli eplan survey` on it, compare with the 2024 survey, and add a `Dialect`.

## What is written

| KoperCAD | PXF | Status |
| --- | --- | --- |
| Root, `EplPxfDir`, `ESymLib` rows | as the findings describe them | field names unverified against EPLAN |
| Page | `O4`, `P49` (function, location, number), title as `P11 P11011` | unverified |
| Device on a page | one `O17` per placed symbol: `P150` tag, `P144` display tag, `P20027` description, `P20100_1` `MAKER.TYPE`, `S40x1201 A762` position (y flipped to the sheet's bottom-left origin) | unverified |
| EPLAN symbol reference | `A1381/A1382/A1383` only when the person's own symbol table has a key for the symbol | unverified; without a key the device has no symbol reference |
| Terminal | one `O17` per terminal with the strip's tag and `P20030` | unverified |
| Cable | `O17` with `P20011` (`-` when it has no description), on the first page | unverified |
| Cable core | `O59` with the cable's tag and `P31004` | unverified |

Text rules from the findings: `;` ends a language entry (replaced by `,`), an empty
multi-language text hangs the import (never written), one number space for all ids and every id
unique (one counter, starting at 1: a fresh project has nothing above it).

## What is not written yet (counted in the export report)

Wires and connection nodes (need O31/O42 end points that are unverified, see the survey),
free texts, interruption points and other symbols, devices and terminals with no drawn symbol
(EPLAN devices live on drawn symbols), tags with more than one device level.

## Verified versus unverified

- Verified: the file is well formed, ids are unique, and `read_pxf_str` and `plan_pxf` read
  every field back (tests in `tests/pxf_write.rs`). That proves agreement with our reader.
- **Not verified: that EPLAN accepts the file.** No KoperCAD file has been opened in a real
  EPLAN. Every export says so. The first real test is: import into a throwaway EPLAN 2024
  project, then report what EPLAN says; each such report becomes a test here.
