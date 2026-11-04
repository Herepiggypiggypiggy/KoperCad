# Busbar and cable impedance, skin and proximity effect, current sharing

Engine: `novo-emc::busbar`. Where the user finds it: **Check > Calculations > Busbar impedance** tab.

## What it computes
Straight, parallel conductors of equal length (bars or round cables), grouped into phases. Each cross-section is cut into a graded grid of filaments; filament resistance `rho l / a` (temperature-corrected) and partial inductances (Neumann integral of parallel filaments, Grover) give `Z = R + jwL`. The conductors of one group (a phase of several parallel bars) share one voltage drop and a stated total current; the solve returns the group impedance matrix, the voltage drop per phase, the current in every bar and filament (so the share between parallel bars), the loss per bar and the AC factor (loss over `R_dc I^2`).

## Exact versus model
Reproduced by hand in tests: partial mutual inductance of two filaments (1 m, 0.1 m: 4.1865e-7 H), flat-bar self inductance against Rosa's formula (0.2 %), DC resistance and its temperature rise, skin depth of copper (9.35 mm at 50 Hz), skin effect of a solid round wire against the Bessel solution (within 4 %), two-branch current sharing against `(R + jw(Lb-M)) / (R + jw(La-M))`, power balance (sum of `V I*` equals the sum of filament losses), mirror symmetry of the outer phases.

Model, not validated against a measurement: the real AC factor and sharing of a built busbar. Not seen: steel plate or enclosure, bends, taps, joints and contact resistance, stranding (cables are solid), capacitance, frequencies above the quasi-static limit, bars of different length. The partial reactance is only a loop value when the return is among the groups (three phases summing to zero). Resistivity and temperature coefficient are planning values (IEC 60228 / 60287 clauses unverified).

A wide isolated bar crowds to its edges even at 50 Hz (no return conductor nearby): this is physics of the partial model, tested, and it is why the slab formula is not used as a reference.

## For other engines
`busbar::solve(&Setup) -> Solution`. `Solution::filaments` lists every filament with position, size, current and loss (W) for a heat map; `conductors[i].loss_w`, `loss_w_per_m` and `flux_w_per_m2` feed the busbar heat solver; `filaments[].current` and `group_zeff` feed a force estimate. Up to 450 filaments per solve.

## Source
Written from textbook formulas (Grover, Rosa, Ruehli's partial elements, Kamon/Tsuk/White's filament method as published mathematics). No FastHenry code or text was used.
