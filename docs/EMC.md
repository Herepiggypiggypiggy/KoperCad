# EMC in KoperCAD: plan and scope

Status: plan, 2026-10-22. Owner: the EMC lead thread. Siblings: EMC rule engine, EMC visualisation.
Working notes and lane state: `handoff/emc/`. Clause and table numbers in this document are from the
author's knowledge and are **unverified** (no licensed copy was read); each one carries a "verify" mark
in the Source column of the rule catalogue (section 5) and must be checked by a person with the standard before any text claims it.
No standard text is copied here.

## 1. Aim

Electromagnetic compatibility (EMC) is the reason a cabinet that is wired "correctly" still resets its PLC when a
drive starts. Today a designer gets it from experience and from vendor guides. KoperCAD already knows the things
that decide it (which wires are power and which are control, where every wire physically runs, which duct it
shares, where the rails, plate, entry and door are). So the program can do two things that are worth having:

1. **See it.** Show coupling, loop area, shield paths and bonding on the real cabinet, in 2D and 3D, so the
   designer sees the problem the way a measurement engineer would reason about it.
2. **Be driven by it.** Run EMC rules while the cabinet is drawn and laid out, put the findings where the
   designer is working, and offer fixes that go through the normal command path (undo, redo, scripts).

It never certifies compliance. Compliance is shown by measurement on the finished machine. What the program
gives is a design that is much less likely to fail, a documented set of reasons, and a list of what it could not judge.

## 2. What EMC means for a machine control cabinet

Two sides: emission (what the machine puts into the supply and the air) and immunity (what it survives).
Both come from the same few physical mechanisms.

### 2.1 Standards landscape (knowledge only, verify before quoting)

| Topic | Document | Use here |
| --- | --- | --- |
| Generic emission, industrial | EN 61000-6-4 | Frame for what limits apply to equipment in industrial places |
| Generic immunity, industrial | EN 61000-6-2 | Test levels the machine must survive |
| Residential variants | EN 61000-6-3, -6-1 | Used when the machine is sold into those places |
| Adjustable speed drives | EN 61800-3 | Drive categories C1 to C4, filter and cable-length conditions come from the drive maker |
| Emission measurement | CISPR 11 / EN 55011 | Class A or B, group 1 or 2: the language of limits |
| Immunity tests | IEC 61000-4-2 (ESD), -4-3 (radiated RF), -4-4 (EFT/burst), -4-5 (surge), -4-6 (conducted RF) | Each test is a coupling path the design must give a way out |
| Machine electrical equipment | IEC 60204-1 / EN 60204-1 | Has a clause on EMC and on protective bonding; **verify clause numbers** |
| Installation of cabling | IEC 61000-5-2, EN 50174-2 | Cable classes, segregation, shield termination practice |
| Legal frame (EU) | EMC Directive 2014/30/EU, Machinery Directive | The maker's declaration; not something software can give |

### 2.2 The mechanisms that matter in a cabinet

1. **Common impedance and bonding.** The mounting plate is the reference of the cabinet. Parts, rails, filters,
   shield clamps and PE bars must bond to it with low impedance at high frequency: bare metal contact over
   area, short wide straps, no painted surfaces, no long round PE wires as "grounding". A 100 mm round wire
   is about 100 nH, which is several ohms at 10 MHz regardless of its cross-section.
2. **Inductive (magnetic) coupling.** A loop carrying changing current induces voltage in any other loop
   it links. Scales with the mutual inductance of the two circuits, which grows with the area of each loop
   and the length they run parallel, and falls quickly with distance. Fix: small loops (go and return together,
   twisted pairs, wire close to the plate), distance, shields that carry return current.
3. **Capacitive (electric) coupling.** A fast dV/dt conductor couples charge into a nearby high-impedance
   victim. Fix: distance, a grounded shield or segregation plate between, low-impedance victims.
4. **Common-mode current and the drive.** A variable frequency drive switches a DC link with steep edges. The
   motor cable and motor winding have capacitance to earth, so common-mode current leaves the drive, goes
   through the motor frame and returns through the earth and the plate. Fix: a screened motor cable terminated
   360 degrees at both ends, the screen as the return path, output chokes or sine filters where the maker asks,
   an input filter bonded at its case to the plate right at the entry, short cables.
