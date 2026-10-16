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
conflict), C13 partly, A09/A12 partly (duplicate page names).

### 2.1 The second batch (`house_rules.rs`, tests in `tests/house_rules.rs`)

All warnings. They show in Outline > Issues like every other check (filter chip
by name, F8, waivers). Rules that need a convention the model does not carry
are written against a project or device property and stay quiet without it.

| Kind | Rule | Reads |
| --- | --- | --- |
| `UnwiredTerminal` (C02) | A terminal with no wire on any side, and a plug pin wired on neither half (or at all, for a plug without a mate). | wires, terminal symbols; plug halves |
| `StrayWireEnd` (C03) | An open wire end within 0.2 mm of a pin, a junction or another wire's line: it looks joined and is not. | wire geometry |
| `ContactType` (C05) | NO contacts end in 3, 4 (7, 8 delayed); NC in 1, 2 (5, 6 delayed and the 95-98 of overload relays); changeover 1, 2, 4. Names of two digits or more only. | pin names, symbol key |
| `ContactPosition` (C06) | The tens digit of a contact (the `2` of `24`) is beyond the sum of `contacts.no`, `.nc`, `.co` the device declares. Quiet without a declared total. | device properties |
| `ContactDrawnTwice` (C07) | Two contact symbols of one device with the same pin numbers. | pin names |
| `VoltageClassMix` (C10) | Extra-low (50 V AC, 120 V DC) and low voltage nets on the pages of one function code. | potentials with a voltage, page structure |
| `ConductorContinuity` (C11) | A PE or N terminal, wired, whose net carries no PE/PEN (N/PEN) potential, in a project that defines one. | netlist potentials |
| `MotorPhases` (C14) | A symbol with pins `U1 V1 W1` whose windings share a net, two get the same potential, or one is on N/PE. | netlist |
| `CoilSupply` (C15) | A main symbol with `A1`/`A2` whose sides share a net or potential, or touch PE/PEN. A coil between two different line potentials is fine. | netlist |
| `PlugPinOneSided` (C17) | A pin wired on one connector half and not on the other. | connector devices |
| `TagRule` (F01) | With project property `tag.rule` = `page-column`, a device name must be letters + two-digit page + one-digit column of its main symbol, `.n` for repeats. | page name, `PageFormat::column_at` |
| `DisplayedTag` (F04) | A typed text on a device symbol that starts with `-`, `=` or `+` and is neither the full tag nor the short tag. | texts |
| `StructureCode` (H01) | With project property `structure.list` = `complete`, a function or location code (on devices or pages) that no structure node names, parents included. | structure list |

Where the user sets what a rule needs:

- **F01 and H01:** File > Project settings (command `project.standard`), section
  "Extra design checks": "Device names follow page and column" sets
  `tag.rule` = `page-column`; "My structure list is complete" sets
  `structure.list` = `complete`. Both are stored in the project's properties
  (`ProjectInfo::properties`, so no novo-core change), one undo step, off by
  default.
- **Plug halves (C17, C02):** a plug or socket is a `Connector` device. Select
  it and fill "Other half" in the inspector with the tag of its mate
  (`-XF1`); the property is `plug.mate`. Set it on both halves; the check
  reads either. KoperCAD's netlist does not join plug halves; C17 and C02
  report the wiring, not a joined net.
- **C06:** the device properties `contacts.no`, `contacts.nc`, `contacts.co`
  (also read by the too-many-contacts check) say how many auxiliary contacts a
  device has; they come with its part.

Not carried: C08, C12, C16 and C18 are not in this lane; D, E and G are house
rule packs. A real-sample test for these rules waits for the PXF reader
(`KOPERCAD_PRIVATE_SAMPLES`); the fixtures here are synthetic.
