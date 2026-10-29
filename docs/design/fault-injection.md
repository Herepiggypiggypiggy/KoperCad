# Fault injection and walk-throughs (lane F1)

Goal (Tadjiro, 2026-10-29): use the program to simulate the niche situations, not only the common ones, and
to rehearse complex ones: a welded contactor, an open brake coil, a broken E-stop chain, a lost phase, an
earth fault, then see what the drawn circuit does and why. Useful for safety walk-throughs and commissioning
rehearsal.

## What it is, and what it is not

It breaks the **drawn** circuit on purpose and solves it again with the same logic as the healthy live view
(coils, contacts, seal-ins, timers, potentials). It is a rehearsal of wiring and logic.

It is not a safety function, not a PL/SIL or FMEA tool and not a compliance result: no failure rates, no
diagnostic coverage, no response times of real devices beyond the timers that are drawn, no electrical
transients. A defect that shows nothing in the list may still matter in the plant, and a list that shows an
effect does not say how likely it is. The card and the printed report say so.

## Where the user finds it

- **Simulate > Defects and walk-throughs** (command `sim.defects`; command field: "defect", "fault", "weld",
  "inject", "scenario", "walk-through", "rehearsal").
- While a defect is injected a red strip, "DEFECT INJECTED", stays over the canvas. Leaving the live view
  repairs every defect, so a faulted plant never lingers unseen.

## Defects (`novo_sim::inject`)

| Defect | Meaning |
| --- | --- |
| Contact welded closed | Conducts whatever its coil or hand does. Without a pin: every make contact of the device (a contactor's mains and auxiliary make contacts). With a pin: that contact, also a break contact (a defeated interlock). `mirror` keeps the device's break contacts open too (positively guided contacts). |
| Contact stuck open | Never conducts, even with the coil on. |
| Protective device tripped | Its main and break contacts open and stay open, its make (signalling) contacts close, as an overload relay's 97-98 does (overload relay, motor protection, drive fault relay). |
| Coil open | The winding is open: never operates (a brake that does not release). |
| Coil stuck on | Operates whatever feeds it (jammed armature, failed-on driver). |
| Conductor broken at a pin / wire broken by number | Every wire at the pin, or the wire with that number, is cut (E-stop chain, a loose terminal). |
| Supply lost | A potential (`L2`, `+24V`) reaches nothing: a phase or fuse gone. |
| Earth fault / pin shorted to a potential | The pin touches protective earth or another potential; a net that already carries a potential becomes `Shorted`. |

Devices are named by tag, short (`-K1`) when only one device ends that way, else in full (`=A1+CAB-K1`);
pins as `tag:pin`. A name that leads nowhere is reported in the card and the report, never ignored.

### Machine-level defects (`novo_sim::machine`)

Defects of the machine the wiring drives, put on a hoist by a walk-through step (`machine`, `machine_repair`):
a lost supply phase, a supply at a share of rated, a brake that is slower or whose lining holds only a share of
its torque, and a brake rectifier fault (one diode open, shorted, coil circuit open, no over-excitation, low
output). They go through the hoist engine's `HoistFaults` (`novo-drive`), so the numbers are that model's: an
averaged brake and motor, not a measured drive.

With hoists in the project the walk-through runs tick by tick (5 ms) instead of jumping to the next event, once
for the healthy plant and once for the faulted one, each settling the drawn circuit every tick; the drawn limit
switches still cut the circuit. `expect` gains `hook` checks: at least or at most a height, standing or moving,
the hoist reports a condition (fed with the brake closed, brake slipping, phase lost, low voltage, both motors
fed) or reports nothing wrong. "What is different" lists a hook that is 5 cm or more away from the healthy
hoist's and any condition only the faulted hoist reports. A run with a hoist stops at 120 s (every 5 ms tick settles the circuit, and the card runs it on the UI thread).

Closed-loop (encoder) winches and winch pairs (`cw_*` properties, design `closed-loop-winch.md`) run in the same
way, on both plants, with one circuit settle per tick. Circuit defects reach them with no extra step: an open
coil or broken wire on an STO channel device (the drive latches the discrepancy and trips), a lost encoder signal
device, a brake contactor that does not pull in. `expect` gains `winch` checks (`{"what":"winch","winch":"=A1+FLD-M1","state":"tripped"}`):
`above` / `below` a hook height, `standing`, `moving`, `tripped`, `fault` with a drive code (`std`, `enc`),
`clear`, `sto_active`, and `found` with a finding name (`trip`, `sto_requested`, `encoder_signal_lost`, `brake_slip`,
`start_rollback`, and for a pair `pair_stopped`, `overload`, `slack`). For a pair the winch is named by either of its
two motor tags. "What is different" lists a winch hook 5 cm or more away from the healthy one, a drive that trips
where the healthy one does not, and the findings only the faulted run has. Hooks of a winch are a model (averaged
drive, spring-damper ropes), not a measurement.

Not built: drive overcurrent and a single lost encoder channel as a drawn device (the winch engine has it), radio
link loss (the controller model), recording hoist faults from the Hoist run window.

## Walk-throughs (`novo_sim::walkthrough`)

A JSON script of steps in time: `operate` / release a hand device (or one contact by pin, such as a receiver
output), `inject` a defect, `repair` one or all, and `expect` something about the **faulted** plant (a coil on or
off, a lamp lit or dark, a motor stopped, running, forward, reverse, humming on two phases or in fault, a net
dead, live, shorted or carrying a named potential).

`run` plays the script **twice** against the same drawing with the same operator inputs: a healthy plant and a
plant with the defects in it. Where the two differ is the effect of the defect. The report lists for each
step what newly differs ("motor -M1 is running forward (a healthy plant: stopped)", "short between L1 and L3 at
-K1:2") and what is back to normal, with the failed expectations and the Markdown form for printing.

Record and replay: the **Record** button collects the operator's moves (from the change of the hand-set
contacts) and the injected and repaired defects into a walk-through; **Run** replays it and shows the report.
Walk-throughs are saved in the project in one text property, `walkthroughs.project` (JSON list), one undo step
"Walk-throughs". A file that cannot be read is kept untouched and reported.

## Limits

- A script runs at most 3600 s (times and tail are cut, and the report says so). Between events the run steps in 0.05 s only while a timer counts, else straight to the next event.

- A running PLC program solves the circuit on its own path: defects are not applied while it runs (the card
  and the banner say so).
- Controller (radio) presses are not recorded, since they come from the controller model; a script presses
  the receiver contacts by pin instead.
- The healthy comparison uses the same inputs. A defect that changes what the operator would do is the
  operator's part of the script.
- Wire breaks are solved on a copy of the drawing without those wires; the original is never changed.

## Tests

`crates/novo-tests/tests/walkthrough_fault_injection.rs` (Gate 1 starter and the radio hoist): healthy run,
welded contactor, open coil, tripped overload and repair, lost phase, broken stop chain, earth fault, welded
up contactor with and without positively guided break contacts (the second keeps the interlock), unresolved
names, JSON round trip, the project is untouched. `crates/novo-app/src/app/defects_card_tests.rs`: the live
view with a welded contactor, leaving repairs, a cut wire, recorded steps, the saved property.
