# Drives, motors and a crane in 3D: audit and plan

Date 2026-10-28.
Status: audit done against `main` at commit 64b31c2 (v0.1.0-rc10 line). Plan not started.

## Part 1. Audit: what KoperCAD can check today

Short answer: today the program can tell you that a motor is fed, which way it turns, and
how a *line-started* induction motor accelerates (direct, star-delta, auto-transformer, soft
start). It cannot check anything about a frequency drive, a motor on a drive, rpm under a
frequency reference, a hoist load, or a moving crane. Nothing is stored in a project for any
of it. The pieces we can build on are real and good, so the plan below is an extension, not a
rewrite.

| Question | Today | Where it lives |
| --- | --- | --- |
| Is the motor fed, which direction, phase loss, two-speed (Dahlander) | Yes, a derived state per motor in the live view (Shift+L): Stopped, Running(direction, speed), PhaseLoss, Fault. Direction comes from the phase order. | `novo-sim/src/devices.rs` (`MotorRun`), design `docs/design/device-animation.md` |
| Does a contactor, relay, timer, seal-in circuit work | Yes: settled step by step, timers on a clock, contacts follow coils. | `novo-sim/src/coupling.rs`, `timing.rs` |
| PLC program effect on outputs | Yes: IEC 61131-3 Structured Text and ladder interpreter with a scan cycle, timers, counters, `%QW` words, a bench that maps program I/O to the drawing. | `novo-sim/src/program/`, `novo-plc` |
| Motor start (current, speed, time) on the mains | Yes, as a model. Per-phase equivalent circuit, `J dw/dt = Te - Tl`, constant, quadratic and breakaway loads, direct/star-delta/auto-transformer/soft start, weak supply voltage dip, stall. Nameplate fit from kW, rpm, current, power factor. Tested against textbook Thevenin torque. | `novo-sim/src/analog/motor.rs`, tests `crates/novo-sim/tests/motor.rs`; UI: Simulate, "Start..." button per motor in `novo-app/src/app/simulate.rs` |
| Steady-state volts, amps, kW on loads and cables | Yes, linear model with ideal conductors. | `novo-sim/src/analog/`, `network.rs` |
| Fault levels, protective device curves, selectivity | Yes. | `novo-sim` study/protection, `novo-checks` |
| Frequency drive (VFD) | **No behaviour.** The symbol `iec/drives/frequency_converter` exists (L1 L2 L3 PE in, U V W out, `behavior kind = "converter"`) and the starter macro "Frequency converter drive" places it. The simulator has no converter model: it does not know frequency, ramps, V/f, current limit, overload, DC bus, braking, control terminals or parameters. EMC rules use it as a noise source tag only. | symbols in `novo-symbols/symbols/*/drives/`, `novo-parts/src/macros/starter/more.rs` |
| Motor speed in rpm under a drive | **No.** `Motor` knows `sync_rpm()` and rated rpm at fixed 50/60 Hz only. | `motor.rs` |
| Hoist, gravity load, brake logic, overhauling load, regeneration | **No.** `Load` has no sign, no gravity, no four-quadrant operation. | `motor.rs` |
| Drive/motor data in the parts catalog | Partly. Parts carry free key/value technical data with SI units (`rated_current_a` and others) and maker part-number checks exist. No drive or motor schema (rated power, rated rpm, poles, inertia, drive output current, overload class, braking resistor minimum ohm). Real data must be entered or imported by the user. | `novo-parts/src/schema.rs`, `recipes/makers.rs` |
| 3D | The 3D view is the **cabinet** (plates, rails, ducts, devices, wires, heat/EMC overlays). Only ambient motion: fan blades and air pulses. No machine or crane scene, no kinematics, no scene driven by the simulator. | `novo-view3d` (`motion.rs`, `scene.rs`) |
| Commissioning | A pure engine for checks, progress and punch list. It has a `Torque` check kind (a terminal torque check), nothing about drive parameters or motor direction/speed tests. | `novo-commissioning` |
| Python/CLI access to simulation | Not for drives. | `novo-py`, `novo-cli` |

