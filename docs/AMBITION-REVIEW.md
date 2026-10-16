# Ambition review: is KoperCAD special and complete yet?

Status: 2026-10-15 19:50 UTC. Audit of `main` (`194c373`) and the open pull request list at 19:25 UTC.
It answers one standing question from Tadjiro: *can this replace suites that cost thousands of dollars?*
Short answer: **the schematic core is already past "usable", the engineering depth around it is not.** The
gaps that matter most are not more buttons. They are the places where a commercial workflow (or a second
tool next to it) still does the thinking for a panel builder: part data at scale, protection and cable
engineering, the whole panel-to-workshop chain, real simulation, standards compliance and variants.

## 1. How to read this document

- **Have** means merged on `main`. **In flight** means an open pull request (PR numbers as of 19:25 UTC).
  Nothing in flight is counted as done.
- **Commercial suites** means EPLAN Electric P8 with Pro Panel, SEE Electrical, Zuken E3.series, SolidWorks
  Electrical and AutoCAD Electrical, plus the specialist tools a panel builder runs next to them
  (maker selection and sizing tools, short-circuit and selectivity tools, functional safety calculators).
  The comparison lines come from public product pages and general industry knowledge, not from hands-on use of
  these programs, and nothing here copies their data, symbols, UI or file formats. Where a line is unsure it says so.
- **Model vs exact.** The engines named below are planning aids unless a validation against published
  reference cases exists (section 6, "validation"). Ampacity tables in #412 were written from memory and must be
  checked against IEC 60364-5-52 before any result is shown as more than a planning value.
- Gap impact is ranked for a **panel builder and electrical designer** who delivers a machine or a cabinet:
  how often the gap forces them out of KoperCAD, and how much time or risk it costs when it does.
