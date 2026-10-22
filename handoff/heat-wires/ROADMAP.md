# Wire heat in ducts: roadmap

Scope: the I²R heat of wires lying in cable ducts, in the 2D heat view and the 3D CFD.

Done (PR from claude/project-thread-ianqu6):
1. `novo_thermal::wires`: resistance, I²R per conductor, sum per duct (hand-calculated tests).
2. `Obstacle::loss_w`: 2D adds it as an air source under the duct, 3D as volumetric heat in the duct box; both count it in the balance and the fan advice.
3. App glue `heat_wires.rs`: current per wire from the single-line runs, length per duct from the routed path, notes for the Heat panel; project input "Wire heat: share of load current" (Check > Electrical inputs).
4. QA fixture `qa/fixtures/cabinet-wire-heat.novo`, scenarios 61.

Next:
- Heat along the duct by where the wires actually run (today spread evenly along each duct).
- Heat of wire stubs between duct and terminal (counted nowhere today).
- Control and signal wires have no current in the model (they add nothing).
- Wire temperature from the heat balance instead of the insulation maximum.
- Load factor in transient runs scales wire heat linearly (should be squared).
