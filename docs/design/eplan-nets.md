# EPLAN nets and EPLAN-style checks (EPLAN-NETS)

Two things, both written from behaviour, never from EPLAN content or customer
data. Sources are the interop findings (`interop/crane-configurator-findings.md`
in the project files, sections 2.1, 4 and 5 item 4) and general drafting
practice; nothing proprietary was copied and no customer project is a fixture.

## 1. Net reconstruction (`novo_interop::eplan::nets`)

EPLAN stores few explicit connections. The reader (EPLAN-PXF-READ) fills a
neutral `ConnGraph`; `rebuild_nets` returns nets plus issues; `compare` checks
them against KoperCAD's own `Netlist`.

| # | Rule | Source |
| --- | --- | --- |
| 1 | Connection points facing each other with nothing between them are joined, over any distance. The first point met must face back; a symbol body (blocker) in between stops the run. | Findings 2.1 "Connection semantics" |
| 2 | A drawn line joins what its ends touch. A node on a line's interior is a T joint; a plain point on a line does not join, and crossing lines do not. | Findings 2.1 (node legs, `O42`) and drafting practice |
| 3 | A node is one point with several legs, each leg a ray as in rule 1. | Findings 2.1 (`A1262`/`A1263` leg table) |
| 4 | A terminal is one conductor: its sides are one net. | Findings 2.1 |
| 5 | Interruption points of one full name pair up two by two in page order; an odd one is unpaired. | Findings 2.1 |
| 6 | Explicit bridges (jumpers) join two points. | KoperCAD's own model |

Not carried: contacts never join nets (as in KoperCAD); cable cores are not
nets of their own here. Positions snap to a micrometre.

**Where the user finds it:** the import preview of File > Import and export >
EPLAN project (EPLAN-PXF-READ) shows the counts and the issues; the nets are
what the importer draws as wires (`ReconNets::spanning_links`). `compare` is the
cross-check run after the import has created wires; its two lists say what EPLAN's rules join that
KoperCAD does not (`import_only`) and the reverse (`project_only`).

Validation: unit tests per rule in `eplan/nets.rs`; `tests/eplan_nets.rs` draws one
synthetic page set both ways (EPLAN geometry, and the wires a draftsman would
draw) and requires equal nets, then removes a wire, adds a wire, and uses three
interruption points of one name to show each disagreement is reported.

Known differences from EPLAN to confirm against a real export (open: sample from
Tadjiro): lines meeting a pin in its interior, junction behaviour of
non-orthogonal lines (only axis-aligned runs are reconstructed), and the leg
table of `O42` nodes (the reader maps it).

## 2. EPLAN-style checks (`novo-checks`)

These appear in the Issues list, the issue pill and F8 like every other check
(Outline > Issues, filter chip by name) and can be waived.

| Kind | Severity | Rule | Source |
| --- | --- | --- | --- |
| `AmbiguousInterruption` | warning | A name used by more than two interruption points. EPLAN pairs two by two; KoperCAD joins all, so an imported project and this one read it differently. | Findings 2.1 |
| `WireNumber` | warning | A wire number on two nets, or several numbers on one net. | Drafting practice (EPLAN reports non-unique connection designations) |
| `UntaggedDevice` | warning | A device without a product level (`-K1`). | Drafting practice |
| `DuplicatePage` | warning | Two pages with one name in one structure. | Drafting practice |
| `DanglingJunction` | warning | A junction no wire ends on (EPLAN: node leg without connection). A junction with one wire is allowed. | Findings 5 item 4 |

Already in `novo-checks` and matching the findings' catalogue: duplicate tags,
lone interruption point, unconnected pins, potential conflict, terminal checks.
The remaining ideas in the crane repo's project-check catalogue
(`PROJECTCONTROLE.md` section 7) are not readable from this session (private
repo, outside this project's scope); an item list from that thread would
extend this table.