What this means in practice:

- "Can I check motor speed and rpm under a drive?" Not yet. You can see the drive drawn, and
  a motor shown as running if it is fed, but no frequency, no rpm, no ramp.
- "Can I see the crane move?" Not yet. There is no machine in 3D, and nothing links a simulated
  motor to a moving part.

Reusable foundations: the settle loop (contacts and coils), the PLC interpreter and bench,
the analog solver, the induction-motor equivalent circuit (valid at any frequency if scaled,
see below), the part catalog, `novo-view3d` render stack and camera, the live view animation
layer, the commissioning engine, the Python layer.

## Part 2. The plan

### Principles

1. Model, not magic. Every number in the UI is labelled *model* or *exact by definition*
   (for example `n_sync = 120 f / p` is exact; slip at load, current at current limit, DC bus
   ripple, thermal state are models). A visible "What this model leaves out" panel per
   device, same style as the motor start estimate.
2. No manufacturer content. No manufacturer manual text, parameter tables, parameter codes or
   macro tables are copied. Only publicly known general behaviour (V/f, ramps, current
   limit, DC bus, braking chopper, I2t overload). A **drive profile** is a data file the user
   enters or imports (CSV/JSON/XLSX from their own documents or from a configuration export
   they own). Generic neutral names inside the program (`frequency_reference`,
   `acceleration_time_s`), with a per-profile alias column so the user can map their own
   parameter codes. Starter profiles are generic ("Generic VFD, 400 V, 7.5 kW"), never
   pretending to be a specific manufacturer's drive.
3. Driven by the drawing. The drive is controlled by what is actually wired: run
   command from a terminal fed by a contact, a PLC output, or an analog output word; the
   motor is the one connected to U V W; the brake coil, limit switches and E-stop are the
   ones in the schematic.
4. Validated. Each engine ships with hand-calculated cases (below), written before the code.
5. Workshop and Windows: pure Rust, no network, no admin rights, nothing written outside the
   user's profile. All data files plain text and shipped in the repository.

### Physics scope (ambitious, in layers)

Layer A, drive.
- Frequency reference sources: panel, analog input (0-10 V, 4-20 mA scaling), PLC word,
  preset speeds, jog, motor-potentiometer up/down; min/max frequency, skip frequencies.
- Ramps: linear and S-ramp, separate accel/decel, fast stop, ramp stretch to avoid DC bus
  overvoltage. Exact: ramp arithmetic.
- Control modes: V/f constant torque, V/f quadratic (pump), V/f with boost, base-frequency
  and field weakening above base (constant power); sensorless vector (steady-state
  dq model with slip compensation); closed-loop vector with an encoder for hoists (speed
  loop PI with a mechanical plant). Switching waveform is not simulated; the drive is an
  averaged model.
- Limits and protection: current limit (reduces frequency or stops ramp), motor thermal
  I2t with a user-entered overload class, drive thermal derating by switching frequency
  and ambient (user data), overvoltage and undervoltage trips, phase loss, output short.
  Trips stored with codes in a neutral list, user-aliased.
- DC bus: capacitance, rectifier, charge, regeneration raises the bus, braking chopper
  threshold and braking resistor (R, P continuous, duty cycle); energy balance per stop;
  regen to mains option flag for regenerative front ends.
- Terminals and macros: digital inputs (source/sink, logic), analog inputs, relay and
  logic outputs, fault relay; a "macro" is a named bundle of terminal functions
  (start/stop 2-wire, 3-wire, forward/reverse, preset speeds, hoisting) stored in the
  drive profile, applied to the symbol's pins.
- Mains side: drive input current from output power, efficiency and power factor (user
  data or a stated generic value), harmonic current spectrum is out of scope for v1 and
  stated as such.

Layer B, motor and load.
- Induction motor at variable frequency: reuse and extend the existing equivalent circuit
  with reactances scaled by f and voltage from V/f; torque-speed curve for any (f, V);
  rated torque, slip, rpm, current, power factor, efficiency; field weakening region and
  breakdown limit. Iron loss and saturation as optional user-entered parameters.
