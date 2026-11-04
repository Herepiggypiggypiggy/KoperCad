# Heat H5 (thermal mass and heat-up time): roadmap

Scope: transient heat-up with the real heat capacity of walls, plate and devices; time constants; duty cycles and day/night ambient. Part of the heat-research lanes (handoff/heat-research/ROADMAP.md, lane H5).

Done (PR 1): `novo-thermal::mass` (network of capacities and conductances, exact modal time stepping, slab discretisation, duty cycle and daily ambient phases, periodic steady state) with analytic tests; `novo-thermal::heatup::heat_up(scn, settings)` builds the cabinet network from a Scenario; Heat-up strip shows "Shell mass: 90 % in ..." with a hover.

Next, in order:
1. Wire into H1's `ShellModel` (layers, door, plate setback) when its contract lands: replace the single-material wall in `heatup::build`.
2. Put the wall/plate mass into the 2D `simulate_transient` and the 3D cabinet transient so the frames themselves slow down (today only the summary line has it).
3. Duty cycle / day-night controls in the Heat-up strip (the engine takes `Phase` lists already).
4. Rails, ducts and wires as mass (Obstacle kinds); device heat capacity from part data.
5. Validation with H6: measured heat-up of a real cabinet.

Open: the default device capacity (500 J/K per dm2) is heavy and dominates the time constant in many layouts.
