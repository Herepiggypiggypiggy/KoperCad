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
| 1 | Ports on one line (other coordinate within 0.2 mm) that face each other are joined, over any distance. The nearest port must face back; any other port met first, facing or not, stops the run (reported as `BlockedRun`). Symbol bodies, texts and lines do not stop it. Two interruption points never join this way; ports of one symbol ignore each other. | Findings 9.2 |
| 2 | A drawn axis-aligned line joins what its ends touch. A node on a line's interior is a T joint; a plain point on a line does not join, and crossing lines do not. Skew lines are counted (`SkewLine`), not used. | Findings 9.2 items 5-7 |
| 3 | A node is one point with several legs, each leg a ray as in rule 1; the reader maps `A1262`/`A1263` to legs with the table in findings 9.1. | Findings 9.1 |
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

Findings section 9 states this is the crane repo's observed model, not EPLAN
documentation: confirm on a real export (open: sample from Tadjiro), in
particular lines ending inside a pin's interior (treated as no connection) and
diagonal connections (treated as drawn only). `Blocker` is optional and unused
by the observed model.

## 2. EPLAN-style checks (`novo-checks`)

These appear in the Issues list, the issue pill and F8 like every other check
(Outline > Issues, filter chip by name) and can be waived.

| Kind | Severity | Rule | Source |
| --- | --- | --- | --- |
| `AmbiguousInterruption` | warning | A name used by more than two interruption points. EPLAN pairs two by two; KoperCAD joins all, so an imported project and this one read it differently. | Findings 2.1 |
| `WireNumber` | warning | A wire number used on two nets. Several numbers on one net are not reported: the sides of a terminal or device are separate connections, and the shipped lessons number junction branches separately. | Drafting practice (EPLAN reports non-unique connection designations) |
| `UntaggedDevice` | warning | A device without a product level (`-K1`). | Drafting practice |
| `UnnamedTerminal` | warning | A terminal without a number or name (C13). | Findings 9.3 C13 |
| `SameShortTag` | info | One short tag under two function or location levels: separate devices (F03). | Findings 9.3 F03 |
| `DuplicatePage` | warning | Two pages with one name in one structure. | Drafting practice |
| `DanglingJunction` | warning | A junction no wire ends on (EPLAN: node leg without connection). A junction with one wire is allowed. | Findings 5 item 4 |

Already in `novo-checks` and matching findings 9.3: C01 (unconnected pins), B05
(lone interruption point), C04 (contacts without a coil), C09 (potential
conflict), C13 partly, A09/A12 partly (duplicate page names). Portable and not
yet done: B01 (node legs, import-time: `DanglingLeg`), C02, C03 (import-time:
`BlockedRun`), C05-C07, C10, C11, C14, C15, C17, F01, F04, H01. D, E and G are
house rule packs, not defaults.