5. **Shield termination.** A shield works by carrying current. A long "pigtail" at the end replaces a shield by a
   wire and loses most of the benefit at high frequency (about 10 nH per centimetre of pigtail; 1 cm is
   0.6 ohm at 10 MHz). 360 degree termination at the gland or on a shield bar, at both ends for noise
   immunity of power and fast signals, is the default; one end only for slow analog signals where low-frequency
   ground loops are the worry. Shield through a terminal block interrupts it; use a shield terminal.
6. **Segregation by cable class.** Group wires by what they do to each other: power and motor (aggressors),
   unfiltered mains, filtered mains, 24 V DC supply, digital control, analog and sensor signals, fieldbus
   and Ethernet, safety circuits. Keep classes in separate ducts, cross at right angles, and keep
   specified distances (manufacturers state many of these; the program already uses a 40 mm default for power and signal ducts).
7. **Filters, chokes, ferrites, suppressors.** Mains filter at the entry, with filtered and unfiltered wire
   on opposite sides and never in the same duct. Ferrite cores on cables that leave the cabinet. RC and
   diode suppression across inductive loads (contactor coils, valves); varistors or TVS on surge paths.
8. **Surge, EFT, ESD paths.** Fast transients must go to the plate at the point where they enter, not travel
   inside. Cable entry plate with shield clamps, surge arrester close to the entry with short leads, shield
   bonded at the gland, door bonded to the frame with a strap, no loose long earth wires on the door.
9. **24 V DC against 400 V AC.** Different voltage domains, different noise: keep 24 V supply and its
   return together (small loop), separate from AC power, and keep the PLC supply away from the drive's output.
10. **Loop area.** The single most useful number. The loop of a circuit is the area between its outgoing
    and return conductor. A signal pair routed far apart across the plate, or returning through the plate
    instead of through its own wire, has a large loop and both radiates and picks up.

## 3. What KoperCAD already knows

All of this exists on main; the plan builds on it and does not duplicate it.

| Knowledge | Where it lives | EMC use |
| --- | --- | --- |
| Circuit class of every net (protective earth, neutral, power, control, unknown), derived from the drawing | `novo-checks` `standards/netclass.rs` | Starting point for cable class |
| Wire colour class (AC power, DC control...) | `novo-reports` `wire_rules.rs` | Cross-check of class |
| Power versus signal device class, 40 mm separation rule and conflict test | `novo-layout` `rules.rs` (`Class`, `SEPARATION_MM`) | The first EMC rule already exists for placement |
| Ducts and rails with position, direction, cross-section and fill | `novo-core` `Rail`, `novo-routing` | Shared-duct and segment geometry |
| Routed path of every wire through ducts, with length basis | `novo-routing` plan, cable runs in 3D (handoff `cable-3d`) | Parallel run length, distance, loop geometry |
| Cable cores with a `shield` flag, cable length | `novo-core` `Cable`, `CableCore`; `novo-cables` screens | Shield presence |
| Cabinet entry point, door, plate, clearance zones | `novo-layout`, `novo-view3d` | Entry-side rules, door bonding |
| 3D cabinet view with overlays, layers, section plane and probes | `novo-view3d` | Where the EMC view is drawn |
| Contour, hot spot and streamline engine for a scalar field | `novo-heatview` | Reuse for a field-estimate slice |
| Standards report with provenance and "not checked" listing | `novo-checks` `standards/` | Where EMC findings and caveats are reported |
| EMC filter symbol | `iec/transformers/emc_filter` in `novo-editor` catalog | A filter can be placed; nothing reads it yet |
| Heat and airflow models with honest labels and validation file | `novo-thermal`, `novo-cfd`, `docs/thermal-validation.md` | Template for how to state model accuracy |

What it does **not** know today, and the plan has to add (kept minimal, and through the `Properties` bags first
so that no `novo-core` public API change is needed before the WS-02 owner agrees):

- An explicit **EMC class** per wire and cable (derived by default, overridable), and per device (aggressor,
  victim, neutral) with a source such as "VFD output", "switch-mode supply", "analog sensor".
