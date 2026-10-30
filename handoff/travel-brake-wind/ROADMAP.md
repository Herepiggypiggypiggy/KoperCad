# Travel brake, inertia and wind: roadmap

Scope: the travel drive of Simulate > Drive run (`novo-machine::travel_loop`, `travel_spec`) with a travel brake, a wind that varies in time, and a stop distance.

Already on main before this lane: reflected mass inertia, gearbox ratio and efficiency (rolling only), a constant wind force.

Done in this lane (PR "Travel brake, gusts, grip and stop distance"):
- Spring brake on the motor shaft, opened by the drive's brake output R2 (written to a bound relay contact, optionally through a brake contactor), delays and wear from `novo_drive::brake`.
- Brake force capped by the grip of the braked wheels (share of weight x friction coefficient); skid is reported.
- Wind from a speed (steady + one trapezoid gust) on a drag area, force from the relative air speed; the old constant force stays.
- Gearbox efficiency applied both ways (opposing load costs 1/eta, pushing load gives back eta).
- Stop distance record, roll-away, brake drag heat, drive slip findings.

Next (not started):
- Brake heat/wear over many stops (reuse `novo_drive::brake_heat`).
- Wheel slip as dynamics (spin up, slide) instead of a force cap.
- Skew and sway of the crane; the wind area changing with the load swing.
- Wind arrow drawn in the 3D picture (today: a status line).
- Controllers (pendant, radio): not part of this lane, asked back to the coordinator.