- Cooling derating of self-ventilated motors at low speed (stated as a model).
- Mechanics: inertia of rotor, gearbox ratio and efficiency, drum/wheel radius, load
  inertia reflected to the motor shaft, viscous and Coulomb friction, backlash off by
  default.
- Hoist load: gravity torque `m g r / (i eta)` with correct sign, hoisting versus lowering
  (overhauling load, regenerating motor), rope/hook mass, payload, slack rope optional.
- Brake logic: holding brake as a spring-applied brake with coil release time and
  apply time, torque rating, slip when overloaded, brake-open conditions from the drive
  relay output in the schematic; "brake closed while drive runs" and "brake opens before
  torque is built" are reported as findings (the classic hoist drop).
- Travel drives (trolley, bridge): friction, wind load (user value), acceleration limits,
  skew optional later.

Layer C, crane kinematics (3D).
- Crane kinds, in this order of delivery: single-girder overhead bridge crane with hoist,
  trolley and bridge; then double girder; gantry; jib.
- Axes: bridge position, trolley position, hoist height, optional hook swing as a
  pendulum with a stated model (length `l`, small-angle plus nonlinear option).
- End limits and slow-down limits as virtual limit switches whose positions the user sets
  in the crane model; each switch maps to a real symbol/terminal in the schematic through
  a binding table (so opening the NC limit contact in the drawing really cuts the drive
  enable).
- Interlocks and safety: E-stop, pendant, overload (load cell) limit, upper hoist
  limit, final limit that trips the main contactor, anti-collision zone between two
  cranes (later).
- Pendant/radio commands become stimulus pushbuttons you click in the live view.

Layer D, co-simulation loop (fixed timestep, e.g. 1 ms to 10 ms, adaptive off by default
so it is deterministic and testable):

```
schematic logic (settle) -> drive control terminals -> drive state (f, V, trips)
  -> motor torque -> mechanics (hoist, trolley, bridge) -> sensors (limit switches,
  encoder, load cell) -> back into the schematic logic and PLC inputs
```

PLC program scan is called inside the same loop with the drive's analog/digital I/O as
the bench map (the `Bench`/`ClosedLoop` pieces in `novo-sim/src/program/bench.rs`).

### Model versus exact, validation

Written in the app and in this plan: exact by definition = synchronous speed, ramp
arithmetic, kinematic geometry, unit conversions, energy balance (a conservation test).
Model = slip under load (equivalent circuit), current at low frequency (boost and stator
drop), thermal state, brake behaviour, pendulum, DC bus. No claim of manufacturer
accuracy or any compliance statement (no SIL/PL claim, same stance as the EMC and safety
crates).

Hand-calculated validation cases (tests written first):
1. `n_sync = 120 f/p`: 4-pole at 25 Hz is 750 rpm; slip from the nameplate gives rated rpm
   scaled; reference to rpm table for 2, 4, 6, 8 poles at 10, 25, 50, 87 Hz.
2. Ramp: 0 to 50 Hz in 5 s reaches 25 Hz at 2.5 s; S-ramp end points and jerk limit.
3. V/f: at the corner frequency V equals rated V, constant volts per hertz below, constant
   voltage above, and flux falls with f above base (torque capability falls as 1/f).
4. Torque-speed: breakdown torque at rated V/f equals the closed-form Thevenin value
   (existing test pattern) and scales with (V/f)^2 within the constant-flux region.
5. Hoist: 5 t payload, 0.2 m drum radius, i = 31.5, eta = 0.9: motor torque by hand;
   lowering sign flips; at standstill the brake must hold: torque held/brake torque.
6. Brake drop: brake opens 150 ms before torque rises: load falls
   `0.5 g t^2` with t = delay minus torque-build time (computed by hand).
7. Braking resistor: kinetic energy `0.5 J w^2` stopped in `t` gives average power and
   peak; the resistor is flagged when above the entered continuous rating or when
   `V_bus^2 / R` exceeds the drive's allowed current.