- **Shield termination** at each end of a cable: gland 360 degrees, shield clamp on bar, pigtail with
  length, unconnected.
- **Bonding facts**: which parts and rails bond to the plate, which surfaces are bare or painted, strap
  lengths for PE and for the door.
- **Filter facts**: the filter's line and load side, its case bond, its attenuation class from the part data.
- **Part EMC data**: drive category, maker's maximum motor cable length with and without filter, required
  accessories (these come from the part catalogue, `novo-parts`, never invented).

## 4. What can be computed honestly

Each result carries one of five levels, shown next to the number in the program. A result never shows a
higher level than the method behind it.

| Level | What it is | Examples | Label in the product |
| --- | --- | --- | --- |
| 0 Fact | A rule on what is drawn | Shielded motor cable has no 360 degree termination; filtered and unfiltered wire share a duct; drive has no filter | "Rule" |
| 1 Geometric index | A number from geometry only, no physics claim beyond ordering | Parallel run length of aggressor and victim, closest distance, shared duct length, loop area of a circuit, pigtail length | "Index" |
| 2 Analytic estimate | A closed-form formula with stated assumptions | Mutual inductance of two parallel wires over a plane; induced voltage for a given dI/dt; capacitive coupling; inductance of a bonding strap; shield pigtail impedance at a frequency | "Estimate" with assumptions and a range |
| 3 Quasi-static field | Biot-Savart (and image method for the plate) from the routed conductor paths and stated currents, sampled on a slice or along a wire | B field map in the cabinet from a motor cable at a stated current and frequency; field at a PLC | "Model, low-frequency near field" |
| 4 Not computed | Needs a full-wave solver, a measurement or a lab | Radiated emission margin, cavity resonances of the enclosure, slot and seam leakage at high frequency, real filter insertion loss, compliance | Always listed in "Not checked" |

Honesty rules for the lane:

- Level 2 and 3 are validated against closed forms (Neumann mutual inductance, Grover tables for wire
  inductance, Biot-Savart of a finite straight segment, field of a rectangular loop on its axis, image method
  for a plane) in tests that print their error like `docs/thermal-validation.md`. Published in `docs/emc-validation.md`.
- Inputs that the program does not know (current, dI/dt, frequency band) are asked for or defaulted **visibly**
  ("assumed 10 A, 2 kV/us; edit"), never hidden.
- Every report page says that it is a design aid, not a test result, and lists level 4 items as not checked.
- Frequency is explicit. Below roughly 30 MHz the cabinet is electrically small and the quasi-static
  models are defensible. Above that, cable lengths approach a wavelength fraction and the program says
  it has no model, instead of drawing a pretty field.

## 5. Rule catalogue (first cut)

The rule engine lane turns this into code. Every rule has an id, a plain-language reason, the standard or
practice it comes from (the Source column; a named clause is always marked verify, and "no clause" means practice only), the level of its method, and a fix proposal.

