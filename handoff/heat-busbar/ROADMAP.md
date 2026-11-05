# Busbar heat: roadmap

Scope: current to temperature along a busbar, with joints and terminals (research item 4 in research/calculix/SUMMARY.md).

Done (first PR): `novo_thermal::busbar` (1D finite volume, temperature-dependent resistivity, slab skin factor, fixed film or natural convection plus linearised radiation, three end conditions, joints, steady solve, heat-up curve, largest current for a limit); Calculations > "Busbar heat" tab.

Next, in order:
1. Heat into the cabinet: feed `Output::heat_to_air_w` (and the position) as a heat source into the 2D/3D scenario. Needs the H1/H7 contract; ask the heat researcher before touching `Scenario`.
2. Bars from the project (cabinet busbar items) instead of typed numbers.
3. Several bars in parallel / three phases with a computed proximity factor (now a typed number).
4. Round and tube sections, bars with several sections (steps in width).
5. Cross-check against CalculiX (H6 harness, optional) and a measured bar.
