# Crane simulation example

A finished project you can run: the overhead crane cabinet with its control drawn the way a
radio receiver is really wired, so the hoist, the bridge and the crab move from the circuit.

## Where the user finds it

- **File > Templates and examples > Examples > Crane simulation**, the Start centre (Examples),
  or the command field: "crane simulation", "radio", "hoist". Command `file.example_crane_sim`.
- Then **Shift+L** (live view), **Simulate > Controller panel** (the radio is already in the
  project and bound), **Simulate > Hoist run** and **Simulate > Drive run** (start both), and
  **Simulate > Hoist in 3D**, which is already set to the bridge and crab axes.

Press Main on the radio, then move a rocker: the receiver contact closes, the drawn relay and
contactor pull in, the motor winding is fed, the brake lifts, and the hook, bridge or crab move
in the 3D picture until a drawn end limit cuts the fast step.

## What is drawn

Six pages and a laid-out cabinet (`+CAB`), the same cabinet as the plain crane example:

1. Supply, main contactor `-K0`, 48 V control transformer.
2. Hoist: reversing pair `-K11` / `-K12`, slow `-K13` and fast `-K14` winding contactors, star
   point `-K15`, brake rectifier and contactor `-K16`, pole-change motor `-M11`.
3. Travel: bridge `-U21`, `-M21` and crab `-U22`, `-M22` on frequency drives.
4. Receiver `-A1` (stop chain, main, four hoist outputs) and the hoist control rungs.
5. Receivers `-A2` (bridge) and `-A3` (crab), the travel relays `-K23` to `-K28`, the end
   limits `-S21` to `-S24` and the drive line contactors.
6. Drive commands to terminal strip `-X3`.

Differences from the plain crane example, all needed so the simulator can run it:

- The receiver is the library block `iec/controllers/outputs_6` with named pins, so the radio's
  signals bind to pins (`-A1:E1`, `-A1:M1`, `-A1:1` ...). Spare outputs may stay free.
- The enabled 48 V rail and the buses behind the main contactor and the reversing pair are
  interruption points of one name, not potential feeds. A feed is a source; an interruption
  point is a node, live only while the contacts in front of it are closed.
- The hoist end limits sit in the fast-step branch only, and the travel end limits in the fast
  relay: they drop the fast step, the slow step is still allowed (as the real two-step crane).

## What is stored in the project

- `-M11` carries `mech_*` properties (1 t, two falls, 5.5 / 1.4 kW, links to `-K0`, `-K16`,
  `-Q11` and the limits `-S11` / `-S12` at +4.5 m / -2.0 m).
- `-M21` and `-M22` carry `trav_*` properties (mass, gear, 3 kW, inputs `-K23` to `-K25` and
  `-K26` to `-K28`, line contactor, end limits).
- Project properties `crane3d.*` set the picture (24 m runway, 12 m span, axes `-M21`, `-M22`).
- The `controllers` add-on block holds the radio `radio-1` (generic 3 motions, 2 steps) and its
  14 bindings.

## Honest limits

A planning model, not a design: the numbers are generic and made up for a 1 t crane, with no
data from any customer. The simulation is of wiring, logic and averaged mechanics; it is not a
safety function (no PL or SIL claim) and the end limit behaviour is as drawn, not a standard.
The travel drives have no brake modelled in this example (set `trav_brake_torque_nm` to add one).