| Id | Rule | Level | Needs | Source (verify before quoting) |
| --- | --- | --- | --- | --- |
| EMC-01 | Power and signal wires do not share a duct | 0 | Routed paths, class | IEC 61000-5-2 / EN 50174-2 segregation practice (verify clause) |
| EMC-02 | Parallel run of aggressor and victim shorter than a limit at the distance set | 1 | Segment geometry | Practice, maker guides; no clause |
| EMC-03 | Crossings of different classes are close to 90 degrees | 1 | Segment geometry | Practice, maker guides; no clause |
| EMC-04 | Motor cable of a drive is shielded, shield on a termination that is 360 degrees at both ends | 0 | Cable, shield data | EN 61800-3 and drive maker manual (verify) |
| EMC-05 | Shield pigtail longer than a limit (length in mm shown, impedance at 10 MHz estimated) | 2 | Shield termination | Practice; physics of pigtail inductance; no clause |
| EMC-06 | Shield is carried through terminals without a shield terminal | 0 | Terminal strip, cable | Practice, maker guides; no clause |
| EMC-07 | Drive has an input filter, filter case bonded to plate, filter near the entry, filtered side not parallel to unfiltered side | 0/1 | Part data, layout | EN 61800-3 and filter maker data (verify) |
| EMC-08 | Motor cable length within the maker's limit for the filter chosen | 0 | Part data | Drive maker data sheet; no clause |
| EMC-09 | Loop area of a circuit above a limit (go and return not together; power pair, 24 V pair, signal pair) | 1 | Paths of both conductors | Practice; physics of loop coupling; no clause |
| EMC-10 | Return through the plate instead of its own conductor for a fast or analog circuit | 1 | Netlist, paths | Practice; no clause |
| EMC-11 | Mounting plate, rails and PE bar bonding: bare contact, strap length, painted surface flagged | 0/2 | Bonding facts | IEC 60204-1 protective bonding / EMC clauses (verify clause numbers) |
| EMC-12 | Door bonded to the frame with a short strap; no devices with fast edges on the door without a filtered path | 0 | Cabinet, door | IEC 60204-1 (verify); IEC 61000-4-2 ESD path |
| EMC-13 | Inductive loads (coils, valves) have suppression where the family needs it | 0 | Symbols, parts | Practice, relay and valve maker data; no clause |
| EMC-14 | Fieldbus or analog cable leaving the cabinet is shielded and bonded at the entry | 0 | Cable, entry | IEC 61000-5-2 / EN 50174-2 (verify clause); fieldbus installation guides |
| EMC-15 | 24 V DC supply and return routed together; supply not near an AC aggressor | 1 | Paths | Practice; no clause |
| EMC-16 | Surge protection at the entry with short leads | 1 | Layout | IEC 61000-4-5 surge path; arrester maker data (verify) |
| EMC-17 | Sensitive part (PLC, analog module, encoder interface) distance to aggressor | 1/2 | Layout | Practice, device maker guides; no clause |
| EMC-18 | Ferrite suggested on cables leaving the cabinet where a drive or switching supply shares the cabinet (advice, not a finding) | 0 | Cable, parts | Practice; advice only; no clause |

Findings use the existing problem model (`novo-checks`, severity and location) so they appear in the Checks
panel, in the standards report and in the handover package without new plumbing.

## 6. Visualisation

The main idea: show the cabinet as the **coupling picture** a good EMC engineer builds in their head.

1. **Zone view.** The plate and 3D cabinet coloured by EMC zone: noisy (drive, switch-mode supply, contactors),
   quiet (PLC, analog, encoder), and neutral. Zones come from device classes; the designer sees at a glance if
   the quiet module sits beside the drive.
2. **Wire class view.** Every wire run drawn in its class colour (a palette that holds in the dark theme and
   for colour blindness, `ui_palette()`), with the filtered/unfiltered split and shielded cables shown as a sheathed run.
3. **Coupling pairs.** Where an aggressor and a victim run parallel, a ribbon between them whose width is the
   run length and whose colour is the coupling index; hover gives the numbers and the assumptions.
4. **Loop sheets.** For a chosen circuit, the loop area is shown as a translucent sheet between go and return
   conductors with the area in cm2, so a big loop is obvious and shrinks as the designer moves a wire.
5. **Shield and bonding graph.** The path of return current: shield to gland to plate to filter case to PE bar.
   Weak points (pigtail, painted contact, long strap) glow; the impedance estimate at the chosen frequency is on the card.
6. **Field slice.** A section plane through the cabinet showing the estimated low-frequency magnetic field
   of the conductors at their stated currents (level 3), with contours and the sensitive parts marked.
   Rendered through `novo-heatview`; the legend says "model, near field below 30 MHz".
7. **What-if.** Move a duct, split a duct, add a segregation plate, add a ferrite, shorten a pigtail, and see
   the index, the finding and the field change live (the same commands the designer would apply).
8. **2D mirror.** The Cabinet plate view and the schematic carry the same colouring and finding markers so the
   result is not only a 3D toy: the schematic shows class per wire; the plate shows zones and coupling pairs.
9. **EMC card per cable and per drive.** One card: class, shield state, length, coupling partners, loop area,
   bonding path, findings, not checked.

Where users find it (each PR repeats this): View menu > EMC layer in the Cabinet and 3D workspaces; Check menu
> EMC check; EMC findings inside the Checks panel; Reports menu > EMC summary; Learn > EMC track.

## 7. EMC-driven design

Beyond showing problems, the program steers while the designer works:

