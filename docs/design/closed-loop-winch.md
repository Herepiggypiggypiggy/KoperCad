# Closed-loop (encoder) hoists and synchronous winches (lane L2c)

Date 2026-10-29. Status: engine, scenarios and window built. Plan lane L2c in
`docs/design/drive-crane-simulation.md`.

## Why

Some hoists are not two-speed contactor hoists: the drive controls torque with an encoder on the
motor shaft, even at zero speed. The drive opens a holding brake through its own relay, only after
it has proven it can carry the load, and two winches can run on one load, alone or synchronous.
The failures that matter here are not in the steady state: a brake that opens before the torque
is there, torque removed before the brake holds, an encoder that dies, one drive of a pair that
trips, a drum that is one percent off.

## What is built

`novo-drive::closed_loop` (pure engine)

| Part | What it does | Exact or model |
| --- | --- | --- |
| `encoder` | Quadrature counts (4 ppr per revolution), speed over a window, position, failure injection (signal lost, direction channel lost) | counts and resolution exact; failures are scenarios |
| `sto` | Two channels; one low channel removes torque at once; a mismatch longer than the limit latches until both channels have been low | gating exact; the monitor is a model, no SIL or PL claim |
| `controller` | Speed PI around a first-order torque loop; torque limit from the current limit with constant power above rated speed; sequence Off, Magnetising, TorqueProof, Releasing, Running, Stopping, BrakeClosing; pre-torque (none, fixed, load cell, memory); jerk-limited ramp; monitoring (overspeed, speed deviation, rollback, encoder plausibility, brake feedback, motor I2t, DC bus); torque follower mode | ramp arithmetic exact; the rest model |
| `unit` | Drive + holding brake + hoist gravity on one shaft | gravity and reduction exact; brake delays are entered values |
| `findings` | Checks on the entered numbers (no run needed) | each states what it computed |

`novo-machine::sync_winch` (two winches on one beam)

Rigid beam of mass M and length L on two vertical ropes, centre of gravity e from the middle. Each
rope is a spring-damper that pulls only when taut. The beam is free to tilt or guided (tilt
locked). Exact statics of the stated model: with a free beam the rope forces are
`F_A = M g (1/2 + e/L)`, `F_B = M g (1/2 - e/L)` whatever the drums did and the tilt is
`(x_A - x_B) / L`; with a guided beam and equal rope stiffness the forces differ by `k (x_A - x_B)`.
Sync modes: same reference (no correction), position lock between encoders, torque follower.
Supervision: position error stop, partner stop on a drive fault (a link) or none.

`novo-machine::winch_scenarios`: six scripted scenarios with traces, events and findings, used by
the window.

## Where the user finds it

Simulate > Winch scenarios (`sim.winches`, Shift+Space search "winch"). The window runs a scenario
with a generic 30 kW hoist, draws hook, speed, torque and rope-force traces, lists the events and
the design findings, and shows which numbers are exact and which are a model.

## Hand-calculated cases (tests `crates/novo-drive/tests/closed_loop.rs`, `crates/novo-machine/tests/sync_winch.rs`)

1. 1024 ppr gives 4096 counts per revolution; one count per 5 ms window is 2.93 rpm.
2. Weight torque 2500 kg x 9.80665 x 0.2 m / 31.5 = 155.67 N m; the closed brake holds it.
3. Pre-torque equal to the weight: the load does not sag when the brake opens.
4. No pre-torque and no integral: the load creeps at `(T_s eta - T_min) / kp` (hand formula).
5. Torque removed 0.15 s before the brake applies: the drop is `a gap^2 / 2 + v^2 / (2 a_dec)` with
   `a = T_s eta / J`, `a_dec = (T_brake - T_s eta) / J`.
6. Static rope forces of a beam with the centre of gravity 0.4 m off: 29.42 kN and 19.61 kN.
7. Free beam, drum B 1 % larger: tilt equals the position difference over the length.
8. Guided beam, drum B 1 % larger: `F_B - F_A = k (x_B - x_A)`.
9. Torque follower: `F_B / F_A = mpr_A / mpr_B`, so no overload from the drum error.
10. Drive A trips, no link, guided beam: B carries `M g` = 49.0 kN against a 30 kN rope rating and
    needs 346 N m against a 320 N m limit, so it slows.

## Honest limits

See `novo_drive::closed_loop::LEAVES_OUT`. In short: averaged drive, perfect field orientation (a
lost encoder is detected, its effect on the field angle is not simulated), the encoder plausibility
check uses the true shaft speed above a threshold, generic parameters, fixed brake delays, linear
rope, vertical ropes (the snapshot flags a tilt beyond 5 degrees as outside the model). Not a
safety or compliance statement.

## The winch in the drawn circuit

A motor with a hook load under **Closed-loop winch** (`cw_*` properties, `novo_machine::cl_winch_spec`)
is a closed-loop winch in Simulate > Drive run (`novo_machine::cl_winch_loop`). The loop reads these
devices of the drawing each tick: up, down and speed step 2, the two STO channels (energised: torque
allowed), the encoder signal device (energised: signal present), the main contactor, the brake
contactor that feeds the coil, an optional brake feedback contact and a reset input. It writes the
drive's brake relay as a contact of the drawing and the end limit switches by hook height, as the
other hoist runs do. The brake coil follows the drawn brake contactor (`ClInputs::brake_coil`), so a
contactor that does not pull in leaves the brake closed and a welded one opens it before the drive
has torque.

