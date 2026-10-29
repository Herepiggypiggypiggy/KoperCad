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
- The **Defects...** button on the Live potentials card (Shift+L).
- While a defect is injected a red strip, "DEFECT INJECTED", stays over the canvas. Leaving the live view
  repairs every defect, so a faulted plant never lingers unseen.

## Defects (`novo_sim::inject`)

| Defect | Meaning |
| --- | --- |
| Contact welded closed | Conducts whatever its coil or hand does. Without a pin: every make contact of the device (a contactor's mains and auxiliary make contacts). With a pin: that contact, also a break contact (a defeated interlock). `mirror` keeps the device's break contacts open too (positively guided contacts). |
| Contact stuck open | Never conducts, even with the coil on. |
| Protective device tripped | All its switched contacts open and stay open (overload relay, motor protection, drive fault relay). |
| Coil open | The winding is open: never operates (a brake that does not release). |
| Coil stuck on | Operates whatever feeds it (jammed armature, failed-on driver). |
| Conductor broken at a pin / wire broken by number | Every wire at the pin, or the wire with that number, is cut (E-stop chain, a loose terminal). |
| Supply lost | A potential (`L2`, `+24V`) reaches nothing: a phase or fuse gone. |
| Earth fault / pin shorted to a potential | The pin touches protective earth or another potential; a net that already carries a potential becomes `Shorted`. |

Devices are named by tag, short (`-K1`) when only one device ends that way, else in full (`=A1+CAB-K1`);
pins as `tag:pin`. A name that leads nowhere is reported in the card and the report, never ignored.

Machine-level defects (a lost encoder, a stuck brake in the mechanics, a drive that faults on overcurrent,
radio link loss) belong to the machine and controller models. Their circuit-visible form is available now:
brake coil open, receiver contacts stuck open or tripped, drive fault relay tripped. Binding the machine-level
causes to the same defect list is the next step (see ROADMAP in `handoff/fault-injection/`).

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