8. DC bus energy balance: input minus losses equals mechanical plus stored change, within
   a stated tolerance, in every run (property test).
9. Current limit: with a current limit at 150 %, the time to accelerate an inertia equals
   `J dw / (T_available)`, a hand integral.
10. I2t trip: the time to trip at 150 % for 60 s class equals the curve value entered by
    the user.
11. Pendulum: period `2 pi sqrt(l/g)` for small angles (l = 4 m gives 4.01 s).
12. End-to-end: the schematic's raise button + upper limit NC contact + drive enable
    gives exactly the stop height of the entered limit (within one sample), and removing
    the NC contact in the drawing makes the check fail.
Plus a fixture crane project in `qa/fixtures/` and QA scenarios in the harness
(`scripts/qa.sh`) with screenshots at 1366x768 and 1920x1080.

### Where the user finds each feature

| Feature | Place in the app |
| --- | --- |
| Drive properties and parameters | Property sheet of a drive device: "Drive" section; Parts tab: "Drive profile..." |
| Drive profile import/edit | File > Import and export > Drive profile (CSV/JSON/XLSX); Parts drawer |
| Live drive behaviour (frequency, rpm, current, trip) | Live view (Shift+L): badge on the drive and the motor, panel "Drive and motor" |
| Start/stop, ramps, rpm plot | Simulate menu > "Drive run..." and a new scope panel (frequency, rpm, torque, current, DC bus, brake) |
| Motor and load setup | Property sheet of the motor: "Mechanical load" section (inertia, gear, hoist, friction) |
| Torque-speed curve with working point | Simulate > "Motor curve..." |
| Crane model (axes, travel, limits, masses) | New workspace "Machine" (next to Cabinet and 3D) and Draw/Insert > "Machine from template" |
| Crane in 3D, moving | Machine workspace: 3D view with the pendant buttons; also the live view side panel |
| Binding limit switches/sensors to schematic symbols | Machine workspace > "Bindings" table with jump to the symbol |
| Findings (brake, resistor, current limit, interlock) | Check menu (design checks) and Commissioning list |
| Drive commissioning sheet (parameters to set, direction and speed test) | Commissioning workspace and printable bench sheet |
| Scripting | Python: `novoelectric.drive`, CLI `kopercad-cli simulate-drive` |

### Build lanes

New crates need the coordinator's small PR first (CLAUDE.md rule). Proposed:
`novo-drive` (drive, motor-at-frequency, mechanics, brake; pure, depends on `novo-core`
and `novo-sim`'s motor model) and `novo-machine` (crane kinematics, bindings, co-simulation
loop; pure, depends on `novo-drive`, `novo-sim`, `novo-core`). Everything is a pure engine
with the UI thin; `novo-sim` stays the logic simulator and only exposes the hooks listed in
lane L5.

| Lane | Owner thread (to start) | Scope | Depends on |
| --- | --- | --- | --- |
| L0 Crate PR | Coordinator | Register `novo-drive` and `novo-machine` in the workspace list | none |
| L1 Drive engine | "Drive engine" | Reference sources, ramps, V/f, boost, field weakening, current limit, I2t, DC bus, braking chopper, trips, terminal/macro model, generic profile format, tests 2,3,7,8,9,10 | L0 |
| L2 Motor at frequency and mechanics | "Motor and load" | Variable-frequency torque-speed (extend existing equivalent circuit, vector steady state, closed-loop speed loop), derating, inertia/gear/friction, hoist gravity, brake, tests 1,4,5,6 | L0 |
| L3 Drive profile and parts data | "Drive data" | Drive/motor schema in the catalog, profile import/export, alias mapping to the user's own parameter codes, part-number helper reuse, property sheet sections, no vendor tables copied | L1 format draft |
| L4 Crane kinematics and 3D | "Crane 3D" | Machine workspace, crane templates, axes, limits, pendulum, rendering and camera in `novo-view3d`, bindings table, QA screenshots | L0, L2 for motion input |
| L5 Co-simulation integration | "Electromechanical loop" | Fixed-step loop joining schematic settle, PLC bench, drive, motor, mechanics, sensors; hooks in `novo-sim` (a drive treated as a controlled source with U V W instead of an inert symbol; analog outputs as drive reference); determinism and the end-to-end test 12 | L1, L2 |
| L6 Scope, checks, commissioning, reports | "Drive scope and checks" | Plot panel, findings in `novo-checks` (brake sequence, resistor sizing, ramp versus DC bus, cable length versus drive), commissioning drive sheet, report page, Python/CLI | L1, L2, L5 |
| L7 Independent validation | "Drive validation" | Writes and owns the hand-calculation suite and fixtures first, reviews every other lane's numbers, keeps the model-vs-exact labels honest | start with L1 |