- The earlier Pro Panel plan (#379, ranks R0 to R9) is folded in. Most of R0 to R3, R5, R6 have since
  become PRs; the table says which.

## 2. Area by area: what we have, what the top suites do, the gap

Size hints are lines of Rust in the crate on `main`: novo-core 14.9k, novo-editor 14.7k, novo-canvas 12.3k,
novo-parts 8.9k, novo-reports 6.4k, novo-workflow 6.0k, novo-interop 5.7k, novo-terminals 5.4k, novo-layout 4.4k,
novo-view3d 3.3k, novo-sim 1.9k, novo-routing 1.8k. About 1,450 tests pass on main.

| Area | What we have | What a top suite does | Concrete gap |
| --- | --- | --- | --- |
| **Schematic drawing** | Pin-to-pin wiring, derived corners and junctions, wheel and Ctrl+K command field, auto tags, smart duplicate, Corners/Lines modes, 10,000-symbol timings, inline rename | Same, plus graphics editing of symbols, multi-line and single-line views, orthogonal/auto-routing assist | Strong. No symbol editor, no single-line (power distribution) view, no page templates editor. Hands-on Windows verification still outstanding |
| **Symbols and standards** | IEC 60617 and NFPA sets (about 184 each, #480 for the NFPA breadth), packs design, aliases | Libraries of several thousand symbols, GOST/GB/JIS/AS packs, symbol and macro editors | Breadth is a fraction of a suite; no way for the user to draw a symbol; no regional packs |
| **Parts and library** | Catalog model, assign, bulk table, STEP bounding box, BMEcat reader, EPLAN parts import (#347), power loss and clearances (#419), starter catalog of about 30 parts, Import button and browser design | Hundreds of thousands of maker parts with macros, 3D, order data, alternatives, via a data portal and maker-sponsored data | **The library is nearly empty.** A CAD without parts is a drawing tool. Network policy blocks maker sites from cloud sessions, so import must run on Tadjiro's PC; WAGO needs a login |
| **Cross references** | Contact images, jumps, linking, interruption points that cross reference | Same, plus parent/child path references, contact mirror pages, 3D xref in reports | Good. Path and function-based references are only partial |
| **Terminals, cables, plugs** | Same-name join rule, strips, bridges, terminal diagrams, cable proposals at boundaries, cores, shields, screens; strip diagrams and cable schedule (#416 in flight, #422 merged, #498) | Terminal and cable plans, plug diagrams, jumper catalogs, accessory auto-add | Plug connection list and accessory parts (end plates, bridges as BOM lines) are in flight, not merged. No multi-core harness or connector pin-out design |
| **Design checks** | 17 problem kinds (tags, contacts, potentials, open wires, undersized wire, terminals, PLC, overlap...), Issues list, waivers (#413 in flight) | Hundreds of rule-based checks with severity, user-defined rules, project-wide check sets | Small, mostly topology checks. No user-defined rules, no standards rule sets (see compliance) |
| **Electrical calculations** | Pure library in draft (#412): ampacity, voltage drop, short circuit, motor, PE; checks on it in draft (#454) | Ecodial, DOC, ETAP and similar: cable sizing with grouping and temperature derating, Ik at every node, time-current curves, selectivity and back-up tables from maker data | **Draft only, and one-way.** No curves, no coordination, no maker trip data, no network (transformer/generator/source) model, no load summary or busbar sizing. Biggest engineering gap |
| **Cabinet (Pro Panel)** | Enclosure, plate, rails, ducts, placement, place-all planner, clearances, enclosure sizing helper, door swing and depth (#409), plate drawing (#397, #470, #477), drill list, cut list design | Pro Panel: 3D layout space, mounting surfaces (plate, door, wall, profile), 3D collision, rule-based placement, machining data per machine, assembly instructions | 2D with a depth constant. Back plate only in practice, no door/side mounting, no 3D collision, no machine formats, planner is first-fit not rule-based |
| **Wire routing and lengths** | Duct graph, shortest path, fill penalty, lengths, duct drawing (#461), wire list with cut lengths (thread done) | 3D routing through real ducts and wiring holes, bundling, wire production export per wire machine | Needs connection point positions on every part; routing is not reachable as a one-click "route all" with preview in the app; no wire-processing export |
| **3D view** | `novo-view3d` scene, camera, picking, culling, look and feel (#457) on main; wgpu renderer (#481), scene builder (#391, draft) and STEP to mesh (#449, draft) in review | 3D mounting layout, collision, photo-real or CAD-like views, MCAD round trip | Views, not edit. No placement in 3D, no door animation, no mesh collision, no STEP export of the assembled cabinet |
| **Thermal** | Plate heat map and sizing helper with IEC 60890 effective-area method (#406, #402/#441) | Loss total, loss density map, hand-off to maker climate tools; no real CFD in the suites themselves | Two threads started (physics engine, visuals) for real 3D CFD with conjugate heat transfer. The right call. Needs benchmark validation and cooling selection (section 6) |
| **Simulation** | Netlist and live potentials, coil/contact coupling, timers, lamps and motors glow, PLC outputs (#333), device animation (#346) | Mostly none in schematic suites; add-ons (PLCSIM, EPLAN Simulate, SimScape) | Logic-level only: no analog solver, no motor start behaviour, no trip curves, no latching fixpoint, no PLC program run, no fault scenarios |
| **PLC and I/O** | Cards, addresses, channels, I/O list, plc checks, plc design | PLC and bus extension, symbol tables and address export to TIA, Studio 5000, CODESYS, fieldbus topology | No export to PLC engineering tools, no bus topology, no functional safety (PL/SIL) verification |
| **Reports** | BOM, device list, wire list, contents, cut list, diagrams, labels (#482 in flight), Excel/CSV, Reports hub (#452) | Report designer with user-built templates, form and title block editor, label printers, language switching | Fixed report set; no report designer; no multi-language documentation |
| **Revisions** | Issue revision, changes mode, change report PDF (#469), compare window (#468, #487), page freeze | Revision management with markup, history, visual diff of pages | Good start. No markup/comments, no merge between two edited copies of a project |
| **Macros and spreadsheets** | Save/Place macro (#489), browser (#455), library thread, spreadsheet round trip with preview (#430, #433, #423) | Macros with parameters and variants, rule-based project generation, project options | Macros are static copies. No parameters, no variants or project options, no generator from a spec sheet |
| **Interop** | DXF reader/writer with background underlay (#385, #497), EPLAN parts import, Excel round trip, Python and CLI | DWG, STEP/IGES, PDF markup, XML exchange formats, PLC and MCAD links, import of the other suites' projects | No DWG, no project-level migration from a competitor, no AutomationML/PLCopen, no STEP export |
| **Commissioning** | Engine, edit lock, Pass all open, punch list, report, tablet layout (in flight) | Tablet checklists, redlining, as-built update | Engine is ahead of the field UI; no photo or signature capture, no as-built round trip into the drawing |
| **Learning and onboarding** | `novo-learn` lesson engine, tracks and lessons (#474 and drafts), onboarding walk-through, sample project | Vendor training portals; mostly outside the product | Ahead of the field. Content depth is the work, not the engine |
| **Platform** | Local SQLite project, autosave and recovery, per-user installer, `kopercad-cli`, Python bindings | Multi-user project servers, check-in/out, cloud or on-prem | Single user by design. A file-based sharing and merge story is missing |
| **UI shell** | Five workspaces, menu bar from the registry, status bar, design system pass, shortcut sheet design | Ribbon or navigator-driven UIs | Mid-flight. No Dutch UI, no accessibility pass, no hands-on Windows sign-off for the whole new shell |

## 3. Ranked gaps

Ranked for a panel builder and electrical designer. "Risk" is the likelihood the lane stays small if not
split. **Effort** is S (one PR), M (two to four), L (five or more), XL (a multi-thread lane).

| # | Gap | Why it ranks here | Effort | In flight? |
| --- | --- | --- | --- | --- |
| 1 | **Parts library depth: makers, data pipeline, geometry** | Every other feature needs parts. Without 10,000+ real parts with terminals, dimensions, loss and geometry, nothing sizes, routes or orders | XL | Parts library threads, #347, #356, #449; no data yet |
| 2 | **Protection and cable engineering: sizing, Ik, selectivity, SCCR** | The core of "intelligent". The designer today does this in a second tool and types the result back | XL | #412 and #454 are drafts, one way |
| 3 | **Wire routing, lengths and wire production** | Ends hand-typed wire lengths and feeds the workshop. Needs part pin positions | L | Engine done, #461 ducts, lengths merged; no app "route all" |
| 4 | **Panel layout intelligence** | Rules-based place-all, mounting surfaces (door, side, wall), 3D collision, door swing, rail and duct cut optimisation | L | #409, #419, #397; planner still first-fit |
| 5 | **Standards compliance: IEC 60204-1, NFPA 79, UL 508A** | Panel builders sell a compliance statement; the checker and the report are the product | L | Nothing |
| 6 | **Variants and generated engineering** | Machine builders sell options. Parametric macros, project options, and a generator from a spec sheet turn days into minutes | L | Macros (#489), sheets; no variants |
| 7 | **Thermal to a validated cooling decision** | Heat map to a cooling unit selection with a real CFD result the user can trust | L | Two threads just started |
| 8 | **Real simulation** | Analog and logic solver, motor start, trip curves, PLC program run, fault scenarios, timing diagrams | XL | Logic level and device models only |
| 9 | **PLC engineering link and functional safety** | I/O lists to the PLC programmer; ISO 13849 PL calculation for safety functions | L | Plc crate and checks; no export |
| 10 | **Migration and interop** | Bring an existing standard, symbols and projects in (public formats), plus DWG/DXF, STEP, AutomationML, PLCopen | L | DXF, EPLAN parts, sheets |
| 11 | **Documentation designer: forms, reports, multi-language** | Customer-specific documents and a translated deliverable are routine | L | Labels (#482), no designer |
| 12 | **Workshop and field mode** | Assembly instructions, commissioning on a tablet, punch list, as-built redlines back into the drawing | L | Commissioning engine, assembly checklist (#451) |
| 13 | 3D editing (place in 3D, door animation, mesh collision) | Presentation and confidence; after rules and surfaces | L | View only |
| 14 | Symbol editor and regional symbol packs | Closes the breadth gap faster than hand-drawing | M | Packs design only |
| 15 | Collaboration: file-based share, semantic merge, review comments | Teams; local-first shape needed | L | Revisions only |
| 16 | AI assistant (WS-23): proposes from a spec using the engineering engines | Valuable after 2 and 6 give it something correct to call; offline or opt-in only | L | Not started |
| 17 | Wire harness and connector design (E3-style) | Separate product-sized effort | XL | Not started |
| 18 | Fluid power and P&ID | Separate product-sized effort | XL | Not started |
| 19 | Dutch UI, accessibility, UI localisation | A real user need (Dutch workshop), cheap relative to value | M | Aliases only |

Items 1 and 2 are where "special" is decided. Items 13 to 19 can wait; they are listed so we do not forget them.

## 4. Top 12: proposed thread briefs

Each brief is the whole ask for a thread: title, what "complete" means, how it is validated, and where the user
finds it. Complete is not "a marker": each says what a top suite does and holds the bar. Lanes that are too large
say how to split them rather than shrinking.

### 1. Parts library: maker data pipeline at scale (split into 4 threads)
- **Threads:** (a) import pipeline and data model for ETIM/BMEcat, CSV, XML and maker downloads, one-click import on the user's PC; (b) maker packs for Wago, Phoenix Contact, Weidmuller, Siemens, Schneider, ABB, Rittal, Murrelektronik, Lapp (each as an import recipe the user runs, never bundled data); (c) part quality: duplicates, alternatives, missing data report, lifecycle status; (d) part geometry and connection points: STEP to mesh, pin positions, macro mapping.
- **Complete means:** a user imports a maker catalog in a few minutes, parts carry order number, price field, dimensions, loss, clearances, connection points and a 3D mesh, a search finds a part by order number, name or function, the BOM and orders follow, and a missing-data report lists what is guessed.
- **Validation:** golden import of a public sample catalog from each maker format; a 100,000-part catalog opens and searches under 100 ms; round trip of exports; parts used by the starter project resolve from imported data.
- **Where:** Parts workspace > Import, Library menu, part inspector.
- **Constraints:** never copy EPLAN's library; WAGO needs a login (hand-off only); maker sites are blocked from cloud sessions, so the recipe runs on Tadjiro's machine.

### 2. Protection and cable engineering (split into 3 threads)
- **Threads:** (a) *Calculation core*: IEC 60364-5-52 ampacity with grouping, temperature, installation method; voltage drop; Ik at each node with source impedance; PE loop; checked against IEC tables (replace the from-memory tables in #412). (b) *Coordination*: maker time-current and let-through data, selectivity and back-up tables, motor starter coordination (type 1 and 2), overload and short-circuit trip. (c) *Selection and the one-click starter*: from motor kW and supply, choose breaker, contactor, overload, cable, terminal, and write a validated circuit.
- **Complete means:** the designer sees, per circuit, a pass/fail with numbers and a curve, an explanation of what was assumed, and a tool to fix it; unknown data shown as unknown, never defaulted silently.
- **Validation:** worked examples from IEC 60364-5-52 annexes and published textbook cases as tests; compare against two public calculators; sample projects with deliberate errors.
- **Where:** Checks workspace (Calculations tab), device inspector (Protection), Ctrl+K "size this circuit".
- **Note:** source of truth for ampacity must be the standard, with a visible table version. Planning values until validated.

### 3. Routing and wire production in the app
- **Complete means:** *Route all* with preview from the netlist and part pin positions, wire lengths by 3D path, duct fill check, wires bundled per duct, wire list with cut lengths and end processing, ferrule and label output, generic wire-cut CSV plus one named machine format once the workshop names the machine.
- **Validation:** golden routes on fixture cabinets with hand-measured lengths; duct fill matches a hand calculation; round trip into the wire list.
- **Where:** Cabinet workspace > Route, Reports > Wire list.
- **Depends on:** thread 1d (pin positions) and the layout rules in thread 4.

### 4. Panel layout intelligence (Pro Panel depth; split 2)
- **Threads:** (a) *Rules-based placement*: power high, drives with clearance, hot devices away from the top, signal and power ducts separated, rail optimiser, cutting list for rails and ducts; (b) *Mounting surfaces and collision*: door, side wall, profiles, mesh-level collision, door swing, wiring holes, multi-plate cabinets.
- **Complete means:** the planner produces a layout a panel builder accepts at a glance, every rule that moved a device is shown, and a violation says which rule.
- **Validation:** curated reference cabinets with hand-checked layouts; property tests that every placement obeys rules.
- **Where:** Cabinet workspace > Place all (with rule set), 3D view.

### 5. Standards compliance engine
- **Complete means:** rule sets for IEC 60204-1 (conductor colours, PE, emergency stop, protection class, control voltage), NFPA 79, and UL 508A (SCCR calculation, wire sizing and spacing rules, component ratings), selectable per project, a compliance report for the file, and waivers with reasons that print.
- **Validation:** clause-by-clause rule table with a test each; sample non-compliant projects; legal note that the tool assists and the engineer certifies.
- **Where:** Checks workspace > Standards, Reports > Compliance.

### 6. Variants and generated engineering
- **Complete means:** parameter fields on macros, project options (a machine with or without a conveyor), a generator that builds pages from a spreadsheet or spec sheet, and a configuration view that shows which pages and parts each option adds.
- **Validation:** golden projects generated from fixtures; option on/off diff produces expected pages and BOM lines; round trip with the spreadsheet engine.
- **Where:** Project > Options, Macros > Generate, Sheets > Generate project.

### 7. Thermal simulation to a cooling decision (already two threads; add validation)
- **Complete means:** steady-state and transient 3D CFD with conjugate heat transfer, fans and filters as boundary conditions, a result the user can compare against the IEC 60890 estimate, and a cooling unit selector that shows margin. Honest labelling: the estimate is a model, CFD is a model with stated error.
- **Validation:** benchmark cases (lid-driven cavity, natural convection in a differentially heated cavity, a heated block with analytic conduction) with published reference values; mesh convergence; energy balance within a stated tolerance.
- **Where:** Cabinet workspace > Heat layer, Thermal workspace.

### 8. Real simulation (split 3)
- **Threads:** (a) *Analog/logic solver*: modified nodal analysis for DC and AC steady state, source, transformer, motor start model, latching fixpoint; (b) *PLC program execution*: ladder and structured text interpreter driving the I/O list; (c) *Fault scenarios and timing*: scenario files, trip curves, timing diagrams and a waveform view.
- **Complete means:** a star-delta starter and a PLC controlled conveyor behave on screen the way they do in the field, including faults and protection trips, with traces.
- **Validation:** textbook circuits with known results (RC, RL, three-phase balanced, motor start current); test vectors for the interpreter against published ladder semantics.
- **Where:** Simulate workspace; Shift+L live view stays.

### 9. PLC engineering link and functional safety (split 2)
- **Threads:** (a) *Export*: symbol tables, I/O lists and hardware config to PLCopen XML, CSV for CODESYS/TIA/Studio 5000 import, fieldbus topology and address plan; (b) *Safety*: ISO 13849 PL calculation (categories, MTTFd, DC, CCF, PFHd) with a safety function editor and report.
- **Validation:** export imports cleanly into a free engineering tool; safety examples from the standard's annexes and the public SISTEMA examples as tests.
- **Where:** PLC workspace > Export, Checks > Safety.

### 10. Migration and interop
- **Complete means:** import of an existing standard (title blocks, symbols from DXF/SVG, macros from public exchange formats), DWG via a free converter workflow, STEP export of the cabinet, AutomationML and PLCopen where a public specification exists, clean Python API for bulk work.
- **Validation:** import fixtures from Tadjiro's own standard folder (kept off the repo), round trip a real project, report lossy parts.
- **Where:** File > Import and Export, `kopercad-cli`.

### 11. Documentation designer and languages
- **Complete means:** a report/form designer with a table builder and live preview, a title block and frame editor, multi-language text with a translation table, language switching on PDF output, label printers, Dutch UI as the first translation.
- **Validation:** golden PDFs per template; switching language changes every string; pixel-diff tests on frames.
- **Where:** Reports workspace > Designer, Project > Languages.

### 12. Workshop and field mode
- **Complete means:** assembly instructions from the layout (order of rails, ducts, devices, wires), a tablet layout for commissioning and punch lists with photos and signatures, as-built redlines that become revisions, and a shop-floor print pack.
- **Validation:** step order tests on fixtures; export and re-import of field results; usability walk-through on a tablet-sized window.
- **Where:** Commissioning workspace, Reports > Assembly.

## 5. Where this leaves the product

- **Strongest today:** schematic authoring speed, cross references, terminals and cables, potentials, revisions and
  compare, workspaces. These are at or above the suites on workflow and below on library depth.
- **Honest soft spots:** part data, engineering calculations, panel-to-workshop output, simulation depth, standards
  rule sets, variants, interop. These are the work of the next days, in parallel.
- **What "special" means in practice:** a designer gives KoperCAD a motor and a supply and gets a validated
  circuit, a layout that respects heat and clearances, wires cut to length, a compliance report and an assembly
  pack, from a library that is full of real parts. No other single tool does that chain. Lanes 1, 2, 3, 4 and 6
  are that chain.

## 6. Validation rules for every lane above

1. Tests against published reference cases are part of the PR, not a follow-up.
2. Anything that is a model says so in the UI and the report text, with the standard and table version named.
3. Unknown input is shown as unknown; no silent defaults in results.
4. Every PR names where the user finds it, and a feature without an entry point is not done.
5. No admin rights, local only, and never EPLAN data or library content.

## 7. Session room: threads that can close

The project is near the cloud concurrent-session limit (about 47). From `list_thread_sessions` at 19:26 UTC,
these threads have `completed` status. **Close a thread only after confirming its PR is merged and its handoff
files are on main.** Ordered by safest first.

**Already resolved but still counted as active sessions (archive the session):**
Data model, Symbol artwork, Project file, PLC and I/O, Pages groups and panels, Terminals and strips,
README and roadmap update, README and roadmap refresh, Live potentials view, Interruption points and PE marks UI,
Wheel fifth wedge, Device models and animation, NFPA symbols and library growth, Autosave and crash recovery,
DXF export menu and CLI, Plate drawing and drill table, Enclosure sizing helper, Terminal diagrams and plug lists.

**Completed and quiet for over an hour, safe once their PR is merged or handed off:**
Canvas engine, Schematic editor, Parts and properties, Cross references, Terminals and terminal screens,
Outputs: PDF, reports and checks, Potentials PE and interruption points, Potential and PE menus, Two wiring modes,
Wire list with cut lengths, EPLAN Pro Panel research and plan (its output is #379), Part power loss and
clearances, Parts library: Import button and browser, Plate drawing: finish and rebase, Lessons: schematic basics,
Lessons: cabinet layout, UI integration: reports and checks, Tester and test reports (the new hands-on tester
replaces it), Project Coordinator (the old one, idle since 12:55 UTC), Release and builds (idle, wake it when a
build is requested).

**Keep running** even though their status says completed, because they changed minutes ago: Enclosure depth and
door swing, 3D view: part geometry import, Design checks panel, Revision tracking and compare, Live view polish.

Freeing the 18 resolved sessions plus about 10 of the completed ones makes room for the 12 briefs above
(about 20 threads once split).

## 8. Next step

The coordinator starts the twelve lanes in parallel, in the order of section 3, with the splits named in section 4.
This document is revised when a lane finishes and the gap closes.