- **At placement.** Dragging a part in the Cabinet shows the live distance to aggressors and to quiet parts
  (like the clearance zones that exist for heat) and warns before the drop.
- **At routing.** The router gets EMC costs: it prefers the duct of the right class, avoids parallel runs with
  aggressors, and keeps pairs together. Wire routing is `novo-routing`; EMC adds a cost term through a rule
  hook, not a fork.
- **At wiring.** When a drive is placed, the program offers the usual set as one macro: filter, output choke
  where needed, shielded cable with shield clamps, PE bar, with the maker's data when the part has it.
- **At entry.** The cabinet entry plate gets a recommended gland and shield bar set per cable from the
  cables that cross it.
- **Autopilot.** The existing Autopilot can fix findings that have a mechanical fix (reroute, change duct,
  add shield clamp, add suppressor) as a reviewed batch.
- **Report.** An EMC design record in the handover package: classes, zones, rules run, results, assumptions,
  and the Not checked list. Never a compliance statement.

## 8. Phases and lanes

Roadmap detail and live state: `handoff/emc/ROADMAP.md`.

| Phase | Content | Lane | Gate |
| --- | --- | --- | --- |
| E0 | This plan merged; the rule engine thread asks the coordinator for `novo-emc` and opens the registration PR first (asked 2026-10-22); data decisions: Properties first | Lead | Plan merged |
| E1 | EMC classes and shield/bonding facts in the model (properties first); classification from netlist with overrides; rules EMC-01 to EMC-06 on the existing routed paths; findings in Checks panel; Zone and Class views in Cabinet and 3D | Rule engine + Visualisation | Example project shows real findings and the overlay at 1366x768 and 1920x1080 |
| E2 | Geometry indices (parallel length, distance, crossing angle, loop area); EMC-07 to EMC-12; coupling ribbons and loop sheets; EMC card; report page | Rule engine + Visualisation | Indices validated against hand-computed cases |
| E3 | Level 2 analytic estimates with ranges (mutual inductance, induced voltage, pigtail impedance, strap inductance); bonding graph view; part EMC data in catalogue; drive macro | Rule engine + Visualisation + Parts | `docs/emc-validation.md` with printed errors |
| E4 | Level 3 quasi-static field from routed paths (Biot-Savart plus plane image), field slice through `novo-heatview`, what-if; router EMC cost; Autopilot fixes | A third thread (field) once E2 lands | Field validated against closed forms; slice renders in 3D |
| E5 | Lesson track and manual page, EMC record in handover, standards report section, polish | Lead | Lessons replay in CI; manual reviewed |

Dependencies: E1 needs a decision with the WS-02 owner on stored fields (start with `Properties`, promote to
typed fields later); `novo-emc` must be registered by the coordinator in its own small PR (the crate list is
fixed); the visualisation lane lives in `novo-view3d` and `novo-app`, reads findings and indices only through
the `novo-emc` query API, and does no physics itself.

Crate split:

- `novo-emc` (new, pure, no UI): classification, indices, estimates, rules, field model. Depends on `novo-core`,
  `novo-routing`, `novo-layout`; readable by `novo-checks` and `novo-reports`.
- `novo-view3d` / `novo-app`: layers, overlays, cards. Thin.
- `novo-learn`: the EMC lessons (lead).

## 9. Risks and answers

| Risk | Answer |
| --- | --- |
| Users read a green EMC view as compliance | Every card and report says design aid; Not checked list is always shown; wording reviewed by the lead |
| Wrong class guessed from the netlist | Class is derived, shown with its reason, and overridable per wire/cable/device |
| Estimates quoted past their validity | Frequency band and assumptions are on every estimate; outside the band the program refuses |
| Rule thresholds taken as law | Thresholds are project settings with the maker's or house value; defaults come from common practice and are labelled |
| Standards clause numbers wrong | Verify marks stay until a person with the licensed text confirms; no text copied |
| Heavy UI load | Indices are cheap; field slices run in a worker with the same governor as the heat view |

## 10. What the lead keeps

The lead thread owns this document, `handoff/emc/`, the lesson track and the manual page, the validation
document, the wording of every honesty label, and the review of both sibling lanes against this plan.