Order: L0 first, L1, L2 and L7 in parallel, then L3 and L5, then L4 and L6. A first
demonstrable slice (single hoist: PLC or pushbutton -> drive -> motor -> brake -> hook rises
and lowers in 3D, end limit stops it) needs L1, L2, L4 (hoist only) and L5.

### Open questions

Which drive families, crane types and drive-to-motor layouts ship first is decided with the
users' own project data, which is kept outside this repository. Until then the plan stays
generic: a drive profile is user data, templates start with a single-girder overhead crane,
and no manufacturer behaviour is assumed.

### Honest limits stated up front

- The first version is an averaged drive model: no PWM, no harmonics, no motor
  saturation unless the user enters it, no real firmware behaviour.
- Matching a real drive's exact behaviour requires the user's own parameter data;
  without it the program is a physics check, not a replica.
- Hook swing and rope stretch are simplified models.
- Nothing here is a safety or compliance verdict.

### Lane L2a as built (motor and load)

`novo-drive` now holds the two-speed hoist engine: `motor` (Kloss torque-speed curve fitted to the
nameplate, pole-change motor with fine and coarse winding), `sequencer` (contactor sequencing with
dead times), `brake` (rectifier-fed spring-applied brake, AC-side versus DC-side apply delay),
`mechanics` (hoist gravity, direction-dependent losses, stiction, reflected inertia) and
`hoist_unit` (all of it stepped as one unit for the co-simulation loop).

Efficiency convention: `eta` is the loss in the drivetrain and acts per direction. Raising at
constant speed needs `m g r / (i eta)`; lowering at constant speed the motor still holds back
`m g r eta / i`; at standstill the weight itself is `m g r / i` (the figure a holding brake is
checked against) and the losses only decide whether the load starts to move.

Exact by definition: `n_sync = 120 f / p`, the arithmetic of gravity and reduction. Model: the
torque-speed curve (Kloss fit), one efficiency constant per direction, brake delays entered by the
user, contactor dead times. Not modelled: stator current, saturation, winding heating, changeover
torque spikes, rope stretch. Inputs that cannot describe a machine (zero poles, zero ratio or
efficiency, negative length, NaN) return `InputError`.

### Lane L4 (hoist half) as built: the hook in 3D

`novo-view3d::crane` builds a scene of a generic single-girder crane from plain numbers; the
`Hoist in 3D` window (Simulate > Hoist in 3D, `sim.hoist3d`) feeds it the hook height, the hoist's
warnings and the heights of the bound limit switches from the Hoist run loop. Exact: the block
stands at the start height plus the simulated height, a limit flag stands at its bound height.
Picture only: the crane sizes and the straight rope (no stretch or slack). Listed in the window as
"Picture, not measurement".

Travel binding: the window picks a travel axis of Simulate > Drive run for the bridge and one for
the trolley ("Bridge driven by", "Trolley driven by"); the axis position (forward positive, along
+x for the bridge and +z for the trolley) is added to a typed start position and held at the ends
of the runway and the span with a note. The hook swings as one pendulum per direction
(`novo_machine::swing`): exact is the equation of a point mass on a rope hung from an accelerating
carriage, the small-angle period `2 pi sqrt(L / g)` and the lean `atan(a / g)` at constant
acceleration; the damping ratio (2 %) is a number, the rope keeps its length, the swing does not
push back on the drive, and the two directions do not couple.