Fault injection needs no machine specific hook: an open coil or a broken wire on an STO device makes
the channels disagree (the drive latches the discrepancy after its time), both low is STO at once, a
lost supply on the encoder device makes the counts stop, an open brake contactor coil keeps the hook
down. The run lists what it found: start rollback, stop travel, brake slip, trips, STO and encoder
events.

Fidelity: reading the devices and the tick order are exact by definition; everything inside the
drive is the model listed above. A channel or the encoder device that is not bound counts as always
present, and the run says so. The encoder loss here is a lost signal (counts stop). A lost single
channel (direction lost) is a second drawn device, the motor's "Encoder channel B device"
(`cw_encoder_b`): when its coil opens the counts keep coming but the direction is gone, the run
names it, and lowering trips the drive on the encoder code. Not bound means the channel is present.

## Two winches on one beam in the drawn circuit

A motor whose **Two winches on one beam** section names a second motor (`cw_pair_partner`, rows in
`cl_winch_spec::PAIR_KEYS`) makes the two a pair in Simulate > Drive run (`novo_machine::cl_pair_loop`).
Both motors keep their own Closed-loop winch rows. The beam (mass, rope distance, centre of gravity,
guided or free), the rope stiffness and damping, the rope rating, the synchronisation (independent,
position lock, torque follower), the position limit and the partner action are entered on the first
motor. The hook load of each winch is replaced by its share of the beam; the beam itself is
integrated by the pair (`sync_winch`).

The first winch's up, down and step 2 devices command both winches; the second winch's are not read
and the binding check says so when they are drawn. Everything else is read per winch: the two STO
channels, the encoder signal, the main contactor, the brake feed and feedback, and the reset. The
drive's brake relay of each winch is its own drawn contact. So an open coil on one STO channel of
winch B trips only winch B (the pair then stops through the partner link, or the position error
stops it first, as set), a lost encoder signal on B is named as winch B's, a brake contactor of B that
does not pull in leaves B's brake closed and the pair stops itself on the position error.

The run lists trips, STO and encoder events per winch, the pair stopping itself, a rope above its
rating, a slack rope and a tilt beyond what the model covers. Start rollback, stop travel and brake
slip are reported for a single winch only; for a pair read them off the snapshot rows.

Fidelity: the tick order and the beam statics are exact by definition; ropes as spring-dampers that
pull only when taut, the beam as a rigid body at small tilt with vertical ropes, and the drives are the
model listed above. No rope swing, no drum grooving, no rope over-lap. The default rope rating is
0.6 times the beam weight when none is entered, and the run says so.

## Walk-throughs

`novo_sim::walkthrough` runs the winch and pair loops on the healthy and the faulted plant, one circuit
settle per tick (the loops split their tick into `write_contacts` and `step_after_settle` for this). The
script writes circuit defects as for any plant and asks `winch` checks (`novo_sim::winch_check`): hook
above or below a height, standing, moving, tripped, tripped with a code, clear, STO active, or the run found
a trip, STO, a lost encoder signal, a brake slip, a start rollback, a pair stopping itself, a rope above
its rating or a slack rope. The report's "what is different" lists a hook gap of 5 cm or more, a drive that
trips only on the faulted plant and findings only the faulted run has. User: Simulate > Defects and
walk-throughs, saved walk-throughs, `▶ Run`; the fixture `qa/fixtures/closed-winch-walk.novo` has one.
Limits as for the hoist walk-throughs: a run with a machine in it stops at 120 s.

## Rated load and the load cell (single winch)

A winch with a rated load (`cw_swl_kg`, the safe working load) watches the force in its rope. The
load cell reads the weight of the hook mass plus its inertia (`m (g + a)`, the acceleration taken
from the hook speed of the last tick; the rope is rigid). Three things follow, all findings of
the run:

- over the rating (above 100 %): "The load cell reads N % of the rated load", once per crossing;
- over the cut-out (`cw_overload_pct`, 110 % when blank): lifting is inhibited, lowering and holding
  stay possible, and the cut-out lets go 2 % below its threshold so a load on the edge does not
  chatter. The drawing can light a lamp from the optional output device (`cw_overload_out`, its
  contact is written closed while lifting is inhibited);
- the load cell signal (`cw_load_cell`, a drawn device, energised: present) lost: the load is unknown,
  so lifting is inhibited (fail safe) and the run says so; back again releases it.

The reading beside the winch shows "load N % of rated" (and "lifting inhibited"). Walk-through checks
`found` with `overload_cut_out` or `load_cell_lost` ask for them.

Exact by definition: the arithmetic of the reading, the order of the tick, lifting only inhibited.
Model: the rigid rope (no rope dynamics in the reading), the load cell itself (no noise, no
calibration error, no response time) and the cut-out threshold behaviour of a real limiter, whose
standard (for example the dynamic factor and test loads) is not looked up here. A rated load is not
monitored in a pair (the pair has its own rope rating). Scenarios 1765 to 1765d, fixtures
`closed-winch-load.novo` and `closed-winch-overload.novo`.

## Not yet

- Unconfirmed customer details: which drive maker's hoist and encoder arrangement applies. Everything
  here is generic public knowledge.