Saved setup: the axis picks, the crane size (runway, span, rail height, hook start, load) and the
start positions are kept in the project info properties (`crane3d.*`, `app/crane_setup.rs`), one
undo step ("Crane picture") once the mouse button is up. A project without them gets the defaults;
a number that is missing or out of range falls back one by one, and an axis the project no longer
has is dropped. Undo and opening another project reload the window from the project.

Travel end limits: the forward and reverse end limit of a picked axis (the `trav_limit_fwd_m` and
`trav_limit_rev_m` set on its motor, metres from the axis start) stand as upright flags, red for
forward and amber for reverse: on the runway beam for the bridge, on the girder for the trolley, at
the typed start plus the distance. They are read from the project, so they show before a run; a
limit that falls outside the runway or the span is not drawn and the window says how many. The
flag shows where the limit is set, not whether the switch is pressed.

### Lane L2c-open as built (open-loop drive hoist)

A hoist behind a frequency drive without an encoder: `novo_drive::hoist_logic` (the brake sequence and
load sensing, pure) and `novo_machine::drive_hoist_loop` / `drive_hoist_spec` (the loop and the `dh_*`
property keys). It reuses the drive (`Drive`), the induction motor, `Hoist` and `SpringBrake`; nothing in
the shared motor files changed.

Sequence (generic, every number is an entered parameter): magnetise at the release frequency, open the
brake relay only when the motor current reaches the release current, wait the release time (and the
brake feedback if one is wired), run the chosen speed step, on stop come down to the engage frequency,
open the relay, keep the output on for the engage time, then switch off. A trip or lost mains closes the
relay at once. A direction change is a full stop and start, unless the jump is set (below). One to four speed steps; the k-th speed input
selects step k.

The drive's brake relay is a contact in the drawing (`dh_brake_relay`): the loop writes it like a limit
switch, so the brake contactor, the rectifier and the coil are real circuit parts (`dh_brake_feed`). A
contactor that does not pull in leaves the brake closed. The brake follows its coil through
`SpringBrake` (AC or DC switching, delays and torque rise). Without a feed device the brake follows the
relay directly and the window says so.

Jump at reversal (optional, `dh_jump_hz`, `dh_jump_ramp_s`): with the key set, a direction change keeps the
brake open and only flips the run direction; inside a band of +-`dh_jump_hz` around zero the drive uses the
short ramp `dh_jump_ramp_s` instead of its normal ramps. The band must lie below the slowest speed step (refused otherwise), because the reversal ends when the frequency rises above it. A slow pass through zero lets a hanging load sink,
so the run reports the sag as "ReversalSag" (metres below the height at the reversal).

Slow-down before the final limit (optional, `dh_slow_up`/`dh_slow_up_m`, `dh_slow_down`/`dh_slow_down_m`,
`dh_clearance_up_m`/`dh_clearance_down_m`): a slow-down switch is a second limit contact in the drawing,
placed before the final limit (the property sheet notes it when it is not). The loop only writes the
contact by hook height; what it cuts is the drawing's business (for example the relays of the fast speed
steps, so the drive falls back to step 1 and the final limit is reached at creep speed). The run reports
the speed at the slow-down switch ("SlowDownPassed"), the speed at the final limit with the speed at the
slow-down switch beside it ("FinalLimit"), how far the hook went past the final limit before it stood
still ("LimitOvertravel") and, with a clearance entered, the speed at which it would hit the stop behind
it ("HitStop"). Exact by definition: the limit contacts follow the hook height, the overtravel is the
hook height past the actuation point. Model: the stopping behaviour (all of the drive, brake and sequence
models above). The stop or buffer itself is not modelled (no impact, no rope shock): the run only says
at what speed the hook would reach it.
Heat over a duty cycle (`dh_heat_*`, `dh_brake_*` rows, always on): the module carries the two-node
winding and frame model of `novo_drive::hoist_thermal` (losses from the fitted motor at the drive's
output, a fan that cools in proportion to shaft speed, so a long slow run at low frequency heats more
than the same work at speed) and `novo_drive::brake_heat` (the brake dissipates torque x speed while it
slips, one heat capacity and cooling constant, wear as the share of a lining life in joules). The run
reports "MotorTooHot" (with the standstill time to cool back to 80 % of the limit), "BrakeHot" and
"BrakeWear" (50 % and 100 %). `novo_machine::duty_cycle` repeats a lift, pause, lower, pause cycle
faster than real time on an ideal operator and wiring (not the drawing) and returns peaks, starts per
hour, the first time the winding hit its limit and a curve; Simulate > Drive run > "Duty cycle: heat
over time" runs it in the background. Exact by definition: the dissipated power and its integral, the
first-order cooling. Model: the generic thermal numbers (rise at rated load 80 K, limit 100 K, brake
25 kJ/K, 15 min, 250 K, 20 MJ lining life) until the maker's replace them, losses from the equivalent
circuit, no temperature effect on resistance or friction, no drive heat sink model. Not a duty class
(S3, S5) rating of the motor. The duty run needs a step of at most 5 ms: a 10 ms step let a load hang
on the drive that had just started to lower it (not investigated further, the run refuses it).

Load sensing (optional, `dh_sense_boost_hz`): the torque is averaged at a probe frequency; a light load
may run the top step above its nominal frequency, never beyond what the motor can deliver with a margin
(breakdown torque of the fitted circuit on the V/f law). This is a generic function; the name and rule of
a maker's function are not reproduced.

What the run reports (`FindingKind`): a hook that sank at the start (reported at once when it runs away),
the hook travel between the brake command and standstill, a brake that does not hold the load with the
drive off, a drive trip (for example over-voltage when lowering without a braking resistor), the sensed
load and the no-torque and brake-feedback faults of the logic.

Exact by definition: the order of the states, the tick order, the hanging-load arithmetic. Model: the
drive (averaged), the motor (quasi-static torque, no flux transient), the flux build-up (one lag with the
rotor time constant of the fit, torque follows flux squared), stiction of the shaft against friction and
brake torque, the brake timing, all sequence timing. Not modelled: encoder or closed-loop control,
slack rope, rope stretch and hook swing, a maker's parameter
set. Parameters of a real drive never appear in a drawing, so the defaults are generic values for a
5.5 kW 4 pole hoist motor and are meant to be edited.

Where the user finds it: motor property sheet section "Drive hoist" (hook load turns the motor into a
drive hoist), then Simulate > Drive run with Live on; the window lists the readings and what the run found.

### Lane L2c as built (closed-loop hoists and synchronous winches)

`novo_drive::closed_loop` holds the encoder hoist drive (torque control at zero speed, torque proof
before brake release, load held under torque while the brake closes, STO, monitoring) and
`novo_machine::sync_winch` two winches on one load with rope stretch, tilt and the three
synchronisation modes. Design, hand cases and limits: `docs/design/closed-loop-winch.md`. UI:
Simulate > Winch scenarios.

### Lane L2c circuit binding as built (closed-loop winch in the drawn circuit)

`novo_machine::cl_winch_spec` (the `cw_*` property keys, marker `cw_mass_kg`) and `cl_winch_loop`
(`ClWinchLoop`, same tick order as the drive hoist loop) bind a closed-loop winch (`novo_drive::closed_loop`,
design `closed-loop-winch.md`) to the live circuit. The STO channels, the encoder signal, the brake
contactor and the commands are ordinary devices of the drawing, so the fault injection lane (F1) reaches the
winch through coils and wires. The only engine change is `ClInputs::brake_coil`, which lets the circuit
decide whether the brake coil is fed. Where the user finds it: motor property sheet section
"Closed-loop winch", then Simulate > Drive run with Live on.
